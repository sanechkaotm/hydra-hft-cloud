use crate::binance::{
    OrderSide, find_subslice, parse_fast_f64, write_bytes, write_f64_6dec, write_u64,
};
use hydra_core::{MAX_BOOK_LEVELS, PriceLevel};

/// Parsed Kraken WebSocket v2 `book` channel tick.
#[derive(Copy, Clone, Debug)]
pub struct KrakenBookV2Event {
    pub bids: [PriceLevel; MAX_BOOK_LEVELS],
    pub bid_count: usize,
    pub asks: [PriceLevel; MAX_BOOK_LEVELS],
    pub ask_count: usize,
    pub checksum: u64,
}

/// Kraken WebSocket v2 Multi-Fiat Book Parser & IOC Order Client (London Node B).
pub struct KrakenConnector {
    ws_token: [u8; 64],
    ws_token_len: usize,
}

impl KrakenConnector {
    pub fn new(ws_token: &str) -> Self {
        let mut buf = [0u8; 64];
        let len = ws_token.len().min(64);
        buf[..len].copy_from_slice(&ws_token.as_bytes()[..len]);
        Self {
            ws_token: buf,
            ws_token_len: len,
        }
    }

    /// Parses a Kraken WebSocket v2 `book` update (`{"channel":"book","type":"update","data":[{"symbol":"EUR/USD","bids":[{"price":1.0895,"qty":5000.0}],"asks":[{"price":1.0897,"qty":4200.0}],"checksum":123456}]}`)
    /// with zero heap allocations.
    #[inline(always)]
    pub fn parse_book_v2(raw: &[u8]) -> Option<KrakenBookV2Event> {
        if find_subslice(raw, b"\"channel\":\"book\"").is_none() {
            return None;
        }
        let mut bids = [PriceLevel::new(0.0, 0.0); MAX_BOOK_LEVELS];
        let bid_count = extract_kraken_v2_levels(raw, b"\"bids\":", &mut bids);

        let mut asks = [PriceLevel::new(0.0, 0.0); MAX_BOOK_LEVELS];
        let ask_count = extract_kraken_v2_levels(raw, b"\"asks\":", &mut asks);

        let checksum = crate::binance::extract_u64_after_key(raw, b"\"checksum\":").unwrap_or(0);

        Some(KrakenBookV2Event {
            bids,
            bid_count,
            asks,
            ask_count,
            checksum,
        })
    }

    /// Formats a Kraken WebSocket v2 `add_order` **IOC (`"time_in_force":"ioc"`)** command
    /// into `out_buf` with zero heap allocation.
    #[inline(always)]
    pub fn build_ws_v2_ioc_order(
        &self,
        req_id: u64,
        symbol: &str,
        side: OrderSide,
        order_qty: f64,
        limit_price: f64,
        out_buf: &mut [u8; 512],
    ) -> usize {
        let side_str = match side {
            OrderSide::Buy => "buy",
            OrderSide::Sell => "sell",
        };
        let mut pos = 0usize;
        pos += write_bytes(&mut out_buf[pos..], b"{\"method\":\"add_order\",\"req_id\":");
        pos += write_u64(&mut out_buf[pos..], req_id);
        pos += write_bytes(&mut out_buf[pos..], b",\"params\":{\"order_type\":\"limit\",\"side\":\"");
        pos += write_bytes(&mut out_buf[pos..], side_str.as_bytes());
        pos += write_bytes(&mut out_buf[pos..], b"\",\"symbol\":\"");
        pos += write_bytes(&mut out_buf[pos..], symbol.as_bytes());
        pos += write_bytes(&mut out_buf[pos..], b"\",\"limit_price\":");
        pos += write_f64_6dec(&mut out_buf[pos..], limit_price);
        pos += write_bytes(&mut out_buf[pos..], b",\"order_qty\":");
        pos += write_f64_6dec(&mut out_buf[pos..], order_qty);
        pos += write_bytes(&mut out_buf[pos..], b",\"time_in_force\":\"ioc\",\"token\":\"");
        pos += write_bytes(&mut out_buf[pos..], &self.ws_token[..self.ws_token_len]);
        pos += write_bytes(&mut out_buf[pos..], b"\"}}");
        pos
    }
}

#[inline(always)]
fn extract_kraken_v2_levels(
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
    idx += 1;

    let mut count = 0usize;
    while idx < raw.len() && count < MAX_BOOK_LEVELS {
        if raw[idx] == b']' {
            break;
        }
        if raw[idx] == b'{' {
            let slice = &raw[idx..];
            let p_opt = find_subslice(slice, b"\"price\":").and_then(|o| parse_fast_f64(&slice[o..]));
            let q_opt = find_subslice(slice, b"\"qty\":").and_then(|o| parse_fast_f64(&slice[o..]));
            while idx < raw.len() && raw[idx] != b'}' {
                idx += 1;
            }
            if idx < raw.len() && raw[idx] == b'}' {
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
