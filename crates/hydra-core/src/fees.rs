use crate::types::{CurrencyId, VenueId};

/// Exact tier-based fee schedule, zero-fee promo pair detector, and L2 gas deduction calculator.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct FeeEngine {
    pub binance_vip_tier: u8,
    pub binance_bnb_discount: bool,
    pub binance_zero_fee_stable_promos: bool,
    pub kraken_taker_bps: f64,
    pub eth_eur_price: f64,
}

impl Default for FeeEngine {
    fn default() -> Self {
        Self {
            binance_vip_tier: 0,
            binance_bnb_discount: true,
            binance_zero_fee_stable_promos: true,
            kraken_taker_bps: 16.0, // Kraken Pro tier taker fee in bps
            eth_eur_price: 2650.0,
        }
    }
}

impl FeeEngine {
    #[inline(always)]
    pub const fn new(
        binance_vip_tier: u8,
        binance_bnb_discount: bool,
        kraken_taker_bps: f64,
        eth_eur_price: f64,
    ) -> Self {
        Self {
            binance_vip_tier,
            binance_bnb_discount,
            binance_zero_fee_stable_promos: true,
            kraken_taker_bps,
            eth_eur_price,
        }
    }

    /// Returns the pair-specific taker fee fraction `f_e`, accounting for zero-fee stablecoin promo bridges
    /// (e.g., `EUR/USDC` and `USDC/USDT` on Binance) which drastically reduce multi-hop fee drag.
    #[inline(always)]
    pub fn pair_fee_fraction(
        &self,
        venue: VenueId,
        from: CurrencyId,
        to: CurrencyId,
        custom_uni_v3_fee_pips: Option<u32>,
    ) -> f64 {
        if self.binance_zero_fee_stable_promos
            && matches!(venue, VenueId::BinanceSpot | VenueId::BinanceMargin)
            && matches!(
                (from, to),
                (CurrencyId::EUR, CurrencyId::USDC)
                    | (CurrencyId::USDC, CurrencyId::EUR)
                    | (CurrencyId::USDC, CurrencyId::USDT)
                    | (CurrencyId::USDT, CurrencyId::USDC)
            )
        {
            return 0.0;
        }
        if matches!(venue, VenueId::BaseAerodromeVolatile)
            && matches!(
                (from, to),
                (CurrencyId::WETH, CurrencyId::USDC) | (CurrencyId::USDC, CurrencyId::WETH)
            )
        {
            return 0.0005; // 5 bps Aerodrome Slipstream WETH/USDC pool on Base Mainnet
        }
        if matches!(venue, VenueId::BaseUniV3)
            && matches!(
                (from, to),
                (CurrencyId::ETH, CurrencyId::USDC) | (CurrencyId::USDC, CurrencyId::ETH)
            )
        {
            return 0.0001; // 1 bps (100 pips) Uniswap V3 WETH/USDC pool 0xb4cb800910b228ed3d0834cf79d697127bbb00e5
        }
        self.venue_fee_fraction(venue, custom_uni_v3_fee_pips)
    }

    /// Returns the exact taker fee fraction `f_e` (e.g., `0.00075` for 7.5 bps) for a given venue.
    #[inline(always)]
    pub fn venue_fee_fraction(&self, venue: VenueId, custom_uni_v3_fee_pips: Option<u32>) -> f64 {
        match venue {
            VenueId::BinanceSpot | VenueId::BinanceMargin => {
                let base_bps = match self.binance_vip_tier {
                    0 => 10.0,
                    1 => 9.0,
                    2 => 8.0,
                    3 => 7.0,
                    4 => 6.0,
                    5 => 5.0,
                    6 => 4.0,
                    7 => 3.5,
                    8 => 3.0,
                    _ => 2.4,
                };
                let effective_bps = if self.binance_bnb_discount {
                    base_bps * 0.75 // 25% BNB deduction
                } else {
                    base_bps
                };
                effective_bps * 1e-4
            }
            VenueId::KrakenSpot => self.kraken_taker_bps * 1e-4,
            VenueId::BaseUniV3 | VenueId::ArbitrumUniV3 => {
                let pips = custom_uni_v3_fee_pips.unwrap_or(500); // default 5 bps (500 pips)
                (pips as f64) * 1e-6
            }
            VenueId::BaseAerodromeVolatile | VenueId::ArbitrumCamelotV2 => 0.0020, // 20 bps
            VenueId::BaseAerodromeStable | VenueId::BaseCurve | VenueId::ArbitrumCurve => 0.0001, // 1 bps
        }
    }

    /// Returns the fee retention factor `\gamma = 1.0 - f_e` used in constant-product AMM formulas.
    #[inline(always)]
    pub fn fee_retention_gamma(&self, venue: VenueId, custom_uni_v3_fee_pips: Option<u32>) -> f64 {
        1.0 - self.venue_fee_fraction(venue, custom_uni_v3_fee_pips)
    }

    /// Returns the FlashLoan fee fraction (Aave V3 = 5 bps = `0.0005`, Balancer Vault = `0.0`).
    #[inline(always)]
    pub const fn flashloan_fee_fraction(use_balancer_zero_fee: bool) -> f64 {
        if use_balancer_zero_fee {
            0.0
        } else {
            0.0005 // 5 bps Aave V3
        }
    }

    /// Estimates the exact L2 execution + L1 blob data gas cost in EUR (`C_gas`) for `HydraAtomicExecutor.sol`.
    #[inline(always)]
    pub fn estimate_l2_gas_eur(
        &self,
        num_hops: usize,
        l2_gas_price_gwei: f64,
        l1_data_fee_wei: u64,
        use_flashloan: bool,
    ) -> f64 {
        let base_gas = if use_flashloan { 165_000u64 } else { 110_000u64 };
        let hop_gas = 63_000u64 * (num_hops as u64); // Reduced via preApproveTokens warmup
        let total_l2_gas = base_gas + hop_gas;

        let l2_exec_eth = (total_l2_gas as f64) * l2_gas_price_gwei * 1e-9;
        let l1_blob_eth = (l1_data_fee_wei as f64) * 1e-18;

        (l2_exec_eth + l1_blob_eth) * self.eth_eur_price
    }
}
