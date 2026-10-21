/// Dynamic Priority Gas Bribe Calculator for L2 Arbitrage (Base Sequencer Optimization).
/// 
/// Automatically allocates a bounded share of expected trade profit (default: 15%)
/// to `max_priority_fee_per_gas_wei` to buy the top-of-block queue position from
/// the Base sequencer, eliminating frontrunning while strictly preserving net profit.

pub struct DynamicBribeEngine;

impl DynamicBribeEngine {
    /// Standard minimum Base priority fee floor: 0.001 Gwei (1,000,000 Wei).
    pub const MIN_PRIORITY_FEE_WEI: u128 = 1_000_000;
    /// Maximum priority fee cap to avoid excessive burns during abnormal volatility: 10.0 Gwei.
    pub const MAX_PRIORITY_FEE_WEI: u128 = 10_000_000_000;
    /// Typical L2 gas consumed by a 2-hop or 3-hop atomic flashloan transaction on HydraAtomicExecutor.
    pub const DEFAULT_GAS_ESTIMATE: u64 = 280_000;

    /// Calculates the optimal `max_priority_fee_per_gas_wei` and estimated total gas cost in EUR.
    ///
    /// # Arguments
    /// * `expected_net_profit_eur` - Net profit in EUR expected from the arbitrage before bribe.
    /// * `gas_used` - Estimated gas units (typically 250,000 - 350,000).
    /// * `eth_price_eur` - Spot price of ETH in EUR (e.g. 2450.0).
    /// * `profit_share_fraction` - Fraction of profit to share with sequencer (e.g. 0.15 for 15%).
    #[inline(always)]
    pub fn compute_bribe(
        expected_net_profit_eur: f64,
        gas_used: u64,
        eth_price_eur: f64,
        profit_share_fraction: f64,
    ) -> (u128, f64) {
        if expected_net_profit_eur <= 0.05 || gas_used == 0 || eth_price_eur <= 0.0 {
            let standard_cost_eur = ((Self::MIN_PRIORITY_FEE_WEI * (gas_used as u128)) as f64 / 1e18) * eth_price_eur.max(1.0);
            return (Self::MIN_PRIORITY_FEE_WEI, standard_cost_eur);
        }

        // Target bribe budget in EUR
        let bribe_budget_eur = expected_net_profit_eur * profit_share_fraction.clamp(0.01, 0.50);
        let bribe_budget_eth = bribe_budget_eur / eth_price_eur;
        let bribe_budget_wei = (bribe_budget_eth * 1e18) as u128;

        // Spread bribe across gas units
        let raw_tip_wei = bribe_budget_wei / (gas_used as u128);
        let effective_tip_wei = raw_tip_wei.clamp(Self::MIN_PRIORITY_FEE_WEI, Self::MAX_PRIORITY_FEE_WEI);
        let total_cost_eur = ((effective_tip_wei * (gas_used as u128)) as f64 / 1e18) * eth_price_eur;

        (effective_tip_wei, total_cost_eur)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_micro_trade_uses_standard_floor() {
        // Trade with 0.02 EUR profit should not overpay bribe
        let (tip, cost) = DynamicBribeEngine::compute_bribe(0.02, 280_000, 2450.0, 0.15);
        assert_eq!(tip, DynamicBribeEngine::MIN_PRIORITY_FEE_WEI);
        assert!(cost < 0.005);
    }

    #[test]
    fn test_lucrative_trade_scales_bribe_proportionally() {
        // Trade with 5.00 EUR profit at 2450 EUR/ETH
        // 15% bribe = 0.75 EUR
        let (tip, cost) = DynamicBribeEngine::compute_bribe(5.00, 280_000, 2450.0, 0.15);
        assert!(tip > DynamicBribeEngine::MIN_PRIORITY_FEE_WEI);
        assert!((cost - 0.75).abs() < 0.05);
    }
}
