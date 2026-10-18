pub mod graph;
pub mod sizing;
pub mod spfa;

pub use graph::{DirectedEdge, ExchangeGraph, INVALID_EDGE_IDX, MAX_OUT_DEGREE};
pub use sizing::{AmmPoolHop, CexSizingResult, DexSizingResult, DirectedLegBook, OptimalTradeSizer};
pub use spfa::{BoundedSpfaDetector, CandidateCycle, MAX_CANDIDATE_CYCLES};

#[cfg(test)]
mod tests {
    use super::*;
    use hydra_core::{CurrencyId, VenueId};
    use std::time::Instant;

    #[test]
    fn test_spfa_detects_3hop_and_4hop_exotic_cycles_under_100us() {
        let mut graph = ExchangeGraph::new();
        let ts = 1_700_000_000_000_000_000u64;

        // 4-hop exotic fiat/stablecoin cycle: EUR -> USDT -> TRY -> BRL -> EUR
        // Rates chosen so net product after 7.5 bps fee per leg = ~1.0068 (+68 bps > 4 bps threshold)
        // Leg 1: 1 EUR -> 1.0900 USDT (fee 0.00075)
        graph.upsert_edge(CurrencyId::EUR, CurrencyId::USDT, VenueId::BinanceSpot, 1.0900, 0.00075, 50_000.0, ts);
        // Leg 2: 1 USDT -> 34.50 TRY (fee 0.00075)
        graph.upsert_edge(CurrencyId::USDT, CurrencyId::TRY, VenueId::BinanceSpot, 34.50, 0.00075, 50_000.0, ts);
        // Leg 3: 1 TRY -> 0.1680 BRL (fee 0.00075)
        graph.upsert_edge(CurrencyId::TRY, CurrencyId::BRL, VenueId::BinanceSpot, 0.1680, 0.00075, 1_500_000.0, ts);
        // Leg 4: 1 BRL -> 0.1592 EUR (fee 0.00075)
        // Gross product = 1.09 * 34.50 * 0.1680 * 0.1592 = 1.005768
        // Net product after 4 legs of 7.5 bps = 1.005768 * (0.99925)^4 = 1.00275 (+27.5 bps > 4 bps!)
        graph.upsert_edge(CurrencyId::BRL, CurrencyId::EUR, VenueId::BinanceSpot, 0.1592, 0.00075, 250_000.0, ts);

        // Also add a 3-hop cycle: EUR -> USDC -> GBP -> EUR
        // Gross: 1.0900 * 0.7850 * 1.1740 = 1.00453
        // Net after 3 legs of 5 bps: 1.00453 * (0.9995)^3 = 1.00302 (+30.2 bps)
        graph.upsert_edge(CurrencyId::EUR, CurrencyId::USDC, VenueId::KrakenSpot, 1.0900, 0.0005, 40_000.0, ts);
        graph.upsert_edge(CurrencyId::USDC, CurrencyId::GBP, VenueId::KrakenSpot, 0.7850, 0.0005, 40_000.0, ts);
        graph.upsert_edge(CurrencyId::GBP, CurrencyId::EUR, VenueId::KrakenSpot, 1.1740, 0.0005, 40_000.0, ts);

        let detector = BoundedSpfaDetector::new(0.0004); // 4 bps threshold

        // Warmup & latency verification (< 100 microseconds)
        let start = Instant::now();
        let candidates = detector.scan_all_anchors(&graph);
        let elapsed_us = start.elapsed().as_micros();

        assert!(
            elapsed_us < 100,
            "SPFA cycle detection took {}us, exceeding the 100us HFT budget!",
            elapsed_us
        );
        assert!(candidates.len() >= 2, "Should detect both 3-hop and 4-hop profitable cycles");

        // Verify the 4-hop EUR -> USDT -> TRY -> BRL -> EUR cycle is present
        let mut found_4hop = false;
        let mut found_3hop = false;
        for i in 0..candidates.len() {
            let c = candidates[i];
            if c.hop_count == 4
                && c.nodes[0] == CurrencyId::EUR
                && c.nodes[1] == CurrencyId::USDT
                && c.nodes[2] == CurrencyId::TRY
                && c.nodes[3] == CurrencyId::BRL
                && c.nodes[4] == CurrencyId::EUR
            {
                found_4hop = true;
                assert!(c.net_profit_bps > 20.0);
                assert!(c.total_log_weight_scaled < detector.threshold_log_weight_scaled);
            }
            if c.hop_count == 3
                && c.nodes[0] == CurrencyId::EUR
                && c.nodes[1] == CurrencyId::USDC
                && c.nodes[2] == CurrencyId::GBP
                && c.nodes[3] == CurrencyId::EUR
            {
                found_3hop = true;
                assert!(c.net_profit_bps > 25.0);
            }
        }
        assert!(found_4hop, "4-hop EUR->USDT->TRY->BRL->EUR cycle not detected");
        assert!(found_3hop, "3-hop EUR->USDC->GBP->EUR cycle not detected");
    }

    #[test]
    fn test_cycles_where_fees_exceed_gross_spread_are_strictly_rejected() {
        let mut graph = ExchangeGraph::new();
        let ts = 1_700_000_000_000_000_000u64;

        // Gross product = 1.0900 * 0.9190 * 1.0000 = 1.00171 (+17.1 bps gross)
        // But 3 legs of 7.5 bps fee = 22.5 bps total fee -> Net = -5.4 bps loss!
        graph.upsert_edge(CurrencyId::EUR, CurrencyId::USDT, VenueId::BinanceSpot, 1.0900, 0.00075, 10_000.0, ts);
        graph.upsert_edge(CurrencyId::USDT, CurrencyId::USDC, VenueId::BinanceSpot, 1.0000, 0.00075, 10_000.0, ts);
        graph.upsert_edge(CurrencyId::USDC, CurrencyId::EUR, VenueId::BinanceSpot, 0.9190, 0.00075, 10_000.0, ts);

        let detector = BoundedSpfaDetector::new(0.0004);
        let candidates = detector.scan_all_anchors(&graph);
        assert_eq!(candidates.len(), 0, "Fee-negative cycle must be strictly rejected!");
    }

    #[test]
    fn test_optimal_sizing_caps_shallow_orderbook_and_amm_depth_on_1500_eur_balance() {
        // 1. Test Case A: CEX Orderbook Walking with €1,500 available capital on a shallow orderbook
        // Level 0 has €200 capacity at profitable spread (+35 bps);
        // Level 1 has €250 capacity at marginal profitable spread (+8 bps);
        // Level 2 has €5,000 capacity at losing spread (-40 bps).
        // Sizer MUST stop after Level 0 + Level 1 (€450 total) and NOT push all €1,500 into Level 2!
        let leg1 = DirectedLegBook {
            rates: [1.0900, 1.0885, 1.0800, 0.0, 0.0],
            capacities_in: [200.0, 250.0, 5000.0, 0.0, 0.0],
            fee_fraction: 0.0005,
            level_count: 3,
        };
        let leg2 = DirectedLegBook {
            rates: [34.50, 34.48, 34.20, 0.0, 0.0],
            capacities_in: [10_000.0, 10_000.0, 50_000.0, 0.0, 0.0],
            fee_fraction: 0.0005,
            level_count: 3,
        };
        let leg3 = DirectedLegBook {
            rates: [0.02672, 0.02669, 0.02650, 0.0, 0.0],
            capacities_in: [500_000.0, 500_000.0, 1_000_000.0, 0.0, 0.0],
            fee_fraction: 0.0005,
            level_count: 3,
        };

        let cex_res = OptimalTradeSizer::size_cex_orderbook_cycle(
            &[leg1, leg2, leg3],
            1500.0, // €1,500 balance
            0.0002, // 2 bps min marginal slice hurdle
        );

        assert!(cex_res.executable);
        assert!(
            (cex_res.optimal_input_base - 450.0).abs() < 1e-6,
            "Expected optimal CEX input to cap at €450 shallow depth, got {}",
            cex_res.optimal_input_base
        );
        assert!(cex_res.expected_net_profit_base > 0.80);
        assert_eq!(cex_res.levels_touched[0], 2);

        // 2. Test Case B: DEX AMM Quadratic Sizing with shallow constant-product pool
        // Pool reserves are small (~15,000 EUR equivalent), so pushing €1,500 would destroy spread via slippage.
        // Closed-form quadratic root x^* should pick ~€95 optimal size instead of €1,500!
        let hops = [
            AmmPoolHop::new(15_000.0, 16_500.0, 0.0005), // EURC -> USDC (1.10 spot)
            AmmPoolHop::new(16_500.0, 8.25, 0.0005),     // USDC -> WETH
            AmmPoolHop::new(8.25, 15_225.0, 0.0005),     // WETH -> EURC (+1.5% spot discrepancy)
        ];

        let dex_res = OptimalTradeSizer::size_dex_amm_cycle(
            &hops,
            1500.0, // €1,500 internal capital
            1500.0, // no flashloan needed
            0.0005,
            0.02,   // €0.02 Base L2 gas
            0.10,   // €0.10 min net profit
        );

        assert!(dex_res.executable);
        assert!(
            dex_res.executable_input_base > 20.0 && dex_res.executable_input_base < 200.0,
            "Quadratic AMM sizer must cap input well below €1,500 on shallow pools, got {}",
            dex_res.executable_input_base
        );
        assert!(dex_res.net_profit_base > 0.15);
    }
}
