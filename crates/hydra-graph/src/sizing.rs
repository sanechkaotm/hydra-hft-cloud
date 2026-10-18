use hydra_core::MAX_BOOK_LEVELS;

/// Represents a single directed leg's top-5 L2 Orderbook levels normalized into directed `(rate, max_input_qty)`.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct DirectedLegBook {
    /// Effective exchange rate `R_i(level)` (output units per 1 input unit) at each book level `0..level_count`.
    pub rates: [f64; MAX_BOOK_LEVELS],
    /// Maximum input volume absorbable at each level in the leg's input currency.
    pub capacities_in: [f64; MAX_BOOK_LEVELS],
    /// Venue taker fee fraction `f_i` (e.g., `0.00075` for 7.5 bps).
    pub fee_fraction: f64,
    /// Number of valid depth levels (`1..=5`).
    pub level_count: usize,
}

impl Default for DirectedLegBook {
    #[inline(always)]
    fn default() -> Self {
        Self {
            rates: [0.0; MAX_BOOK_LEVELS],
            capacities_in: [0.0; MAX_BOOK_LEVELS],
            fee_fraction: 0.0,
            level_count: 0,
        }
    }
}

/// Result of Case A: CEX Multi-Level L2 Orderbook Water-Filling Sizing.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct CexSizingResult {
    pub executable: bool,
    /// Optimal trade input `x^*` in base currency units (e.g., EUR or USDT).
    pub optimal_input_base: f64,
    /// Expected gross output in base currency after all fees and multi-level slippage.
    pub expected_output_base: f64,
    /// Expected net profit `expected_output_base - optimal_input_base`.
    pub expected_net_profit_base: f64,
    /// Worst-acceptable limit rate for each leg `0..hop_count` to set on IOC orders.
    pub worst_limit_rates: [f64; 4],
    /// Number of L2 book levels touched on each leg.
    pub levels_touched: [u8; 4],
}

/// Constant-product AMM pool parameters for a single directed hop `(R_in, R_out, \gamma = 1 - f)`.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct AmmPoolHop {
    pub reserve_in: f64,
    pub reserve_out: f64,
    /// Fee retention factor `\gamma_i = 1.0 - f_i` (e.g., `0.9995` for 5 bps).
    pub gamma: f64,
}

impl AmmPoolHop {
    #[inline(always)]
    pub const fn new(reserve_in: f64, reserve_out: f64, fee_fraction: f64) -> Self {
        Self {
            reserve_in,
            reserve_out,
            gamma: 1.0 - fee_fraction,
        }
    }

    /// Exact constant-product output for single hop: `(gamma * dx * R_out) / (R_in + gamma * dx)`.
    #[inline(always)]
    pub fn get_amount_out(&self, amount_in: f64) -> f64 {
        if amount_in <= 0.0 || self.reserve_in <= 0.0 || self.reserve_out <= 0.0 {
            return 0.0;
        }
        let amount_in_with_fee = amount_in * self.gamma;
        (amount_in_with_fee * self.reserve_out) / (self.reserve_in + amount_in_with_fee)
    }
}

/// Result of Case B: DEX AMM Closed-Form Quadratic Optimal Sizing.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct DexSizingResult {
    pub executable: bool,
    pub use_flashloan: bool,
    /// Unclamped profit-maximizing root `x^*`.
    pub unconstrained_optimal_in: f64,
    /// Clamped execution size `x_exec = min(x^*, C_cap)`.
    pub executable_input_base: f64,
    /// Expected output from the multi-hop AMM path before gas/flashloan fees.
    pub expected_amm_output_base: f64,
    /// FlashLoan fee in base currency (`0.0` if internal capital or Balancer Vault).
    pub flashloan_fee_base: f64,
    /// Estimated L2 + L1 blob gas cost in base currency (`C_gas`).
    pub gas_cost_base: f64,
    /// Final net expected profit `\Pi_net(x_exec)`.
    pub net_profit_base: f64,
}

/// Zero-allocation Dynamic Optimal Trade Sizing Engine (`hydra-graph::sizing`).
pub struct OptimalTradeSizer;

impl OptimalTradeSizer {
    /// **Case A: CEX Orderbook Walking (Tokyo & London Nodes)**
    ///
    /// Performs a piecewise linear water-filling pass across the top-5 L2 orderbook levels
    /// of a 3-hop or 4-hop cycle, accumulating input volume `\Delta x` only while:
    /// `M_slice = \prod_{i=1}^k R_i(level_i) * (1 - f_i) >= 1.0 + min_marginal_tau`
    /// AND `x_cum <= available_capital_base` AND book depth remains available.
    #[inline(always)]
    pub fn size_cex_orderbook_cycle(
        legs: &[DirectedLegBook],
        available_capital_base: f64,
        min_marginal_tau: f64,
    ) -> CexSizingResult {
        let hop_count = legs.len();
        if !(hop_count == 3 || hop_count == 4) || available_capital_base <= 0.0 {
            return CexSizingResult::default();
        }

        let mut rem_cap = [[0.0f64; MAX_BOOK_LEVELS]; 4];
        let mut lvl_idx = [0usize; 4];

        for (i, leg) in legs.iter().enumerate() {
            if leg.level_count == 0 {
                return CexSizingResult::default();
            }
            let count = leg.level_count.min(MAX_BOOK_LEVELS);
            rem_cap[i][..count].copy_from_slice(&leg.capacities_in[..count]);
        }

        let min_marginal_mult = 1.0 + min_marginal_tau;
        let mut total_in_base = 0.0f64;
        let mut total_out_base = 0.0f64;
        let mut worst_limit_rates = [0.0f64; 4];
        let mut levels_touched = [0u8; 4];

        // Water-filling loop bounded by max total level transitions (<= 4 * 5 = 20 slices)
        for _ in 0..24 {
            if total_in_base >= available_capital_base {
                break;
            }

            // 1. Check that all legs still have valid depth and compute marginal slice multiplier M_slice
            let mut m_slice = 1.0f64;
            let mut prefix_net_mult = [1.0f64; 5]; // cumulative net conversion from base to leg i input
            let mut exhausted = false;

            for i in 0..hop_count {
                let li = lvl_idx[i];
                if li >= legs[i].level_count.min(MAX_BOOK_LEVELS) {
                    exhausted = true;
                    break;
                }
                let r = legs[i].rates[li];
                if r <= 0.0 {
                    exhausted = true;
                    break;
                }
                let net_r = r * (1.0 - legs[i].fee_fraction);
                m_slice *= net_r;
                prefix_net_mult[i + 1] = m_slice;
            }

            if exhausted || m_slice < min_marginal_mult {
                break;
            }

            // 2. Find bottleneck slice input `\Delta x` in base currency across all legs and remaining capital
            let mut delta_x_base = available_capital_base - total_in_base;
            for i in 0..hop_count {
                let li = lvl_idx[i];
                let leg_cap_in_base = rem_cap[i][li] / prefix_net_mult[i];
                if leg_cap_in_base < delta_x_base {
                    delta_x_base = leg_cap_in_base;
                }
            }

            if delta_x_base <= 1e-9 {
                break;
            }

            // 3. Accumulate slice volume and update worst-acceptable IOC limit prices
            total_in_base += delta_x_base;
            total_out_base += delta_x_base * m_slice;

            for i in 0..hop_count {
                let li = lvl_idx[i];
                worst_limit_rates[i] = legs[i].rates[li];
                levels_touched[i] = (li + 1) as u8;

                let consumed_leg_in = delta_x_base * prefix_net_mult[i];
                rem_cap[i][li] -= consumed_leg_in;
                if rem_cap[i][li] <= 1e-7 {
                    lvl_idx[i] += 1;
                }
            }
        }

        let net_profit = total_out_base - total_in_base;
        CexSizingResult {
            executable: total_in_base > 0.0 && net_profit > 0.0,
            optimal_input_base: total_in_base,
            expected_output_base: total_out_base,
            expected_net_profit_base: net_profit,
            worst_limit_rates,
            levels_touched,
        }
    }

    /// **Case B: DEX AMM Closed-Form Optimal Sizing (Frankfurt Node)**
    ///
    /// Composes virtual reserves `(E_in, E_out)` recursively across constant-product AMM hops,
    /// solves the exact quadratic root `x^*`, clamps to `internal_capital` (or `flashloan_cap` if profitable
    /// after FlashLoan fee), and verifies `\Pi_net(x_exec) >= min_profit_base` after subtracting `c_gas_base`.
    #[inline(always)]
    pub fn size_dex_amm_cycle(
        pools: &[AmmPoolHop],
        internal_capital_base: f64,
        flashloan_cap_base: f64,
        flashloan_fee_fraction: f64,
        c_gas_base: f64,
        min_profit_base: f64,
    ) -> DexSizingResult {
        if pools.is_empty() {
            return DexSizingResult::default();
        }

        // 1. Recursive virtual pool composition (Section 3.2 Case B)
        let gamma_1 = pools[0].gamma;
        let mut e_in = pools[0].reserve_in;
        let mut e_out = pools[0].reserve_out;

        if e_in <= 0.0 || e_out <= 0.0 || gamma_1 <= 0.0 {
            return DexSizingResult::default();
        }

        for pool in &pools[1..] {
            let r_in = pool.reserve_in;
            let r_out = pool.reserve_out;
            let gamma_i = pool.gamma;
            let denom = r_in + e_out * gamma_i;
            if denom <= 0.0 {
                return DexSizingResult::default();
            }
            let next_e_in = (e_in * r_in) / denom;
            let next_e_out = (gamma_i * e_out * r_out) / denom;
            e_in = next_e_in;
            e_out = next_e_out;
        }

        // 2. Compute unrestricted profit-maximizing input x^* via exact quadratic root:
        //    d(out(x))/dx = (gamma_1 * E_in * E_out) / (E_in + gamma_1 * x)^2 = 1
        //    => x^* = (sqrt(gamma_1 * E_in * E_out) - E_in) / gamma_1
        let discriminant = gamma_1 * e_in * e_out;
        if discriminant <= e_in * e_in {
            return DexSizingResult::default();
        }
        let x_star = (discriminant.sqrt() - e_in) / gamma_1;
        if x_star <= 0.0 {
            return DexSizingResult::default();
        }

        // Helper closure: evaluate exact output through composed virtual pool
        let eval_out = |x: f64| -> f64 { (gamma_1 * e_out * x) / (e_in + gamma_1 * x) };

        // 3. Evaluate Internal Capital mode: x_int = min(x^*, internal_capital_base)
        let x_int = x_star.min(internal_capital_base.max(0.0));
        let out_int = eval_out(x_int);
        let net_int = out_int - x_int - c_gas_base;

        // 4. Evaluate FlashLoan mode if x^* > internal_capital_base and flashloan_cap_base > internal_capital_base
        //    When FlashLoan fee `f_flash` applies, optimal input solves d(out(x))/dx = 1 + f_flash:
        //    x^*_flash = (sqrt(gamma_1 * E_in * E_out / (1 + f_flash)) - E_in) / gamma_1
        let mut best_use_flash = false;
        let mut best_x = x_int;
        let mut best_out = out_int;
        let mut best_flash_fee = 0.0;
        let mut best_net = net_int;

        if flashloan_cap_base > internal_capital_base {
            let flash_hurdle = 1.0 + flashloan_fee_fraction;
            let x_star_flash = ((discriminant / flash_hurdle).sqrt() - e_in) / gamma_1;
            if x_star_flash > internal_capital_base {
                let x_flash = x_star_flash.min(flashloan_cap_base);
                let out_flash = eval_out(x_flash);
                let borrowed = (x_flash - internal_capital_base).max(0.0);
                let flash_fee = borrowed * flashloan_fee_fraction;
                let net_flash = out_flash - x_flash - flash_fee - c_gas_base;

                if net_flash > best_net {
                    best_use_flash = true;
                    best_x = x_flash;
                    best_out = out_flash;
                    best_flash_fee = flash_fee;
                    best_net = net_flash;
                }
            }
        }

        let executable = best_x > 0.0 && best_net >= min_profit_base;

        DexSizingResult {
            executable,
            use_flashloan: best_use_flash,
            unconstrained_optimal_in: x_star,
            executable_input_base: best_x,
            expected_amm_output_base: best_out,
            flashloan_fee_base: best_flash_fee,
            gas_cost_base: c_gas_base,
            net_profit_base: best_net,
        }
    }
}
