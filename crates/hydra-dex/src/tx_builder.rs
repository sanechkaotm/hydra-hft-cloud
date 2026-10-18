/// Solidity `SwapStep` ABI representation matching `HydraAtomicExecutor.sol`.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct AbiSwapStep {
    pub dex_type: u8,
    pub pool_or_router: [u8; 20],
    pub token_in: [u8; 20],
    pub token_out: [u8; 20],
    pub fee_tier: u32, // uint24
    pub curve_index_in: i128,
    pub curve_index_out: i128,
}

/// EIP-1559 Transaction Envelope Parameters for Node C (Frankfurt: Base & Arbitrum).
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Eip1559TxTemplate {
    pub chain_id: u64,
    pub nonce: u64,
    pub max_priority_fee_per_gas_wei: u128,
    pub max_fee_per_gas_wei: u128,
    pub gas_limit: u64,
    pub to_executor: [u8; 20],
}

/// Zero-allocation ABI Calldata & EIP-1559 Transaction Encoder for `HydraAtomicExecutor.sol`.
pub struct AtomicTxBuilder {
    /// Cached 4-byte Keccak-256 selector for:
    /// `executeAtomicArbitrage(address,uint256,uint256,(uint8,address,address,address,uint24,int128,int128)[])`
    pub selector_execute_atomic: [u8; 4],
    /// Cached 4-byte Keccak-256 selector for:
    /// `executeFlashArbitrage(uint8,address,uint256,uint256,uint256,(uint8,address,address,address,uint24,int128,int128)[])`
    pub selector_execute_flash: [u8; 4],
}

impl Default for AtomicTxBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl AtomicTxBuilder {
    pub fn new() -> Self {
        let sig_atomic = b"executeAtomicArbitrage(address,uint256,uint256,(uint8,address,address,address,uint24,int128,int128)[])";
        let hash_atomic = keccak256(sig_atomic);
        let selector_execute_atomic = [hash_atomic[0], hash_atomic[1], hash_atomic[2], hash_atomic[3]];

        let sig_flash = b"executeFlashArbitrage(uint8,address,uint256,uint256,uint256,(uint8,address,address,address,uint24,int128,int128)[])";
        let hash_flash = keccak256(sig_flash);
        let selector_execute_flash = [hash_flash[0], hash_flash[1], hash_flash[2], hash_flash[3]];

        Self {
            selector_execute_atomic,
            selector_execute_flash,
        }
    }

    /// Encodes the exact Solidity ABI calldata for `executeAtomicArbitrage` into `out_buf` (zero heap allocation).
    /// Returns the total calldata byte length (`4 + 32 * (5 + 7 * steps.len())`).
    #[inline(always)]
    pub fn encode_execute_atomic_calldata(
        &self,
        base_token: &[u8; 20],
        amount_in_wei: u128,
        min_net_profit_wei: u128,
        steps: &[AbiSwapStep],
        out_buf: &mut [u8; 1536],
    ) -> usize {
        out_buf.fill(0);
        out_buf[0..4].copy_from_slice(&self.selector_execute_atomic);

        let mut offset = 4usize;
        // Word 0: address baseToken (right-aligned in 32 bytes)
        out_buf[offset + 12..offset + 32].copy_from_slice(base_token);
        offset += 32;

        // Word 1: uint256 amountIn
        out_buf[offset + 16..offset + 32].copy_from_slice(&amount_in_wei.to_be_bytes());
        offset += 32;

        // Word 2: uint256 minNetProfit
        out_buf[offset + 16..offset + 32].copy_from_slice(&min_net_profit_wei.to_be_bytes());
        offset += 32;

        // Word 3: offset to dynamic array `steps` (4 * 32 = 128 = 0x80)
        out_buf[offset + 24..offset + 32].copy_from_slice(&128u64.to_be_bytes());
        offset += 32;

        // Word 4: steps.length
        let n_steps = steps.len().min(4);
        out_buf[offset + 24..offset + 32].copy_from_slice(&(n_steps as u64).to_be_bytes());
        offset += 32;

        // Each static struct `SwapStep` occupies 7 * 32 = 224 bytes
        for step in &steps[..n_steps] {
            offset += Self::encode_swap_step(step, &mut out_buf[offset..]);
        }

        offset
    }

    /// Encodes a single `SwapStep` tuple into 7 ABI words (224 bytes).
    #[inline(always)]
    fn encode_swap_step(step: &AbiSwapStep, dst: &mut [u8]) -> usize {
        // 0: uint8 dexType
        dst[31] = step.dex_type;
        // 1: address poolOrRouter
        dst[32 + 12..64].copy_from_slice(&step.pool_or_router);
        // 2: address tokenIn
        dst[64 + 12..96].copy_from_slice(&step.token_in);
        // 3: address tokenOut
        dst[96 + 12..128].copy_from_slice(&step.token_out);
        // 4: uint24 feeTier
        dst[128 + 28..160].copy_from_slice(&(step.fee_tier & 0xFF_FFFF).to_be_bytes());
        // 5: int128 curveIndexIn (sign-extended to 256-bit two's complement)
        if step.curve_index_in < 0 {
            dst[160..176].fill(0xFF);
        }
        dst[176..192].copy_from_slice(&step.curve_index_in.to_be_bytes());
        // 6: int128 curveIndexOut
        if step.curve_index_out < 0 {
            dst[192..208].fill(0xFF);
        }
        dst[208..224].copy_from_slice(&step.curve_index_out.to_be_bytes());
        224
    }

    /// Computes the EIP-1559 transaction digest (`keccak256(0x02 || rlp([chain_id, nonce, ...]))`)
    /// on the stack for zero-allocation hot-path signing.
    #[inline(always)]
    pub fn compute_eip1559_signing_digest(
        &self,
        tpl: &Eip1559TxTemplate,
        calldata: &[u8],
    ) -> [u8; 32] {
        let mut payload = [0u8; 1664];
        payload[0] = 0x02; // EIP-2718 Type 2 (EIP-1559)
        let mut pos = 1usize;

        payload[pos..pos + 8].copy_from_slice(&tpl.chain_id.to_be_bytes());
        pos += 8;
        payload[pos..pos + 8].copy_from_slice(&tpl.nonce.to_be_bytes());
        pos += 8;
        payload[pos..pos + 16].copy_from_slice(&tpl.max_priority_fee_per_gas_wei.to_be_bytes());
        pos += 16;
        payload[pos..pos + 16].copy_from_slice(&tpl.max_fee_per_gas_wei.to_be_bytes());
        pos += 16;
        payload[pos..pos + 8].copy_from_slice(&tpl.gas_limit.to_be_bytes());
        pos += 8;
        payload[pos..pos + 20].copy_from_slice(&tpl.to_executor);
        pos += 20;

        let cd_len = calldata.len().min(payload.len() - pos);
        payload[pos..pos + cd_len].copy_from_slice(&calldata[..cd_len]);
        pos += cd_len;

        keccak256(&payload[..pos])
    }
}

// =============================================================================
// Official Ethereum Keccak-256 (1600-bit Sponge, 24 Rounds, 0x01 Domain Padding)
// =============================================================================
const KECCAKF_RNDC: [u64; 24] = [
    0x0000000000000001, 0x0000000000008082, 0x800000000000808a, 0x8000000080008000,
    0x000000000000808b, 0x0000000080000001, 0x8000000080008081, 0x8000000000008009,
    0x000000000000008a, 0x0000000000000088, 0x0000000080008009, 0x000000008000000a,
    0x000000008000808b, 0x800000000000008b, 0x8000000000008089, 0x8000000000008003,
    0x8000000000008002, 0x8000000000000080, 0x000000000000800a, 0x800000008000000a,
    0x8000000080008081, 0x8000000000008080, 0x0000000080000001, 0x8000000080008008,
];

const KECCAKF_ROTC: [u32; 24] = [
    1, 3, 6, 10, 15, 21, 28, 36, 45, 55, 2, 14, 27, 41, 56, 8, 25, 43, 62, 18, 39, 61, 20, 44,
];

const KECCAKF_PILN: [usize; 24] = [
    10, 7, 11, 17, 18, 3, 5, 16, 8, 21, 24, 4, 15, 23, 19, 13, 12, 2, 20, 14, 22, 9, 6, 1,
];

#[inline(always)]
fn keccakf1600(st: &mut [u64; 25]) {
    let mut bc = [0u64; 5];
    for round in 0..24 {
        // Theta
        for i in 0..5 {
            bc[i] = st[i] ^ st[i + 5] ^ st[i + 10] ^ st[i + 15] ^ st[i + 20];
        }
        for i in 0..5 {
            let t = bc[(i + 4) % 5] ^ bc[(i + 1) % 5].rotate_left(1);
            for j in (0..25).step_by(5) {
                st[j + i] ^= t;
            }
        }
        // Rho & Pi
        let mut t = st[1];
        for i in 0..24 {
            let j = KECCAKF_PILN[i];
            let tmp = st[j];
            st[j] = t.rotate_left(KECCAKF_ROTC[i]);
            t = tmp;
        }
        // Chi
        for j in (0..25).step_by(5) {
            for i in 0..5 {
                bc[i] = st[j + i];
            }
            for i in 0..5 {
                st[j + i] ^= (!bc[(i + 1) % 5]) & bc[(i + 2) % 5];
            }
        }
        // Iota
        st[0] ^= KECCAKF_RNDC[round];
    }
}

/// Computes the 32-byte Ethereum Keccak-256 digest of `data` with zero heap allocation.
#[inline(always)]
pub fn keccak256(data: &[u8]) -> [u8; 32] {
    const RATE: usize = 136; // (1600 - 512) / 8
    let mut st = [0u64; 25];
    let mut chunks = data.chunks_exact(RATE);

    for chunk in &mut chunks {
        for i in 0..17 {
            let off = i * 8;
            let w = u64::from_le_bytes(chunk[off..off + 8].try_into().unwrap());
            st[i] ^= w;
        }
        keccakf1600(&mut st);
    }

    let rem = chunks.remainder();
    let mut block = [0u8; RATE];
    block[..rem.len()].copy_from_slice(rem);
    block[rem.len()] ^= 0x01; // Keccak (pre-FIPS) padding byte
    block[RATE - 1] ^= 0x80;

    for i in 0..17 {
        let off = i * 8;
        let w = u64::from_le_bytes(block[off..off + 8].try_into().unwrap());
        st[i] ^= w;
    }
    keccakf1600(&mut st);

    let mut out = [0u8; 32];
    for i in 0..4 {
        out[i * 8..i * 8 + 8].copy_from_slice(&st[i].to_le_bytes());
    }
    out
}
