use crate::registry::L2MainnetRegistry;

/// Canonical Multicall3 interface on Base Mainnet (`0xca11bde05977b3631167028862be2a173976ca11`).
/// 
/// Allows atomic batch querying of 20+ liquidity pools in a single transaction
/// with a 100% mathematical guarantee of zero block drift (all states from the same block).
pub struct Multicall3Builder;

impl Multicall3Builder {
    /// Multicall3 contract address on Base Mainnet (Chain ID 8453).
    pub const ADDRESS: [u8; 20] = L2MainnetRegistry::BASE_MULTICALL3;

    /// Function selector for `aggregate3((address,bool,bytes)[])`: `0x82ad56cb`
    pub const SELECTOR_AGGREGATE3: [u8; 4] = [0x82, 0xad, 0x56, 0xcb];

    /// Function selector for `getBlockNumber()`: `0x42cbb15c`
    pub const SELECTOR_GET_BLOCK_NUMBER: [u8; 4] = [0x42, 0xcb, 0xb1, 0x5c];

    /// Function selector for `getEthBalance(address)`: `0x4d2301cc`
    pub const SELECTOR_GET_ETH_BALANCE: [u8; 4] = [0x4d, 0x23, 0x01, 0xcc];

    /// Selector for Uniswap V3 `slot0()`: `0x3850c7bd`
    pub const SELECTOR_SLOT0: [u8; 4] = [0x38, 0x50, 0xc7, 0xbd];

    /// Selector for Aerodrome V2 `getReserves()`: `0x0902f1ac`
    pub const SELECTOR_GET_RESERVES: [u8; 4] = [0x09, 0x02, 0xf1, 0xac];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multicall3_selectors_and_address() {
        assert_eq!(
            Multicall3Builder::ADDRESS,
            L2MainnetRegistry::BASE_MULTICALL3
        );
        assert_eq!(Multicall3Builder::SELECTOR_AGGREGATE3, [0x82, 0xad, 0x56, 0xcb]);
        assert_eq!(Multicall3Builder::SELECTOR_GET_BLOCK_NUMBER, [0x42, 0xcb, 0xb1, 0x5c]);
    }
}
