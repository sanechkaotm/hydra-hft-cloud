use hydra_core::{CurrencyId, VenueId};

/// Pre-allocated pool state entry synchronized from Base / Arbitrum Sequencer feeds.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct OnChainPoolState {
    pub pool_address: [u8; 20],
    pub venue: VenueId,
    pub token0: CurrencyId,
    pub token1: CurrencyId,
    pub decimals0: u8,
    pub decimals1: u8,
    /// Normalized reserve0 (or virtual reserve0 for UniV3).
    pub reserve0: f64,
    /// Normalized reserve1 (or virtual reserve1 for UniV3).
    pub reserve1: f64,
    /// Raw Q64.96 `sqrtPriceX96` for UniV3 pools (`0` for UniV2/Curve).
    pub sqrt_price_x96: u128,
    /// Raw active tick liquidity `L` for UniV3 pools.
    pub liquidity_l: u128,
    pub fee_pips: u32,
    pub block_number: u64,
    pub last_exchange_ts_ns: u64,
    pub local_recv_ts_ns: u64,
}

impl Default for OnChainPoolState {
    #[inline(always)]
    fn default() -> Self {
        Self {
            pool_address: [0u8; 20],
            venue: VenueId::BaseUniV3,
            token0: CurrencyId::EURC,
            token1: CurrencyId::USDC,
            decimals0: 6,
            decimals1: 6,
            reserve0: 0.0,
            reserve1: 0.0,
            sqrt_price_x96: 0,
            liquidity_l: 0,
            fee_pips: 500,
            block_number: 0,
            last_exchange_ts_ns: 0,
            local_recv_ts_ns: 0,
        }
    }
}

pub const MAX_TRACKED_POOLS: usize = 64;

/// L2 EVM WSS Block / Log / Sequencer Feed State Synchronizer (`hydra-dex::state_sync`).
pub struct L2StateSynchronizer {
    pub pools: [OnChainPoolState; MAX_TRACKED_POOLS],
    pub pool_count: usize,
    pub latest_l2_block: u64,
    pub latest_base_fee_gwei: f64,
}

impl Default for L2StateSynchronizer {
    fn default() -> Self {
        Self::new()
    }
}

impl L2StateSynchronizer {
    pub fn new() -> Self {
        Self {
            pools: [OnChainPoolState::default(); MAX_TRACKED_POOLS],
            pool_count: 0,
            latest_l2_block: 0,
            latest_base_fee_gwei: 0.005, // 0.005 gwei Base L2 baseline
        }
    }

    /// Registers an L2 DEX pool for zero-allocation hot-path tracking.
    #[inline(always)]
    pub fn register_pool(&mut self, state: OnChainPoolState) -> Option<usize> {
        if self.pool_count >= MAX_TRACKED_POOLS {
            return None;
        }
        let idx = self.pool_count;
        self.pools[idx] = state;
        self.pool_count += 1;
        Some(idx)
    }

    /// Decodes a 64-byte ABI `Sync(uint112 reserve0, uint112 reserve1)` log data payload
    /// from Uniswap V2 / Aerodrome pools and updates the registered pool in-place.
    #[inline(always)]
    pub fn apply_univ2_sync_log(
        &mut self,
        pool_idx: usize,
        log_data_64: &[u8; 64],
        block_number: u64,
        exchange_ts_ns: u64,
        local_recv_ts_ns: u64,
    ) -> bool {
        if pool_idx >= self.pool_count {
            return false;
        }
        let p = &mut self.pools[pool_idx];

        // uint112 sits in the lower 16 bytes (bytes 16..32 and 48..64) of each 32-byte ABI word
        let r0_raw = u128::from_be_bytes(log_data_64[16..32].try_into().unwrap());
        let r1_raw = u128::from_be_bytes(log_data_64[48..64].try_into().unwrap());

        let scale0 = 10.0f64.powi(p.decimals0 as i32);
        let scale1 = 10.0f64.powi(p.decimals1 as i32);

        p.reserve0 = (r0_raw as f64) / scale0;
        p.reserve1 = (r1_raw as f64) / scale1;
        p.block_number = block_number;
        p.last_exchange_ts_ns = exchange_ts_ns;
        p.local_recv_ts_ns = local_recv_ts_ns;
        self.latest_l2_block = self.latest_l2_block.max(block_number);
        true
    }

    /// Decodes a 160-byte ABI `Swap(int256 amount0, int256 amount1, uint160 sqrtPriceX96, uint128 liquidity, int24 tick)`
    /// log data payload from Uniswap V3 / Slipstream and updates virtual reserves in-place.
    #[inline(always)]
    pub fn apply_univ3_swap_log(
        &mut self,
        pool_idx: usize,
        log_data_160: &[u8; 160],
        block_number: u64,
        exchange_ts_ns: u64,
        local_recv_ts_ns: u64,
    ) -> bool {
        if pool_idx >= self.pool_count {
            return false;
        }
        let p = &mut self.pools[pool_idx];

        // Word 2 (bytes 64..96): uint160 sqrtPriceX96 ( take lower 16 bytes 80..96 for u128 )
        let sqrt_price_x96 = u128::from_be_bytes(log_data_160[80..96].try_into().unwrap());
        // Word 3 (bytes 96..128): uint128 liquidity ( lower 16 bytes 112..128 )
        let liquidity_l = u128::from_be_bytes(log_data_160[112..128].try_into().unwrap());

        p.sqrt_price_x96 = sqrt_price_x96;
        p.liquidity_l = liquidity_l;

        let sqrt_p = crate::amm_math::UniV3Math::sqrt_price_from_x96(
            sqrt_price_x96,
            p.decimals0,
            p.decimals1,
        );
        let avg_dec = ((p.decimals0 as i32) + (p.decimals1 as i32)) / 2;
        let norm_l = (liquidity_l as f64) / 10.0f64.powi(avg_dec);

        if sqrt_p > 0.0 {
            p.reserve0 = norm_l / sqrt_p;
            p.reserve1 = norm_l * sqrt_p;
        }

        p.block_number = block_number;
        p.last_exchange_ts_ns = exchange_ts_ns;
        p.local_recv_ts_ns = local_recv_ts_ns;
        self.latest_l2_block = self.latest_l2_block.max(block_number);
        true
    }
}
