use hydra_core::VenueId;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Maximum allowable data staleness: 100 milliseconds (`100_000_000` ns).
pub const MAX_STALE_DATA_NS: u64 = 100_000_000;

/// Maximum allowable exchange ping RTT: 25 milliseconds (`25_000_000` ns).
pub const MAX_PING_RTT_NS: u64 = 25_000_000;

/// Required consecutive healthy heartbeats before unblocking a degraded venue.
pub const REQUIRED_HEALTHY_HEARTBEATS: u64 = 100;

/// Rolling window for broken CEX leg counting: 60 seconds (`60_000_000_000` ns).
pub const BROKEN_LEG_WINDOW_NS: u64 = 60_000_000_000;

/// Maximum consecutive DEX transaction reverts before global halt.
pub const MAX_CONSECUTIVE_DEX_REVERTS: u64 = 5;

/// Maximum broken CEX legs within a rolling 60-second window before global halt.
pub const MAX_BROKEN_CEX_LEGS_IN_WINDOW: u64 = 2;

/// Maximum daily reverted L2 gas expenditure in micro-EUR (`€2.50` = `2_500_000` micro-EUR).
pub const MAX_DAILY_REVERT_GAS_MICRO_EUR: u64 = 2_500_000;

/// Reason why the Kill-Switch or Venue Circuit Breaker tripped.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HaltReason {
    MaxConsecutiveDexReverts(u64),
    MaxBrokenCexLegsInWindow(u64),
    RollingDrawdownExceeded {
        hwm_micro_eur: u64,
        current_micro_eur: u64,
    },
    DailyRevertGasDrainExceeded {
        spent_micro_eur: u64,
    },
    RemoteMeshKillSwitchSignal,
}

/// Lock-free Military-Grade Kill-Switch and Venue Circuit Breaker (`hydra-risk::killswitch`).
pub struct KillSwitch {
    /// Global atomic halt flag shared across all execution, networking, and solver threads.
    pub global_halt: AtomicBool,
    /// Starting day equity in micro-EUR (`1 EUR = 1_000_000`).
    pub start_equity_micro_eur: AtomicU64,
    /// Rolling high-water mark equity in micro-EUR (`E_hwm`).
    pub hwm_equity_micro_eur: AtomicU64,
    /// Current equity in micro-EUR (`E_current`).
    pub current_equity_micro_eur: AtomicU64,
    /// Consecutive DEX revert counter.
    pub consecutive_reverts: AtomicU64,
    /// Broken CEX legs count inside current rolling 60-second window.
    pub broken_cex_legs: AtomicU64,
    /// Timestamp (ns) of the first broken CEX leg in the current 60s window.
    pub broken_leg_window_start_ns: AtomicU64,
    /// Cumulative EUR spent on reverted L2 DEX transactions today (scaled by `1e6` micro-EUR).
    pub daily_revert_gas_micro_eur: AtomicU64,
    /// Per-venue consecutive healthy heartbeats counter (`100` required to clear venue block).
    pub venue_healthy_heartbeats: [AtomicU64; 10],
}

impl KillSwitch {
    pub fn new(initial_equity_eur: f64) -> Self {
        let initial_micro = (initial_equity_eur.max(0.0) * 1_000_000.0).round() as u64;
        Self {
            global_halt: AtomicBool::new(false),
            start_equity_micro_eur: AtomicU64::new(initial_micro),
            hwm_equity_micro_eur: AtomicU64::new(initial_micro),
            current_equity_micro_eur: AtomicU64::new(initial_micro),
            consecutive_reverts: AtomicU64::new(0),
            broken_cex_legs: AtomicU64::new(0),
            broken_leg_window_start_ns: AtomicU64::new(0),
            daily_revert_gas_micro_eur: AtomicU64::new(0),
            venue_healthy_heartbeats: std::array::from_fn(|_| {
                AtomicU64::new(REQUIRED_HEALTHY_HEARTBEATS)
            }),
        }
    }

    /// Fast hot-path check: returns `true` if global trading is halted.
    #[inline(always)]
    pub fn is_halted(&self) -> bool {
        self.global_halt.load(Ordering::Acquire)
    }

    /// Immediately trips `GLOBAL_HALT = true`.
    #[inline(always)]
    pub fn trigger_global_halt(&self) {
        self.global_halt.store(true, Ordering::Release);
    }

    /// Updates current account equity and enforces the 1.0% High-Water Mark Drawdown Breaker:
    /// Halts immediately if `E_current < E_hwm * 0.990`.
    #[inline(always)]
    pub fn update_equity(&self, current_equity_eur: f64) -> Result<(), HaltReason> {
        let current_micro = (current_equity_eur.max(0.0) * 1_000_000.0).round() as u64;
        self.current_equity_micro_eur
            .store(current_micro, Ordering::Release);

        // Update High-Water Mark if equity reached a new peak
        let mut hwm = self.hwm_equity_micro_eur.load(Ordering::Acquire);
        while current_micro > hwm {
            match self.hwm_equity_micro_eur.compare_exchange_weak(
                hwm,
                current_micro,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    hwm = current_micro;
                    break;
                }
                Err(actual) => hwm = actual,
            }
        }

        // Floor = E_hwm * 990 / 1000 (1.0% max drawdown)
        let floor_micro = (hwm as u128 * 990 / 1000) as u64;
        if current_micro < floor_micro {
            self.trigger_global_halt();
            return Err(HaltReason::RollingDrawdownExceeded {
                hwm_micro_eur: hwm,
                current_micro_eur: current_micro,
            });
        }
        Ok(())
    }

    /// Records a successful DEX trade, resetting the consecutive revert counter to 0.
    #[inline(always)]
    pub fn record_dex_success(&self) {
        self.consecutive_reverts.store(0, Ordering::Release);
    }

    /// Records a reverted DEX transaction and checks both:
    /// 1) `consecutive_reverts >= 5`
    /// 2) `daily_revert_gas_eur > €2.50`
    #[inline(always)]
    pub fn record_dex_revert(&self, revert_gas_eur: f64) -> Result<(), HaltReason> {
        let rev_count = self.consecutive_reverts.fetch_add(1, Ordering::AcqRel) + 1;
        let gas_micro = (revert_gas_eur.max(0.0) * 1_000_000.0).round() as u64;
        let total_gas_micro = self
            .daily_revert_gas_micro_eur
            .fetch_add(gas_micro, Ordering::AcqRel)
            + gas_micro;

        if rev_count >= MAX_CONSECUTIVE_DEX_REVERTS {
            self.trigger_global_halt();
            return Err(HaltReason::MaxConsecutiveDexReverts(rev_count));
        }

        if total_gas_micro > MAX_DAILY_REVERT_GAS_MICRO_EUR {
            self.trigger_global_halt();
            return Err(HaltReason::DailyRevertGasDrainExceeded {
                spent_micro_eur: total_gas_micro,
            });
        }

        Ok(())
    }

    /// Records a broken CEX multi-leg execution (partial fill / leg break) within a rolling 60s window.
    /// Trips `GLOBAL_HALT = true` if `broken_cex_legs >= 2` within 60 seconds.
    #[inline(always)]
    pub fn record_broken_cex_leg(&self, now_ns: u64) -> Result<(), HaltReason> {
        let win_start = self.broken_leg_window_start_ns.load(Ordering::Acquire);
        if win_start == 0 || now_ns.saturating_sub(win_start) > BROKEN_LEG_WINDOW_NS {
            self.broken_leg_window_start_ns
                .store(now_ns, Ordering::Release);
            self.broken_cex_legs.store(1, Ordering::Release);
            return Ok(());
        }

        let count = self.broken_cex_legs.fetch_add(1, Ordering::AcqRel) + 1;
        if count >= MAX_BROKEN_CEX_LEGS_IN_WINDOW {
            self.trigger_global_halt();
            return Err(HaltReason::MaxBrokenCexLegsInWindow(count));
        }
        Ok(())
    }

    /// Latency & Stale Data Guard:
    /// Checks `now_ns - local_recv_ts_ns <= 100ms` and `ping_rtt_ns <= 25ms`.
    /// If violated, zeroes the venue's consecutive healthy heartbeats counter and blocks execution
    /// on that venue until 100 consecutive healthy heartbeats are recorded.
    #[inline(always)]
    pub fn check_venue_data_and_latency(
        &self,
        venue: VenueId,
        local_recv_ts_ns: u64,
        now_ns: u64,
        ping_rtt_ns: u64,
    ) -> bool {
        if self.is_halted() {
            return false;
        }
        let v_idx = venue as usize;
        let age_ns = now_ns.saturating_sub(local_recv_ts_ns);

        if age_ns > MAX_STALE_DATA_NS || ping_rtt_ns > MAX_PING_RTT_NS {
            self.venue_healthy_heartbeats[v_idx].store(0, Ordering::Release);
            return false;
        }

        let hb = self.venue_healthy_heartbeats[v_idx].load(Ordering::Acquire);
        hb >= REQUIRED_HEALTHY_HEARTBEATS
    }

    /// Records a healthy heartbeat (`ping_rtt_ns <= 25ms`) for `venue`.
    #[inline(always)]
    pub fn record_venue_heartbeat(&self, venue: VenueId, ping_rtt_ns: u64) -> u64 {
        let v_idx = venue as usize;
        if ping_rtt_ns > MAX_PING_RTT_NS {
            self.venue_healthy_heartbeats[v_idx].store(0, Ordering::Release);
            0
        } else {
            self.venue_healthy_heartbeats[v_idx].fetch_add(1, Ordering::AcqRel) + 1
        }
    }
}
