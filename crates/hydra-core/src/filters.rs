use crate::types::CurrencyId;

/// Exchange symbol trading rules (`LOT_SIZE`, `PRICE_FILTER`, `MIN_NOTIONAL`) for Binance and Kraken.
/// Prevents `Filter failure: LOT_SIZE` and `MIN_NOTIONAL` rejections on multi-leg IOC cycles
/// and accounts for lot-truncation dust in profit calculations.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct SymbolFilterSpec {
    pub base: CurrencyId,
    pub quote: CurrencyId,
    /// Quantity step size (`LOT_SIZE.stepSize`, e.g., `0.00001` for BTC, `0.1` for TRY/BRL, `0.01` for EUR/USDT).
    pub step_size: f64,
    /// Price tick size (`PRICE_FILTER.tickSize`, e.g., `0.0001` for FX, `0.01` for BTC).
    pub tick_size: f64,
    /// Minimum order quantity in `base` units.
    pub min_qty: f64,
    /// Minimum order notional (`qty * price`) in `quote` units (e.g., `5.0` USDT or `5.0` EUR).
    pub min_notional: f64,
}

impl SymbolFilterSpec {
    /// Returns the production exchange filter specification for a `(base, quote)` pair.
    #[inline(always)]
    pub const fn for_pair(base: CurrencyId, quote: CurrencyId) -> Self {
        let (step_size, min_qty) = match base {
            CurrencyId::BTC | CurrencyId::WBTC => (0.00001, 0.0001),
            CurrencyId::ETH | CurrencyId::WETH => (0.0001, 0.001),
            CurrencyId::BNB | CurrencyId::SOL => (0.001, 0.01),
            CurrencyId::TRY | CurrencyId::BRL | CurrencyId::MXN | CurrencyId::UAH | CurrencyId::JPY => {
                (0.1, 1.0)
            }
            _ => (0.01, 1.0),
        };

        let tick_size = match (base, quote) {
            (CurrencyId::BTC | CurrencyId::ETH, _) => 0.01,
            (_, CurrencyId::JPY | CurrencyId::TRY | CurrencyId::BRL) => 0.001,
            _ => 0.0001,
        };

        let min_notional = match quote {
            CurrencyId::TRY => 150.0,
            CurrencyId::BRL => 25.0,
            CurrencyId::JPY => 750.0,
            _ => 5.0,
        };

        Self {
            base,
            quote,
            step_size,
            tick_size,
            min_qty,
            min_notional,
        }
    }

    /// Floors `raw_qty` to an exact integer multiple of `step_size` so an IOC order never exceeds
    /// the wallet balance received from the preceding leg.
    #[inline(always)]
    pub fn quantize_qty_floor(&self, raw_qty: f64) -> f64 {
        if raw_qty <= 0.0 || self.step_size <= 0.0 {
            return 0.0;
        }
        let steps = (raw_qty / self.step_size + 1e-9).floor();
        steps * self.step_size
    }

    /// Quantizes `raw_price` to an exact multiple of `tick_size`.
    #[inline(always)]
    pub fn quantize_price(&self, raw_price: f64) -> f64 {
        if raw_price <= 0.0 || self.tick_size <= 0.0 {
            return 0.0;
        }
        let ticks = (raw_price / self.tick_size).round();
        ticks * self.tick_size
    }

    /// Validates that a quantized `(qty, price)` pair satisfies both `min_qty` and `min_notional`.
    #[inline(always)]
    pub fn is_valid_order(&self, qty: f64, price: f64) -> bool {
        qty >= self.min_qty && (qty * price) >= self.min_notional
    }
}
