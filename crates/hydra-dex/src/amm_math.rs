use hydra_graph::AmmPoolHop;

/// Constant-Product AMM Math (`Uniswap V2` / `Aerodrome Volatile` / `Camelot V2`).
pub struct ConstantProductMath;

impl ConstantProductMath {
    /// Computes exact output `dy` given input `dx`, reserves `(r_in, r_out)`, and fee fraction `f`.
    #[inline(always)]
    pub fn get_amount_out(dx: f64, r_in: f64, r_out: f64, fee_fraction: f64) -> f64 {
        if dx <= 0.0 || r_in <= 0.0 || r_out <= 0.0 {
            return 0.0;
        }
        let gamma = 1.0 - fee_fraction;
        let dx_with_fee = dx * gamma;
        (dx_with_fee * r_out) / (r_in + dx_with_fee)
    }

    /// Computes marginal top-of-book spot exchange rate `R_{u -> v}` before fee.
    #[inline(always)]
    pub fn marginal_rate(r_in: f64, r_out: f64) -> f64 {
        if r_in <= 0.0 {
            0.0
        } else {
            r_out / r_in
        }
    }
}

/// Concentrated Liquidity AMM Math (`Uniswap V3` / `Aerodrome Slipstream`).
pub struct UniV3Math;

impl UniV3Math {
    pub const Q96: f64 = 79_228_162_514_264_337_593_543_950_336.0; // 2^96

    /// Converts Q64.96 `sqrt_price_x96` (`u128`) into floating-point `sqrt(P)` adjusted for token decimal delta.
    #[inline(always)]
    pub fn sqrt_price_from_x96(sqrt_price_x96: u128, decimals_0: u8, decimals_1: u8) -> f64 {
        let raw_sqrt_p = (sqrt_price_x96 as f64) / Self::Q96;
        let dec_diff = (decimals_0 as i32) - (decimals_1 as i32);
        let scale = 10.0f64.powi(dec_diff);
        raw_sqrt_p * scale.sqrt()
    }

    /// Computes exact spot price `P` (units of `token1` per 1 unit of `token0`).
    #[inline(always)]
    pub fn spot_price_token0_to_token1(
        sqrt_price_x96: u128,
        decimals_0: u8,
        decimals_1: u8,
    ) -> f64 {
        let sp = Self::sqrt_price_from_x96(sqrt_price_x96, decimals_0, decimals_1);
        sp * sp
    }

    /// Projects active tick liquidity `L` and `sqrtPrice` into an equivalent virtual constant-product
    /// `AmmPoolHop` (`R_virtual_0 = L / sqrt(P)`, `R_virtual_1 = L * sqrt(P)`) for closed-form sizing.
    #[inline(always)]
    pub fn to_virtual_amm_hop(
        sqrt_price: f64,
        normalized_liquidity_l: f64,
        zero_for_one: bool,
        fee_pips: u32,
    ) -> AmmPoolHop {
        let fee_fraction = (fee_pips as f64) * 1e-6;
        if sqrt_price <= 0.0 || normalized_liquidity_l <= 0.0 {
            return AmmPoolHop::new(0.0, 0.0, fee_fraction);
        }
        let r_virt_0 = normalized_liquidity_l / sqrt_price;
        let r_virt_1 = normalized_liquidity_l * sqrt_price;

        if zero_for_one {
            AmmPoolHop::new(r_virt_0, r_virt_1, fee_fraction)
        } else {
            AmmPoolHop::new(r_virt_1, r_virt_0, fee_fraction)
        }
    }

    /// Computes exact single-tick output within a Uniswap V3 active tick range.
    #[inline(always)]
    pub fn compute_exact_input_single_tick(
        amount_in: f64,
        sqrt_price: f64,
        normalized_liquidity_l: f64,
        zero_for_one: bool,
        fee_pips: u32,
    ) -> f64 {
        let hop = Self::to_virtual_amm_hop(
            sqrt_price,
            normalized_liquidity_l,
            zero_for_one,
            fee_pips,
        );
        hop.get_amount_out(amount_in)
    }
}

/// Curve Finance StableSwap Invariant (`A * n^n * sum(x_i) + D = A * D * n^n + D^(n+1) / (n^n * prod(x_i))`)
/// Zero-Allocation Newton-Raphson Solver.
pub struct CurveStableSwapMath;

impl CurveStableSwapMath {
    /// Computes the StableSwap invariant `D` for a 2-coin pool `(x, y)` with amplification coefficient `amp`.
    #[inline(always)]
    pub fn compute_invariant_d(x: f64, y: f64, amp: f64) -> f64 {
        let s = x + y;
        if s <= 0.0 {
            return 0.0;
        }
        let ann = amp * 4.0; // A * n^n where n = 2
        let mut d = s;

        for _ in 0..16 {
            // d_p = D^3 / (4 * x * y)
            let d_p = (d * d * d) / (4.0 * x * y);
            let d_prev = d;
            let num = (ann * s + 2.0 * d_p) * d;
            let den = (ann - 1.0) * d + 3.0 * d_p;
            d = num / den;
            if (d - d_prev).abs() <= 1e-9 {
                break;
            }
        }
        d
    }

    /// Computes exact output `dy` when swapping `dx` into a 2-coin Curve StableSwap pool `(x, y)`
    /// with amplification `amp` and fee fraction `fee_fraction`.
    #[inline(always)]
    pub fn get_dy(x: f64, y: f64, dx: f64, amp: f64, fee_fraction: f64) -> f64 {
        if dx <= 0.0 || x <= 0.0 || y <= 0.0 {
            return 0.0;
        }
        let d = Self::compute_invariant_d(x, y, amp);
        let ann = amp * 4.0;
        let x_new = x + dx;

        // Solve quadratic/Newton iteration for y_new given x_new and D:
        // y^2 + y * (x_new + D/ann - D) = D^3 / (4 * ann * x_new)
        let c = (d * d * d) / (4.0 * ann * x_new);
        let b = x_new + (d / ann);

        let mut y_new = d;
        for _ in 0..16 {
            let y_prev = y_new;
            y_new = (y_new * y_new + c) / (2.0 * y_new + b - d);
            if (y_new - y_prev).abs() <= 1e-9 {
                break;
            }
        }

        let dy_gross = (y - y_new).max(0.0);
        dy_gross * (1.0 - fee_fraction)
    }
}
