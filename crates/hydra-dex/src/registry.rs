use hydra_core::CurrencyId;

/// Verified Production Mainnet Contract Registry for Node C (Frankfurt: Base 8453 & Arbitrum 42161).
pub struct L2MainnetRegistry;

impl L2MainnetRegistry {
    // =========================================================================
    // Base Mainnet (Chain ID: 8453)
    // =========================================================================
    pub const BASE_CHAIN_ID: u64 = 8453;

    pub const BASE_AAVE_V3_POOL: [u8; 20] =
        hex_20(*b"A238Dd80C259a72e81d7e4664a9801593F98d1c5");
    pub const BASE_BALANCER_VAULT: [u8; 20] =
        hex_20(*b"BA12222222228d8Ba445958a75a0704d566BF2C8");
    pub const BASE_AERODROME_ROUTER: [u8; 20] =
        hex_20(*b"cF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43");
    pub const BASE_UNIV3_ROUTER02: [u8; 20] =
        hex_20(*b"2626664c2603336E57B271c5C0b26F421741e481");
    pub const BASE_PANCAKESWAP_ROUTER: [u8; 20] =
        hex_20(*b"1b81D678ffb9C0263b24A97847620C99d213eB14");

    pub const BASE_USDC: [u8; 20] =
        hex_20(*b"833589fCD6eDb6E08f4c7C32D4f71b54bdA02913");
    pub const BASE_EURC: [u8; 20] =
        hex_20(*b"60a3E35Cc302bFA44Cb288Bc5a4F316Fdb1adb42");
    pub const BASE_WETH: [u8; 20] =
        hex_20(*b"4200000000000000000000000000000000000006");
    pub const BASE_DAI: [u8; 20] =
        hex_20(*b"50c5725949A6F0c72E6C4a641F24049A917DB0Cb");
    pub const BASE_AERO: [u8; 20] =
        hex_20(*b"940181a94A35A4569E4529A3CDfB74e38FD98631");
    pub const BASE_VIRTUAL: [u8; 20] =
        hex_20(*b"0b3e328455c4059EEb9e3f84b5543F74E24e7E1b");
    pub const BASE_CBBTC: [u8; 20] =
        hex_20(*b"cbB7C0000aB88B473b1f5aFd9ef808440eed33Bf");
    pub const BASE_BRETT: [u8; 20] =
        hex_20(*b"532f27101965dd16442e59d40670faf5ebb142e4");
    pub const BASE_DEGEN: [u8; 20] =
        hex_20(*b"4ed4e862860bed51a9570b96d89af5e1b0efefed");
    pub const BASE_TOSHI: [u8; 20] =
        hex_20(*b"AC1Bd2486aAf3B5C0fc3Fd868558b082a531B2B4");
    pub const BASE_USDBC: [u8; 20] =
        hex_20(*b"d9aAEc86B65D86f6A7B5B1b0c42FFA531710b6CA");
    pub const BASE_CBETH: [u8; 20] =
        hex_20(*b"2Ae3F1Ec7F1F5012CFEab0185bfc7aa3cf0DEc22");
    pub const BASE_MULTICALL3: [u8; 20] =
        hex_20(*b"ca11bde05977b3631167028862be2a173976ca11");

    // =========================================================================
    // Arbitrum One (Chain ID: 42161)
    // =========================================================================
    pub const ARB_CHAIN_ID: u64 = 42161;

    pub const ARB_AAVE_V3_POOL: [u8; 20] =
        hex_20(*b"794a61358D6845594F94dc1DB02A252b5b4814aD");
    pub const ARB_BALANCER_VAULT: [u8; 20] =
        hex_20(*b"BA12222222228d8Ba445958a75a0704d566BF2C8");
    pub const ARB_UNIV3_ROUTER: [u8; 20] =
        hex_20(*b"E592427A0AEce92De3Edee1F18E0157C05861564");

    pub const ARB_USDC: [u8; 20] =
        hex_20(*b"af88d065e77c8cC2239327C5EDb3A432268e5831");
    pub const ARB_USDT: [u8; 20] =
        hex_20(*b"Fd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9");
    pub const ARB_WETH: [u8; 20] =
        hex_20(*b"82aF49447D8a07e3bd95BD0d56f35241523fBab1");
    pub const ARB_ARB: [u8; 20] =
        hex_20(*b"912CE59144191C1204E64559FE8253a0e49E6548");

    /// Maps a `CurrencyId` to its canonical 20-byte token address on Base Mainnet (8453).
    #[inline(always)]
    pub const fn base_token_address(currency: CurrencyId) -> [u8; 20] {
        match currency {
            CurrencyId::EUR | CurrencyId::EURC => Self::BASE_EURC,
            CurrencyId::USD | CurrencyId::USDC | CurrencyId::USDT => Self::BASE_USDC,
            CurrencyId::ETH | CurrencyId::WETH => Self::BASE_WETH,
            CurrencyId::DAI => Self::BASE_DAI,
            CurrencyId::AERO => Self::BASE_AERO,
            CurrencyId::VIRTUAL => Self::BASE_VIRTUAL,
            CurrencyId::CBBTC | CurrencyId::BTC | CurrencyId::WBTC => Self::BASE_CBBTC,
            CurrencyId::BRETT => Self::BASE_BRETT,
            CurrencyId::DEGEN => Self::BASE_DEGEN,
            CurrencyId::TOSHI => Self::BASE_TOSHI,
            CurrencyId::USDBC => Self::BASE_USDBC,
            CurrencyId::CBETH => Self::BASE_CBETH,
            _ => Self::BASE_USDC,
        }
    }
}

#[inline(always)]
const fn hex_nibble(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => 0,
    }
}

#[inline(always)]
const fn hex_20(chars: [u8; 40]) -> [u8; 20] {
    let mut out = [0u8; 20];
    let mut i = 0;
    while i < 20 {
        let hi = hex_nibble(chars[i * 2]);
        let lo = hex_nibble(chars[i * 2 + 1]);
        out[i] = (hi << 4) | lo;
        i += 1;
    }
    out
}
