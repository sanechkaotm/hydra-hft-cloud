use hydra_core::{MAX_BOOK_LEVELS, PriceLevel};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OrderSide {
    Buy,
    Sell,
}

impl OrderSide {
    #[inline(always)]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Buy => "BUY",
            Self::Sell => "SELL",
        }
    }
}

/// Zero-allocation parsed Binance `depthUpdate` event (`@depth@100ms` or `@depth5@100ms`).
#[derive(Copy, Clone, Debug)]
pub struct BinanceDepthEvent {
    pub first_update_id: u64,
    pub final_update_id: u64,
    pub event_time_ms: u64,
    pub bids: [PriceLevel; MAX_BOOK_LEVELS],
    pub bid_count: usize,
    pub asks: [PriceLevel; MAX_BOOK_LEVELS],
    pub ask_count: usize,
}

/// Binance Spot/Margin Zero-Allocation Diff-Depth Parser & IOC Order Serializer (Tokyo Node A).
pub struct BinanceConnector {
    api_key: [u8; 64],
    api_key_len: usize,
    api_secret: [u8; 64],
    api_secret_len: usize,
}

impl BinanceConnector {
    pub fn new(api_key: &str, api_secret: &str) -> Self {
        let mut key_buf = [0u8; 64];
        let k_len = api_key.len().min(64);
        key_buf[..k_len].copy_from_slice(&api_key.as_bytes()[..k_len]);

        let mut sec_buf = [0u8; 64];
        let s_len = api_secret.len().min(64);
        sec_buf[..s_len].copy_from_slice(&api_secret.as_bytes()[..s_len]);

        Self {
            api_key: key_buf,
            api_key_len: k_len,
            api_secret: sec_buf,
            api_secret_len: s_len,
        }
    }

    /// Parses a raw Binance `depthUpdate` JSON payload directly from a byte slice with zero heap allocations.
    /// Expected format: `{"e":"depthUpdate","E":1700000000123,"s":"EURUSDT","U":1001,"u":1005,"b":[["1.0895","5000.0"]],"a":[["1.0897","4200.0"]]}`
    #[inline(always)]
    pub fn parse_depth_update(raw: &[u8]) -> Option<BinanceDepthEvent> {
        let event_time_ms = extract_u64_after_key(raw, b"\"E\":")?;
        let first_update_id = extract_u64_after_key(raw, b"\"U\":")?;
        let final_update_id = extract_u64_after_key(raw, b"\"u\":")?;

        let mut bids = [PriceLevel::new(0.0, 0.0); MAX_BOOK_LEVELS];
        let bid_count = extract_price_levels_after_key(raw, b"\"b\":", &mut bids);

        let mut asks = [PriceLevel::new(0.0, 0.0); MAX_BOOK_LEVELS];
        let ask_count = extract_price_levels_after_key(raw, b"\"a\":", &mut asks);

        Some(BinanceDepthEvent {
            first_update_id,
            final_update_id,
            event_time_ms,
            bids,
            bid_count,
            asks,
            ask_count,
        })
    }

    /// Serializes a pre-signed Binance WebSocket API v3 `order.place` **IOC (Immediate-Or-Cancel)** request
    /// into a fixed stack buffer `out_buf` with zero heap allocation.
    #[inline(always)]
    pub fn build_ws_ioc_order(
        &self,
        req_id: u64,
        symbol: &str,
        side: OrderSide,
        quantity: f64,
        limit_price: f64,
        timestamp_ms: u64,
        out_buf: &mut [u8; 1024],
    ) -> usize {
        // Build query string for HMAC-SHA256 signature in stack buffer
        let mut query_buf = [0u8; 384];
        let mut q_len = 0usize;

        q_len += write_bytes(&mut query_buf[q_len..], b"apiKey=");
        q_len += write_bytes(&mut query_buf[q_len..], &self.api_key[..self.api_key_len]);
        q_len += write_bytes(&mut query_buf[q_len..], b"&price=");
        q_len += write_f64_6dec(&mut query_buf[q_len..], limit_price);
        q_len += write_bytes(&mut query_buf[q_len..], b"&quantity=");
        q_len += write_f64_6dec(&mut query_buf[q_len..], quantity);
        q_len += write_bytes(&mut query_buf[q_len..], b"&side=");
        q_len += write_bytes(&mut query_buf[q_len..], side.as_str().as_bytes());
        q_len += write_bytes(&mut query_buf[q_len..], b"&symbol=");
        q_len += write_bytes(&mut query_buf[q_len..], symbol.as_bytes());
        q_len += write_bytes(&mut query_buf[q_len..], b"&timeInForce=IOC&timestamp=");
        q_len += write_u64(&mut query_buf[q_len..], timestamp_ms);
        q_len += write_bytes(&mut query_buf[q_len..], b"&type=LIMIT");

        let sig_hex = hmac_sha256_hex(
            &self.api_secret[..self.api_secret_len],
            &query_buf[..q_len],
        );

        let mut pos = 0usize;
        pos += write_bytes(&mut out_buf[pos..], b"{\"id\":\"");
        pos += write_u64(&mut out_buf[pos..], req_id);
        pos += write_bytes(&mut out_buf[pos..], b"\",\"method\":\"order.place\",\"params\":{\"apiKey\":\"");
        pos += write_bytes(&mut out_buf[pos..], &self.api_key[..self.api_key_len]);
        pos += write_bytes(&mut out_buf[pos..], b"\",\"symbol\":\"");
        pos += write_bytes(&mut out_buf[pos..], symbol.as_bytes());
        pos += write_bytes(&mut out_buf[pos..], b"\",\"side\":\"");
        pos += write_bytes(&mut out_buf[pos..], side.as_str().as_bytes());
        pos += write_bytes(&mut out_buf[pos..], b"\",\"type\":\"LIMIT\",\"timeInForce\":\"IOC\",\"price\":\"");
        pos += write_f64_6dec(&mut out_buf[pos..], limit_price);
        pos += write_bytes(&mut out_buf[pos..], b"\",\"quantity\":\"");
        pos += write_f64_6dec(&mut out_buf[pos..], quantity);
        pos += write_bytes(&mut out_buf[pos..], b"\",\"timestamp\":");
        pos += write_u64(&mut out_buf[pos..], timestamp_ms);
        pos += write_bytes(&mut out_buf[pos..], b",\"signature\":\"");
        pos += write_bytes(&mut out_buf[pos..], &sig_hex);
        pos += write_bytes(&mut out_buf[pos..], b"\"}}");
        pos
    }
}

// =============================================================================
// Zero-Allocation Byte Scanners & Formatters
// =============================================================================
#[inline(always)]
pub(crate) fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    let max = haystack.len() - needle.len();
    for i in 0..=max {
        if &haystack[i..i + needle.len()] == needle {
            return Some(i + needle.len());
        }
    }
    None
}

#[inline(always)]
pub(crate) fn extract_u64_after_key(raw: &[u8], key: &[u8]) -> Option<u64> {
    let mut idx = find_subslice(raw, key)?;
    while idx < raw.len() && (raw[idx] == b' ' || raw[idx] == b'"') {
        idx += 1;
    }
    let mut val = 0u64;
    let mut found = false;
    while idx < raw.len() && raw[idx].is_ascii_digit() {
        val = val.wrapping_mul(10).wrapping_add((raw[idx] - b'0') as u64);
        found = true;
        idx += 1;
    }
    if found { Some(val) } else { None }
}

#[inline(always)]
pub(crate) fn parse_fast_f64(bytes: &[u8]) -> Option<f64> {
    let mut idx = 0usize;
    while idx < bytes.len() && (bytes[idx] == b' ' || bytes[idx] == b'"') {
        idx += 1;
    }
    let mut int_part = 0u64;
    let mut frac_part = 0u64;
    let mut frac_div = 1.0f64;
    let mut in_frac = false;
    let mut digits = 0usize;

    while idx < bytes.len() {
        let b = bytes[idx];
        if b.is_ascii_digit() {
            if !in_frac {
                int_part = int_part.wrapping_mul(10).wrapping_add((b - b'0') as u64);
            } else if frac_div < 1e12 {
                frac_part = frac_part.wrapping_mul(10).wrapping_add((b - b'0') as u64);
                frac_div *= 10.0;
            }
            digits += 1;
        } else if b == b'.' && !in_frac {
            in_frac = true;
        } else {
            break;
        }
        idx += 1;
    }
    if digits == 0 {
        return None;
    }
    Some((int_part as f64) + (frac_part as f64) / frac_div)
}

#[inline(always)]
pub(crate) fn extract_price_levels_after_key(
    raw: &[u8],
    key: &[u8],
    out: &mut [PriceLevel; MAX_BOOK_LEVELS],
) -> usize {
    let Some(mut idx) = find_subslice(raw, key) else {
        return 0;
    };
    while idx < raw.len() && raw[idx] != b'[' {
        idx += 1;
    }
    idx += 1; // enter outer '['

    let mut count = 0usize;
    while idx < raw.len() && count < MAX_BOOK_LEVELS {
        if raw[idx] == b']' {
            break;
        }
        if raw[idx] == b'[' {
            idx += 1;
            let p_opt = parse_fast_f64(&raw[idx..]);
            while idx < raw.len() && raw[idx] != b',' && raw[idx] != b']' {
                idx += 1;
            }
            if idx < raw.len() && raw[idx] == b',' {
                idx += 1;
            }
            let q_opt = parse_fast_f64(&raw[idx..]);
            while idx < raw.len() && raw[idx] != b']' {
                idx += 1;
            }
            if idx < raw.len() && raw[idx] == b']' {
                idx += 1;
            }
            if let (Some(p), Some(q)) = (p_opt, q_opt) {
                out[count] = PriceLevel::new(p, q);
                count += 1;
            }
        } else {
            idx += 1;
        }
    }
    count
}

#[inline(always)]
pub(crate) fn write_bytes(dst: &mut [u8], src: &[u8]) -> usize {
    let n = src.len().min(dst.len());
    dst[..n].copy_from_slice(&src[..n]);
    n
}

#[inline(always)]
pub(crate) fn write_u64(dst: &mut [u8], mut val: u64) -> usize {
    if val == 0 {
        if !dst.is_empty() {
            dst[0] = b'0';
            return 1;
        }
        return 0;
    }
    let mut tmp = [0u8; 20];
    let mut i = 20usize;
    while val > 0 {
        i -= 1;
        tmp[i] = b'0' + (val % 10) as u8;
        val /= 10;
    }
    write_bytes(dst, &tmp[i..])
}

#[inline(always)]
pub(crate) fn write_f64_6dec(dst: &mut [u8], val: f64) -> usize {
    let scaled = (val.max(0.0) * 1_000_000.0).round() as u64;
    let int_part = scaled / 1_000_000;
    let frac_part = scaled % 1_000_000;

    let mut pos = write_u64(dst, int_part);
    pos += write_bytes(&mut dst[pos..], b".");

    let mut frac_buf = [b'0'; 6];
    let mut f = frac_part;
    for j in (0..6).rev() {
        frac_buf[j] = b'0' + (f % 10) as u8;
        f /= 10;
    }
    pos += write_bytes(&mut dst[pos..], &frac_buf);
    pos
}

// =============================================================================
// FIPS 180-4 SHA-256 & RFC 2104 HMAC-SHA256 (Zero Heap Allocation)
// =============================================================================
const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

#[inline(always)]
fn sha256_compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 64];
    for i in 0..16 {
        let off = i * 4;
        w[i] = u32::from_be_bytes([block[off], block[off + 1], block[off + 2], block[off + 3]]);
    }
    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
    }

    let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = *state;
    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ ((!e) & g);
        let temp1 = h.wrapping_add(s1).wrapping_add(ch).wrapping_add(SHA256_K[i]).wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let temp2 = s0.wrapping_add(maj);

        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(temp1);
        d = c;
        c = b;
        b = a;
        a = temp1.wrapping_add(temp2);
    }

    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
    state[5] = state[5].wrapping_add(f);
    state[6] = state[6].wrapping_add(g);
    state[7] = state[7].wrapping_add(h);
}

#[inline(always)]
pub fn sha256_two_slices(a: &[u8], b: &[u8]) -> [u8; 32] {
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
    ];
    let total_len = a.len() + b.len();
    let mut block = [0u8; 64];
    let mut block_len = 0usize;

    for &byte in a.iter().chain(b.iter()) {
        block[block_len] = byte;
        block_len += 1;
        if block_len == 64 {
            sha256_compress(&mut state, &block);
            block_len = 0;
        }
    }

    block[block_len] = 0x80;
    block_len += 1;
    if block_len > 56 {
        for slot in &mut block[block_len..64] {
            *slot = 0;
        }
        sha256_compress(&mut state, &block);
        block_len = 0;
    }
    for slot in &mut block[block_len..56] {
        *slot = 0;
    }
    let bit_len = (total_len as u64) * 8;
    block[56..64].copy_from_slice(&bit_len.to_be_bytes());
    sha256_compress(&mut state, &block);

    let mut out = [0u8; 32];
    for i in 0..8 {
        out[i * 4..i * 4 + 4].copy_from_slice(&state[i].to_be_bytes());
    }
    out
}

#[inline(always)]
pub fn hmac_sha256_hex(key: &[u8], msg: &[u8]) -> [u8; 64] {
    let mut k_pad = [0u8; 64];
    if key.len() > 64 {
        let kh = sha256_two_slices(key, &[]);
        k_pad[..32].copy_from_slice(&kh);
    } else {
        k_pad[..key.len()].copy_from_slice(key);
    }

    let mut i_key_pad = [0x36u8; 64];
    let mut o_key_pad = [0x5cu8; 64];
    for i in 0..64 {
        i_key_pad[i] ^= k_pad[i];
        o_key_pad[i] ^= k_pad[i];
    }

    let inner = sha256_two_slices(&i_key_pad, msg);
    let mac = sha256_two_slices(&o_key_pad, &inner);

    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut hex_out = [0u8; 64];
    for i in 0..32 {
        hex_out[i * 2] = HEX[(mac[i] >> 4) as usize];
        hex_out[i * 2 + 1] = HEX[(mac[i] & 0x0F) as usize];
    }
    hex_out
}
