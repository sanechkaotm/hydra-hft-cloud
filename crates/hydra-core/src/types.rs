use std::fmt;
use std::ops::{Add, Div, Mul, Sub};

/// Maximum number of currencies supported in the static adjacency graph.
pub const MAX_CURRENCIES: usize = 128;

/// Maximum number of directed edges in the static adjacency graph.
pub const MAX_EDGES: usize = 2048;

/// Maximum L2 orderbook depth levels maintained on the hot path.
pub const MAX_BOOK_LEVELS: usize = 5;

/// Scale factor for logarithmic edge weights: 10^12 (`1_000_000_000_000`).
pub const LOG_WEIGHT_SCALE: f64 = 1_000_000_000_000.0;
pub const LOG_WEIGHT_SCALE_I64: i64 = 1_000_000_000_000;

/// Pre-indexed Currencies & Tokens (`0..63`) as defined in Section 3.1.
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CurrencyId {
    // --- Fiat & Stablecoins (0..20) ---
    EUR = 0,
    USD = 1,
    GBP = 2,
    CHF = 3,
    TRY = 4,
    BRL = 5,
    PLN = 6,
    UAH = 7,
    CZK = 8,
    MXN = 9,
    JPY = 10,
    CAD = 11,
    AUD = 12,
    USDT = 13,
    USDC = 14,
    EURC = 15,
    DAI = 16,
    BRZ = 17,
    TRYB = 18,
    VCHF = 19,
    TGBP = 20,

    // --- Liquid Crypto Bridges (21..28) ---
    BTC = 21,
    ETH = 22,
    WETH = 23,
    WBTC = 24,
    SOL = 25,
    BNB = 26,
    ARB = 27,
    AERO = 28,
    VIRTUAL = 29,
    CBBTC = 30,
    BRETT = 31,
    DEGEN = 32,
}

impl CurrencyId {
    pub const ALL: [CurrencyId; 33] = [
        Self::EUR,
        Self::USD,
        Self::GBP,
        Self::CHF,
        Self::TRY,
        Self::BRL,
        Self::PLN,
        Self::UAH,
        Self::CZK,
        Self::MXN,
        Self::JPY,
        Self::CAD,
        Self::AUD,
        Self::USDT,
        Self::USDC,
        Self::EURC,
        Self::DAI,
        Self::BRZ,
        Self::TRYB,
        Self::VCHF,
        Self::TGBP,
        Self::BTC,
        Self::ETH,
        Self::WETH,
        Self::WBTC,
        Self::SOL,
        Self::BNB,
        Self::ARB,
        Self::AERO,
        Self::VIRTUAL,
        Self::CBBTC,
        Self::BRETT,
        Self::DEGEN,
    ];

    /// Base capital anchor currencies (`EUR`, `USDC`, `USDT`, `EURC`, `WETH`).
    pub const BASE_ANCHORS: [CurrencyId; 5] = [Self::EUR, Self::USDC, Self::USDT, Self::EURC, Self::WETH];

    #[inline(always)]
    pub const fn as_idx(self) -> usize {
        self as usize
    }

    #[inline(always)]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    #[inline(always)]
    pub const fn from_u8(v: u8) -> Option<Self> {
        if (v as usize) < Self::ALL.len() {
            Some(Self::ALL[v as usize])
        } else {
            None
        }
    }

    #[inline(always)]
    pub const fn is_base_capital(self) -> bool {
        matches!(self, Self::EUR | Self::USDC | Self::USDT | Self::EURC)
    }

    #[inline(always)]
    pub const fn symbol(self) -> &'static str {
        match self {
            Self::EUR => "EUR",
            Self::USD => "USD",
            Self::GBP => "GBP",
            Self::CHF => "CHF",
            Self::TRY => "TRY",
            Self::BRL => "BRL",
            Self::PLN => "PLN",
            Self::UAH => "UAH",
            Self::CZK => "CZK",
            Self::MXN => "MXN",
            Self::JPY => "JPY",
            Self::CAD => "CAD",
            Self::AUD => "AUD",
            Self::USDT => "USDT",
            Self::USDC => "USDC",
            Self::EURC => "EURC",
            Self::DAI => "DAI",
            Self::BRZ => "BRZ",
            Self::TRYB => "TRYB",
            Self::VCHF => "VCHF",
            Self::TGBP => "tGBP",
            Self::BTC => "BTC",
            Self::ETH => "ETH",
            Self::WETH => "WETH",
            Self::WBTC => "WBTC",
            Self::SOL => "SOL",
            Self::BNB => "BNB",
            Self::ARB => "ARB",
            Self::AERO => "AERO",
            Self::VIRTUAL => "VIRTUAL",
            Self::CBBTC => "cbBTC",
            Self::BRETT => "BRETT",
            Self::DEGEN => "DEGEN",
        }
    }

    pub fn from_symbol(sym: &str) -> Option<Self> {
        match sym {
            "EUR" | "ZEUR" => Some(Self::EUR),
            "USD" | "ZUSD" => Some(Self::USD),
            "GBP" | "ZGBP" => Some(Self::GBP),
            "CHF" => Some(Self::CHF),
            "TRY" => Some(Self::TRY),
            "BRL" => Some(Self::BRL),
            "PLN" => Some(Self::PLN),
            "UAH" => Some(Self::UAH),
            "CZK" => Some(Self::CZK),
            "MXN" => Some(Self::MXN),
            "JPY" | "ZJPY" => Some(Self::JPY),
            "CAD" | "ZCAD" => Some(Self::CAD),
            "AUD" | "ZAUD" => Some(Self::AUD),
            "USDT" => Some(Self::USDT),
            "USDC" => Some(Self::USDC),
            "EURC" => Some(Self::EURC),
            "DAI" => Some(Self::DAI),
            "BRZ" => Some(Self::BRZ),
            "TRYB" => Some(Self::TRYB),
            "VCHF" => Some(Self::VCHF),
            "tGBP" | "TGBP" => Some(Self::TGBP),
            "BTC" | "XXBT" | "XBT" => Some(Self::BTC),
            "ETH" | "XETH" => Some(Self::ETH),
            "WETH" => Some(Self::WETH),
            "WBTC" => Some(Self::WBTC),
            "SOL" => Some(Self::SOL),
            "BNB" => Some(Self::BNB),
            "ARB" => Some(Self::ARB),
            "AERO" => Some(Self::AERO),
            "VIRTUAL" => Some(Self::VIRTUAL),
            "CBBTC" | "cbBTC" | "CBBtc" => Some(Self::CBBTC),
            "BRETT" | "Brett" => Some(Self::BRETT),
            "DEGEN" | "Degen" => Some(Self::DEGEN),
            _ => None,
        }
    }
}

impl fmt::Display for CurrencyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.symbol())
    }
}

/// Pre-mapped Asset Pair ID (`u16`) for the 64-byte UDP `ImpulseFrame`.
/// High byte = base `CurrencyId`, Low byte = quote `CurrencyId`.
#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct AssetPairId(pub u16);

impl AssetPairId {
    pub const USDT_TRY: Self = Self::new(CurrencyId::USDT, CurrencyId::TRY);
    pub const EUR_USD: Self = Self::new(CurrencyId::EUR, CurrencyId::USD);
    pub const BTC_USDT: Self = Self::new(CurrencyId::BTC, CurrencyId::USDT);
    pub const EUR_USDT: Self = Self::new(CurrencyId::EUR, CurrencyId::USDT);
    pub const USDT_BRL: Self = Self::new(CurrencyId::USDT, CurrencyId::BRL);
    pub const ETH_USDC: Self = Self::new(CurrencyId::ETH, CurrencyId::USDC);
    pub const BTC_EUR: Self = Self::new(CurrencyId::BTC, CurrencyId::EUR);

    #[inline(always)]
    pub const fn new(base: CurrencyId, quote: CurrencyId) -> Self {
        Self(((base as u16) << 8) | (quote as u16))
    }

    #[inline(always)]
    pub const fn base(self) -> Option<CurrencyId> {
        CurrencyId::from_u8((self.0 >> 8) as u8)
    }

    #[inline(always)]
    pub const fn quote(self) -> Option<CurrencyId> {
        CurrencyId::from_u8((self.0 & 0xFF) as u8)
    }
}

/// Distributed Node Role in the Tri-Node topology.
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum NodeRole {
    Tokyo = 1,
    London = 2,
    Frankfurt = 3,
}

impl NodeRole {
    #[inline(always)]
    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::Tokyo),
            2 => Some(Self::London),
            3 => Some(Self::Frankfurt),
            _ => None,
        }
    }

    #[inline(always)]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Tokyo => "TOKYO (ap-northeast-1)",
            Self::London => "LONDON (eu-west-2)",
            Self::Frankfurt => "FRANKFURT (eu-central-1)",
        }
    }
}

/// Execution Venue Identifier across CEX and L2 DEXs.
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum VenueId {
    BinanceSpot = 0,
    BinanceMargin = 1,
    KrakenSpot = 2,
    BaseUniV3 = 3,
    BaseAerodromeVolatile = 4,
    BaseAerodromeStable = 5,
    BaseCurve = 6,
    ArbitrumUniV3 = 7,
    ArbitrumCurve = 8,
    ArbitrumCamelotV2 = 9,
    BaseSushiV3 = 10,
}

impl VenueId {
    #[inline(always)]
    pub const fn is_dex(self) -> bool {
        (self as u8) >= 3
    }

    #[inline(always)]
    pub const fn name(self) -> &'static str {
        match self {
            Self::BinanceSpot => "BinanceSpot",
            Self::BinanceMargin => "BinanceMargin",
            Self::KrakenSpot => "KrakenSpot",
            Self::BaseUniV3 => "BaseUniV3",
            Self::BaseAerodromeVolatile => "BaseAeroVol",
            Self::BaseAerodromeStable => "BaseAeroStable",
            Self::BaseCurve => "BaseCurve",
            Self::ArbitrumUniV3 => "ArbUniV3",
            Self::ArbitrumCurve => "ArbCurve",
            Self::ArbitrumCamelotV2 => "ArbCamelotV2",
            Self::BaseSushiV3 => "BaseSushiV3",
        }
    }
}

/// Signed Q64.64 Fixed-Point number (`I64F64`) backed by `i128` for deterministic
/// zero-allocation arithmetic without IEEE-754 drift.
#[repr(transparent)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct I64F64(pub i128);

impl I64F64 {
    pub const FRAC_BITS: u32 = 64;
    pub const ONE: Self = Self(1i128 << 64);
    pub const ZERO: Self = Self(0);

    #[inline(always)]
    pub const fn from_num(val: i64) -> Self {
        Self((val as i128) << 64)
    }

    #[inline(always)]
    pub const fn from_raw(raw: i128) -> Self {
        Self(raw)
    }

    #[inline(always)]
    pub fn from_f64(val: f64) -> Self {
        Self((val * (1u128 << 64) as f64) as i128)
    }

    #[inline(always)]
    pub fn to_f64(self) -> f64 {
        (self.0 as f64) / ((1u128 << 64) as f64)
    }

    #[inline(always)]
    pub const fn saturating_add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }

    #[inline(always)]
    pub const fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }

    /// Multiply two Q64.64 fixed-point values using 128-bit split arithmetic to prevent overflow.
    #[inline(always)]
    pub const fn mul_fixed(self, rhs: Self) -> Self {
        let a = self.0;
        let b = rhs.0;
        let sign = if (a < 0) ^ (b < 0) { -1i128 } else { 1i128 };
        let ua = a.unsigned_abs();
        let ub = b.unsigned_abs();

        let a_hi = ua >> 64;
        let a_lo = ua & 0xFFFF_FFFF_FFFF_FFFF;
        let b_hi = ub >> 64;
        let b_lo = ub & 0xFFFF_FFFF_FFFF_FFFF;

        let hi = (a_hi * b_hi) << 64;
        let mid = a_hi * b_lo + a_lo * b_hi;
        let lo = (a_lo * b_lo) >> 64;

        let mag = (hi + mid + lo) as i128;
        Self(mag * sign)
    }

    /// Divide two Q64.64 fixed-point values.
    #[inline(always)]
    pub fn div_fixed(self, rhs: Self) -> Self {
        if rhs.0 == 0 {
            return Self::ZERO;
        }
        let num = (self.0 as f64) / (rhs.0 as f64);
        Self::from_f64(num)
    }
}

impl Add for I64F64 {
    type Output = Self;
    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for I64F64 {
    type Output = Self;
    #[inline(always)]
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Mul for I64F64 {
    type Output = Self;
    #[inline(always)]
    fn mul(self, rhs: Self) -> Self::Output {
        self.mul_fixed(rhs)
    }
}

impl Div for I64F64 {
    type Output = Self;
    #[inline(always)]
    fn div(self, rhs: Self) -> Self::Output {
        self.div_fixed(rhs)
    }
}

/// Single L2 Orderbook price-quantity level.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct PriceLevel {
    /// Price in quote currency per 1 unit of base currency.
    pub price: f64,
    /// Available quantity in base currency units.
    pub quantity: f64,
}

impl PriceLevel {
    #[inline(always)]
    pub const fn new(price: f64, quantity: f64) -> Self {
        Self { price, quantity }
    }

    #[inline(always)]
    pub fn is_valid(&self) -> bool {
        self.price > 0.0 && self.quantity > 0.0
    }
}

/// Pre-allocated, zero-heap-allocation L2 OrderBook Snapshot (top 5 bids and asks).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct OrderBookSnapshot {
    pub venue: VenueId,
    pub base: CurrencyId,
    pub quote: CurrencyId,
    pub bids: [PriceLevel; MAX_BOOK_LEVELS],
    pub asks: [PriceLevel; MAX_BOOK_LEVELS],
    pub bid_depth: u8,
    pub ask_depth: u8,
    pub seq_no: u64,
    pub last_exchange_ts_ns: u64,
    pub local_recv_ts_ns: u64,
}

impl OrderBookSnapshot {
    #[inline(always)]
    pub const fn empty(venue: VenueId, base: CurrencyId, quote: CurrencyId) -> Self {
        Self {
            venue,
            base,
            quote,
            bids: [PriceLevel::new(0.0, 0.0); MAX_BOOK_LEVELS],
            asks: [PriceLevel::new(0.0, 0.0); MAX_BOOK_LEVELS],
            bid_depth: 0,
            ask_depth: 0,
            seq_no: 0,
            last_exchange_ts_ns: 0,
            local_recv_ts_ns: 0,
        }
    }

    #[inline(always)]
    pub fn best_bid(&self) -> Option<PriceLevel> {
        if self.bid_depth > 0 && self.bids[0].is_valid() {
            Some(self.bids[0])
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn best_ask(&self) -> Option<PriceLevel> {
        if self.ask_depth > 0 && self.asks[0].is_valid() {
            Some(self.asks[0])
        } else {
            None
        }
    }
}

/// Fixed-capacity stack-allocated vector (`ArrayVec` equivalent) for zero-allocation hot paths.
#[derive(Copy, Clone, Debug)]
pub struct StackVec<T: Copy + Default, const N: usize> {
    buf: [T; N],
    len: usize,
}

impl<T: Copy + Default, const N: usize> Default for StackVec<T, N> {
    #[inline(always)]
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Copy + Default, const N: usize> StackVec<T, N> {
    #[inline(always)]
    pub fn new() -> Self {
        Self {
            buf: [T::default(); N],
            len: 0,
        }
    }

    #[inline(always)]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline(always)]
    pub const fn capacity(&self) -> usize {
        N
    }

    #[inline(always)]
    pub fn clear(&mut self) {
        self.len = 0;
    }

    #[inline(always)]
    pub fn try_push(&mut self, item: T) -> bool {
        if self.len < N {
            self.buf[self.len] = item;
            self.len += 1;
            true
        } else {
            false
        }
    }

    #[inline(always)]
    pub fn as_slice(&self) -> &[T] {
        &self.buf[..self.len]
    }

    #[inline(always)]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.buf[..self.len]
    }
}

impl<T: Copy + Default, const N: usize> std::ops::Index<usize> for StackVec<T, N> {
    type Output = T;
    #[inline(always)]
    fn index(&self, index: usize) -> &Self::Output {
        &self.buf[index]
    }
}

impl<T: Copy + Default, const N: usize> std::ops::Deref for StackVec<T, N> {
    type Target = [T];
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T: Copy + Default, const N: usize> std::ops::DerefMut for StackVec<T, N> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

