pub mod fees;
pub mod filters;
pub mod spsc;
pub mod types;

pub use fees::FeeEngine;
pub use filters::SymbolFilterSpec;
pub use spsc::SpscQueue;
pub use types::{
    AssetPairId, CurrencyId, I64F64, LOG_WEIGHT_SCALE, LOG_WEIGHT_SCALE_I64, MAX_BOOK_LEVELS,
    MAX_CURRENCIES, MAX_EDGES, NodeRole, OrderBookSnapshot, PriceLevel, StackVec, VenueId,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_point_q64_64_precision() {
        let a = I64F64::from_f64(1.0925);
        let b = I64F64::from_f64(34.50);
        let prod = a * b;
        let expected = 1.0925 * 34.50;
        assert!((prod.to_f64() - expected).abs() < 1e-12);
    }

    #[test]
    fn test_lock_free_spsc_ring_buffer() {
        let q: SpscQueue<u64, 8> = SpscQueue::new();
        assert!(q.is_empty());

        for i in 0..8 {
            assert_eq!(q.try_push(i * 10), Ok(()));
        }
        assert_eq!(q.try_push(999), Err(999));
        assert_eq!(q.len(), 8);

        for i in 0..8 {
            assert_eq!(q.try_pop(), Some(i * 10));
        }
        assert_eq!(q.try_pop(), None);
        assert!(q.is_empty());
    }

    #[test]
    fn test_tier_fee_promo_bridges_and_lot_size_quantizer() {
        let fees = FeeEngine::default();
        // Standard Binance VIP 0 with BNB discount = 7.5 bps = 0.00075
        let binance_f = fees.venue_fee_fraction(VenueId::BinanceSpot, None);
        assert!((binance_f - 0.00075).abs() < 1e-10);

        // Zero-fee promo bridge EUR -> USDC on Binance = 0.0 bps
        let promo_f = fees.pair_fee_fraction(
            VenueId::BinanceSpot,
            CurrencyId::EUR,
            CurrencyId::USDC,
            None,
        );
        assert_eq!(promo_f, 0.0);

        // Lot-size floor quantizer prevents "Filter failure: LOT_SIZE" rejections
        let filter = SymbolFilterSpec::for_pair(CurrencyId::BTC, CurrencyId::USDT);
        let q_btc = filter.quantize_qty_floor(0.01458932);
        assert!((q_btc - 0.01458).abs() < 1e-10);
        assert!(filter.is_valid_order(q_btc, 68500.0));
    }
}
