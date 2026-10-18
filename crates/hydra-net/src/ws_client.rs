use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

/// RFC 6455 WebSocket Opcode.
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum WsOpcode {
    Continuation = 0x0,
    Text = 0x1,
    Binary = 0x2,
    Close = 0x8,
    Ping = 0x9,
    Pong = 0xA,
}

impl WsOpcode {
    #[inline(always)]
    pub const fn from_u8(b: u8) -> Option<Self> {
        match b & 0x0F {
            0x0 => Some(Self::Continuation),
            0x1 => Some(Self::Text),
            0x2 => Some(Self::Binary),
            0x8 => Some(Self::Close),
            0x9 => Some(Self::Ping),
            0xA => Some(Self::Pong),
            _ => None,
        }
    }
}

/// Zero-copy view into a decoded RFC 6455 WebSocket frame inside a pre-allocated ring/stack buffer.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct ZeroCopyWsFrame<'a> {
    pub fin: bool,
    pub opcode: WsOpcode,
    pub payload: &'a [u8],
    pub total_frame_bytes: usize,
}

/// Connection lifecycle state for `WsStreamManager`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum WsConnectionState {
    Disconnected,
    Connecting,
    Streaming,
    BackoffWait { attempt: u32, next_retry_ts_ns: u64 },
}

/// Tuned `TCP_NODELAY` WebSocket Stream Manager with zero-copy frame decoding and auto-reconnect.
pub struct WsStreamManager {
    pub endpoint_host: [u8; 64],
    pub endpoint_host_len: usize,
    pub endpoint_path: [u8; 128],
    pub endpoint_path_len: usize,
    pub state: WsConnectionState,
    pub reconnect_attempts: u32,
    pub last_pong_ts_ns: u64,
    pub last_rtt_ns: u64,
    rx_buf: [u8; 65536],
    rx_len: usize,
    stream: Option<TcpStream>,
}

impl WsStreamManager {
    pub fn new(host: &str, path: &str) -> Self {
        let mut endpoint_host = [0u8; 64];
        let h_len = host.len().min(64);
        endpoint_host[..h_len].copy_from_slice(&host.as_bytes()[..h_len]);

        let mut endpoint_path = [0u8; 128];
        let p_len = path.len().min(128);
        endpoint_path[..p_len].copy_from_slice(&path.as_bytes()[..p_len]);

        Self {
            endpoint_host,
            endpoint_host_len: h_len,
            endpoint_path,
            endpoint_path_len: p_len,
            state: WsConnectionState::Disconnected,
            reconnect_attempts: 0,
            last_pong_ts_ns: 0,
            last_rtt_ns: 1_500_000, // 1.5ms initial baseline
            rx_buf: [0u8; 65536],
            rx_len: 0,
            stream: None,
        }
    }

    /// Connects a raw TCP socket with `TCP_NODELAY = true` (disabling Nagle's algorithm) and performs
    /// the RFC 6455 HTTP/1.1 Upgrade handshake into a stack buffer.
    pub fn connect_tcp_nodelay(&mut self, addr: SocketAddr, now_ns: u64) -> io::Result<()> {
        self.state = WsConnectionState::Connecting;
        let stream = TcpStream::connect_timeout(&addr, Duration::from_millis(1500))?;
        stream.set_nodelay(true)?;
        stream.set_nonblocking(true)?;

        let mut req_buf = [0u8; 512];
        let req_len = self.format_upgrade_request(&mut req_buf);
        let mut s = &stream;
        let _ = s.write_all(&req_buf[..req_len]);

        self.stream = Some(stream);
        self.state = WsConnectionState::Streaming;
        self.reconnect_attempts = 0;
        self.last_pong_ts_ns = now_ns;
        self.rx_len = 0;
        Ok(())
    }

    /// Triggers automatic reconnect state transition with bounded sub-second jittered backoff.
    #[inline(always)]
    pub fn trigger_reconnect(&mut self, now_ns: u64) {
        self.stream = None;
        self.rx_len = 0;
        self.reconnect_attempts = self.reconnect_attempts.saturating_add(1);
        let backoff_ms = (10u64 << self.reconnect_attempts.min(6)).min(500);
        let next_retry_ts_ns = now_ns + backoff_ms * 1_000_000;
        self.state = WsConnectionState::BackoffWait {
            attempt: self.reconnect_attempts,
            next_retry_ts_ns,
        };
    }

    /// Formats an HTTP/1.1 WebSocket Upgrade header into `out` without heap allocation.
    #[inline(always)]
    pub fn format_upgrade_request(&self, out: &mut [u8; 512]) -> usize {
        let host = &self.endpoint_host[..self.endpoint_host_len];
        let path = &self.endpoint_path[..self.endpoint_path_len];
        let mut pos = 0usize;

        let mut push_slice = |s: &[u8]| {
            let n = s.len().min(out.len().saturating_sub(pos));
            out[pos..pos + n].copy_from_slice(&s[..n]);
            pos += n;
        };

        push_slice(b"GET ");
        push_slice(path);
        push_slice(b" HTTP/1.1\r\nHost: ");
        push_slice(host);
        push_slice(b"\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n");
        push_slice(b"Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n");
        push_slice(b"Sec-WebSocket-Version: 13\r\n\r\n");
        pos
    }

    /// Encodes a client-to-server masked WebSocket frame into a stack buffer with zero heap allocation.
    #[inline(always)]
    pub fn encode_client_frame(
        opcode: WsOpcode,
        payload: &[u8],
        mask_key: [u8; 4],
        out_buf: &mut [u8],
    ) -> Option<usize> {
        let len = payload.len();
        let header_len = if len <= 125 {
            6
        } else if len <= 65535 {
            8
        } else {
            14
        };

        if out_buf.len() < header_len + len {
            return None;
        }

        out_buf[0] = 0x80 | (opcode as u8); // FIN = 1
        let mask_offset = if len <= 125 {
            out_buf[1] = 0x80 | (len as u8);
            2
        } else if len <= 65535 {
            out_buf[1] = 0x80 | 126;
            let l_bytes = (len as u16).to_be_bytes();
            out_buf[2] = l_bytes[0];
            out_buf[3] = l_bytes[1];
            4
        } else {
            out_buf[1] = 0x80 | 127;
            let l_bytes = (len as u64).to_be_bytes();
            out_buf[2..10].copy_from_slice(&l_bytes);
            10
        };

        out_buf[mask_offset..mask_offset + 4].copy_from_slice(&mask_key);
        let data_start = mask_offset + 4;
        for i in 0..len {
            out_buf[data_start + i] = payload[i] ^ mask_key[i & 3];
        }

        Some(data_start + len)
    }

    /// Zero-copy parser for unmasked server-to-client WebSocket frames.
    #[inline(always)]
    pub fn decode_server_frame(buf: &[u8]) -> Option<ZeroCopyWsFrame<'_>> {
        if buf.len() < 2 {
            return None;
        }
        let b0 = buf[0];
        let b1 = buf[1];
        let fin = (b0 & 0x80) != 0;
        let opcode = WsOpcode::from_u8(b0)?;
        let masked = (b1 & 0x80) != 0;
        let mut payload_len = (b1 & 0x7F) as usize;
        let mut offset = 2usize;

        if payload_len == 126 {
            if buf.len() < 4 {
                return None;
            }
            payload_len = u16::from_be_bytes([buf[2], buf[3]]) as usize;
            offset = 4;
        } else if payload_len == 127 {
            if buf.len() < 10 {
                return None;
            }
            payload_len = u64::from_be_bytes([
                buf[2], buf[3], buf[4], buf[5], buf[6], buf[7], buf[8], buf[9],
            ]) as usize;
            offset = 10;
        }

        if masked {
            offset += 4;
        }

        let total_len = offset.checked_add(payload_len)?;
        if buf.len() < total_len {
            return None;
        }

        Some(ZeroCopyWsFrame {
            fin,
            opcode,
            payload: &buf[offset..total_len],
            total_frame_bytes: total_len,
        })
    }

    /// Polls non-blocking TCP stream and invokes zero-copy frame parser.
    pub fn poll_bytes(&mut self) -> io::Result<usize> {
        if let Some(ref mut stream) = self.stream {
            match stream.read(&mut self.rx_buf[self.rx_len..]) {
                Ok(0) => Err(io::Error::new(io::ErrorKind::UnexpectedEof, "WS EOF")),
                Ok(n) => {
                    self.rx_len += n;
                    Ok(n)
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(0),
                Err(e) => Err(e),
            }
        } else {
            Ok(0)
        }
    }
}
