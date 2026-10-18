pub mod binance;
pub mod kraken;
pub mod orderbook;
pub mod unwind;

pub use binance::{BinanceConnector, BinanceDepthEvent, OrderSide, hmac_sha256_hex};
pub use kraken::{KrakenBookV2Event, KrakenConnector};
pub use orderbook::{LocalL2OrderBook, SequenceError};
pub use unwind::{EmergencyUnwindMachine, EmergencyUnwindOrder, LegFillReport, UnwindState};

#[cfg(test)]
mod tests {
    use super::*;
    use hydra_core::{CurrencyId, PriceLevel, VenueId};

    #[test]
    fn test_l2_orderbook_sequence_gap_detection() {
        let mut book = LocalL2OrderBook::new(VenueId::BinanceSpot, CurrencyId::EUR, CurrencyId::USDT);
        book.apply_snapshot(
            &[PriceLevel::new(1.0895, 5000.0)],
            &[PriceLevel::new(1.0897, 4500.0)],
            1000,
            1_700_000_000_000_000_000,
            1_700_000_000_001_000_000,
        );

        // Contiguous diff update (1001..=1002) succeeds
        assert!(book
            .apply_diff_update(
                1001,
                1002,
                &[PriceLevel::new(1.0896, 3000.0)],
                &[],
                1_700_000_000_005_000_000,
                1_700_000_000_006_000_000,
            )
            .is_ok());
        assert_eq!(book.snapshot.best_bid().unwrap().price, 1.0896);

        // Sequence gap (expected 1003, received first_update_id = 1008) must return Err(SequenceGapDetected)
        let gap_res = book.apply_diff_update(
            1008,
            1010,
            &[PriceLevel::new(1.0898, 1000.0)],
            &[],
            1_700_000_000_010_000_000,
            1_700_000_000_011_000_000,
        );
        assert_eq!(
            gap_res,
            Err(SequenceError::SequenceGapDetected {
                expected: 1003,
                received_first: 1008
            })
        );
        assert!(!book.synchronized);
    }

    #[test]
    fn test_cex_emergency_unwind_on_partial_fill() {
        let mut unwind = EmergencyUnwindMachine::new(CurrencyId::EUR, 20.0); // 20 bps slippage allowance

        // Scenario: 3-leg cycle EUR -> USDT -> TRY -> EUR
        // Leg 0 (EUR -> USDT) fills 100%: 500 EUR -> 545 USDT
        // Leg 1 (USDT -> TRY) partially fills 40%: consumes 218 USDT -> receives 7,521 TRY, leaving 327 USDT stranded!
        let reports = [
            LegFillReport {
                leg_index: 0,
                venue: VenueId::BinanceSpot,
                from_currency: CurrencyId::EUR,
                to_currency: CurrencyId::USDT,
                requested_input: 500.0,
                actual_input_consumed: 500.0,
                actual_output_received: 545.0,
                exec_latency_us: 1100,
            },
            LegFillReport {
                leg_index: 1,
                venue: VenueId::BinanceSpot,
                from_currency: CurrencyId::USDT,
                to_currency: CurrencyId::TRY,
                requested_input: 545.0,
                actual_input_consumed: 218.0, // Only 40% filled before IOC expired!
                actual_output_received: 7521.0,
                exec_latency_us: 1450,
            },
        ];

        // Reference top-of-book bids back to EUR: USDT/EUR = 0.9174, TRY/EUR = 0.0266
        let unwind_orders = unwind.evaluate_cycle_fills(&reports, &[0.9174, 0.0266]);

        assert_eq!(unwind.state, UnwindState::EmergencyUnwound { broken_leg: 1 });
        assert_eq!(unwind_orders.len(), 2, "Must unwind both stranded USDT and partial TRY");

        // Order 0 liquidates 327.0 stranded USDT -> EUR
        assert_eq!(unwind_orders[0].stranded_currency, CurrencyId::USDT);
        assert_eq!(unwind_orders[0].target_base_currency, CurrencyId::EUR);
        assert!((unwind_orders[0].quantity_to_liquidate - 327.0).abs() < 1e-6);
        assert!(unwind_orders[0].decision_latency_ns < 10_000_000); // Well below 10ms (< 1us)

        // Order 1 liquidates 7521.0 partial TRY -> EUR
        assert_eq!(unwind_orders[1].stranded_currency, CurrencyId::TRY);
        assert_eq!(unwind_orders[1].target_base_currency, CurrencyId::EUR);
        assert!((unwind_orders[1].quantity_to_liquidate - 7521.0).abs() < 1e-6);
    }

    #[test]
    fn test_binance_and_kraken_zero_alloc_parsers_and_ioc_builders() {
        let raw_binance = br#"{"e":"depthUpdate","E":1700000000123,"s":"EURUSDT","U":1001,"u":1005,"b":[["1.0895","5000.0"]],"a":[["1.0897","4200.0"]]}"#;
        let ev = BinanceConnector::parse_depth_update(raw_binance).expect("Valid Binance depthUpdate");
        assert_eq!(ev.first_update_id, 1001);
        assert_eq!(ev.final_update_id, 1005);
        assert_eq!(ev.bid_count, 1);
        assert!((ev.bids[0].price - 1.0895).abs() < 1e-9);

        let binance = BinanceConnector::new("TEST_KEY", "TEST_SECRET");
        let mut out_buf = [0u8; 1024];
        let n = binance.build_ws_ioc_order(
            77,
            "EURUSDT",
            OrderSide::Buy,
            250.0,
            1.0897,
            1700000000125,
            &mut out_buf,
        );
        let json_str = std::str::from_utf8(&out_buf[..n]).unwrap();
        assert!(json_str.contains("\"timeInForce\":\"IOC\""));
        assert!(json_str.contains("\"signature\":\""));

        let raw_kraken = br#"{"channel":"book","type":"update","data":[{"symbol":"EUR/USD","bids":[{"price":1.0892,"qty":3100.0}],"asks":[{"price":1.0894,"qty":2900.0}],"checksum":998877}]}"#;
        let kev = KrakenConnector::parse_book_v2(raw_kraken).expect("Valid Kraken v2 book");
        assert_eq!(kev.bid_count, 1);
        assert!((kev.bids[0].price - 1.0892).abs() < 1e-9);
        assert_eq!(kev.checksum, 998877);
    }
}
