pub mod alerts;
pub mod killswitch;

pub use alerts::{AlertEvent, AlertSeverity, AsyncAlertReporter};
pub use killswitch::{
    BROKEN_LEG_WINDOW_NS, HaltReason, KillSwitch, MAX_BROKEN_CEX_LEGS_IN_WINDOW,
    MAX_CONSECUTIVE_DEX_REVERTS, MAX_DAILY_REVERT_GAS_MICRO_EUR, MAX_PING_RTT_NS,
    MAX_STALE_DATA_NS, REQUIRED_HEALTHY_HEARTBEATS,
};

#[cfg(test)]
mod tests {
    use super::*;
    use hydra_core::{NodeRole, VenueId};

    #[test]
    fn test_killswitch_trips_on_1pct_hwm_drawdown() {
        let ks = KillSwitch::new(1500.0);
        assert!(!ks.is_halted());

        // Compound equity up to €1,600 HWM
        assert!(ks.update_equity(1600.0).is_ok());

        // Minor dip to €1,586 (-0.875% from €1,600 HWM) -> still allowed
        assert!(ks.update_equity(1586.0).is_ok());
        assert!(!ks.is_halted());

        // Drop to €1,582 (< €1,600 * 0.990 = €1,584) -> immediate global halt!
        let err = ks.update_equity(1582.0).unwrap_err();
        assert_eq!(
            err,
            HaltReason::RollingDrawdownExceeded {
                hwm_micro_eur: 1_600_000_000,
                current_micro_eur: 1_582_000_000,
            }
        );
        assert!(ks.is_halted());
    }

    #[test]
    fn test_killswitch_trips_on_5_consecutive_dex_reverts_or_gas_drain() {
        let ks = KillSwitch::new(1500.0);

        for _ in 0..4 {
            assert!(ks.record_dex_revert(0.05).is_ok());
        }
        assert!(!ks.is_halted());

        // 5th consecutive revert trips GLOBAL_HALT
        let err = ks.record_dex_revert(0.05).unwrap_err();
        assert_eq!(err, HaltReason::MaxConsecutiveDexReverts(5));
        assert!(ks.is_halted());

        // Also test €2.50 daily revert gas drain breaker
        let ks_gas = KillSwitch::new(1500.0);
        assert!(ks_gas.record_dex_revert(1.40).is_ok());
        ks_gas.record_dex_success(); // Reset consecutive counter, but cumulative gas remains €1.40
        let gas_err = ks_gas.record_dex_revert(1.20).unwrap_err(); // Total €2.60 > €2.50
        assert_eq!(
            gas_err,
            HaltReason::DailyRevertGasDrainExceeded {
                spent_micro_eur: 2_600_000
            }
        );
        assert!(ks_gas.is_halted());

        // Test micro-capital scaled gas breaker for €20 trial run (cap = €0.40)
        let ks_micro = KillSwitch::new(20.0);
        assert!(ks_micro.record_dex_revert(0.25).is_ok());
        let micro_err = ks_micro.record_dex_revert(0.20).unwrap_err(); // Total €0.45 > €0.40
        assert_eq!(
            micro_err,
            HaltReason::DailyRevertGasDrainExceeded {
                spent_micro_eur: 450_000
            }
        );
        assert!(ks_micro.is_halted());
    }

    #[test]
    fn test_killswitch_broken_cex_legs_and_100_heartbeat_stale_recovery() {
        let ks = KillSwitch::new(1500.0);
        let t0 = 1_700_000_000_000_000_000u64;

        // First broken leg in 60s window is tolerated (after EmergencyUnwind)
        assert!(ks.record_broken_cex_leg(t0).is_ok());
        // Second broken leg 15s later (< 60s window) trips GLOBAL_HALT!
        assert_eq!(
            ks.record_broken_cex_leg(t0 + 15_000_000_000).unwrap_err(),
            HaltReason::MaxBrokenCexLegsInWindow(2)
        );
        assert!(ks.is_halted());

        // Test Stale Data (> 100ms) & 100-heartbeat recovery on a fresh KillSwitch
        let ks2 = KillSwitch::new(1500.0);
        assert!(ks2.check_venue_data_and_latency(
            VenueId::BinanceSpot,
            t0,
            t0 + 15_000_000, // 15ms age
            4_000_000,       // 4ms RTT
        ));

        // 120ms stale data blocks BinanceSpot and resets healthy heartbeat count to 0
        assert!(!ks2.check_venue_data_and_latency(
            VenueId::BinanceSpot,
            t0,
            t0 + 120_000_000,
            4_000_000,
        ));

        // 99 healthy heartbeats are still not enough to unblock
        for _ in 0..99 {
            ks2.record_venue_heartbeat(VenueId::BinanceSpot, 3_000_000);
        }
        assert!(!ks2.check_venue_data_and_latency(
            VenueId::BinanceSpot,
            t0 + 200_000_000,
            t0 + 205_000_000,
            3_000_000,
        ));

        // 100th consecutive healthy heartbeat restores venue eligibility!
        ks2.record_venue_heartbeat(VenueId::BinanceSpot, 3_000_000);
        assert!(ks2.check_venue_data_and_latency(
            VenueId::BinanceSpot,
            t0 + 200_000_000,
            t0 + 205_000_000,
            3_000_000,
        ));

        // Verify non-blocking alert reporter
        let reporter = AsyncAlertReporter::new("99887766");
        assert!(reporter.emit_nonblocking(AlertEvent::new(
            NodeRole::Frankfurt,
            AlertSeverity::CriticalKillSwitch,
            Some(HaltReason::MaxConsecutiveDexReverts(5)),
            t0,
            "HALT: 5 consecutive DEX reverts",
        )));
        let mut buf = [0u8; 512];
        let len = reporter.pop_and_format_json(&mut buf).unwrap();
        let s = std::str::from_utf8(&buf[..len]).unwrap();
        assert!(s.contains("FRANKFURT"));
        assert!(s.contains("HALT: 5 consecutive DEX reverts"));
    }
}
