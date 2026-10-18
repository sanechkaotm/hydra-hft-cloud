pub mod amm_math;
pub mod registry;
pub mod state_sync;
pub mod tx_builder;

pub use amm_math::{ConstantProductMath, CurveStableSwapMath, UniV3Math};
pub use registry::L2MainnetRegistry;
pub use state_sync::{L2StateSynchronizer, MAX_TRACKED_POOLS, OnChainPoolState};
pub use tx_builder::{AbiSwapStep, AtomicTxBuilder, Eip1559TxTemplate, keccak256};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_curve_stableswap_and_univ3_concentrated_liquidity_math() {
        // 1. Curve StableSwap 2-coin pool (1,000,000 EURC / 1,000,000 USDC, A = 100)
        // Swapping 10,000 units should have minimal slippage (< 2 bps before fee) compared to UniV2
        let dy_curve = CurveStableSwapMath::get_dy(1_000_000.0, 1_000_000.0, 10_000.0, 100.0, 0.0001);
        assert!(dy_curve > 9997.0 && dy_curve < 10_000.0);

        // 2. Uniswap V3 virtual reserve projection at 1.09 EURC/USDC spot
        let hop = UniV3Math::to_virtual_amm_hop(1.09f64.sqrt(), 500_000.0, true, 500);
        let out_v3 = hop.get_amount_out(1_000.0);
        assert!(out_v3 > 1085.0 && out_v3 < 1090.0);
    }

    #[test]
    fn test_keccak256_and_atomic_executor_abi_calldata_encoder() {
        // Verify Keccak-256 against standard Ethereum test vector: keccak256("transfer(address,uint256)") == a9059cbb...
        let transfer_hash = keccak256(b"transfer(address,uint256)");
        assert_eq!(&transfer_hash[0..4], &[0xa9, 0x05, 0x9c, 0xbb]);

        let builder = AtomicTxBuilder::new();
        let steps = [
            AbiSwapStep {
                dex_type: 2, // Curve
                pool_or_router: [0x11; 20],
                token_in: [0xAA; 20],
                token_out: [0xBB; 20],
                fee_tier: 0,
                curve_index_in: 0,
                curve_index_out: 1,
            },
            AbiSwapStep {
                dex_type: 1, // UniV3
                pool_or_router: [0x22; 20],
                token_in: [0xBB; 20],
                token_out: [0xCC; 20],
                fee_tier: 500,
                curve_index_in: 0,
                curve_index_out: 0,
            },
            AbiSwapStep {
                dex_type: 0, // UniV2 / Aerodrome
                pool_or_router: [0x33; 20],
                token_in: [0xCC; 20],
                token_out: [0xAA; 20],
                fee_tier: 0,
                curve_index_in: 0,
                curve_index_out: 0,
            },
        ];

        let mut calldata = [0u8; 1536];
        let len = builder.encode_execute_atomic_calldata(
            &[0xAA; 20],
            1_000_000_000,
            5_000_000,
            &steps,
            &mut calldata,
        );

        // 4-byte selector + 5 header words (160B) + 3 * 7 step words (672B) = 836 bytes
        assert_eq!(len, 4 + 160 + 3 * 224);
        assert_eq!(&calldata[0..4], &builder.selector_execute_atomic);
    }
}
