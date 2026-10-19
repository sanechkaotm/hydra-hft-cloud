use hydra_core::{AssetPairId, NodeRole};
use std::io;
use std::net::{SocketAddr, UdpSocket};

pub const HYDRA_MAGIC: u32 = 0x4859_4452; // "HYDR"
pub const HYDRA_PROTOCOL_VERSION: u8 = 0x04;

pub const SIGNAL_FIAT_BREAKOUT: u8 = 1;
pub const SIGNAL_CRYPTO_SWEEP: u8 = 2;
pub const SIGNAL_GLOBAL_KILLSWITCH: u8 = 3;

/// Maximum allowable Lead-Lag UDP impulse age: 250 milliseconds (`250_000_000` ns).
pub const MAX_IMPULSE_AGE_NS: u64 = 250_000_000;

/// Minimum aggressive sweep threshold in basis points required for Tokyo to broadcast an impulse (`15 bps`).
pub const MIN_SWEEP_MAGNITUDE_BPS: u16 = 15;

/// Fixed 64-byte binary frame (`#[repr(C, packed)]`) for zero-copy Tri-Node UDP Lead-Lag Mesh.
#[repr(C, packed)]
#[derive(Copy, Clone, Debug)]
pub struct ImpulseFrame {
    pub magic: u32,           // 0x48594452 ("HYDR")
    pub version: u8,          // 0x04
    pub source_node: u8,      // 1 = Tokyo, 2 = London, 3 = Frankfurt
    pub signal_type: u8,      // 1 = FiatBreakout, 2 = CryptoSweep, 3 = GlobalKillSwitch
    pub asset_pair_id: u16,   // Pre-mapped ID (e.g., USDT_TRY, EUR_USD, BTC_USDT)
    pub direction: i8,        // +1 = Upward surge, -1 = Downward dump
    pub magnitude_bps: u16,   // Impulse strength in basis points (e.g., 35 = 0.35%)
    pub timestamp_ns: u64,    // Monotonic UNIX nanoseconds
    pub seq_no: u64,          // Monotonic sequence number per node
    pub volume_usd_est: u32,  // Swept volume in USD
    pub reserved: [u8; 16],   // Padding for alignment
    pub blake3_mac: [u8; 16], // Truncated 128-bit keyed BLAKE3 authentication tag
}

const _: () = assert!(std::mem::size_of::<ImpulseFrame>() == 64);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FrameVerifyError {
    InvalidMagic,
    UnsupportedVersion,
    MacMismatch,
    StaleFrame { age_ns: u64 },
    InvalidSignalType,
    ReplayedSequence { last_seen: u64, received: u64 },
}

impl ImpulseFrame {
    /// Constructs and authenticates a 64-byte `ImpulseFrame` using a 32-byte BLAKE3 symmetric key.
    #[inline(always)]
    pub fn new_signed(
        source_node: NodeRole,
        signal_type: u8,
        asset_pair: AssetPairId,
        direction: i8,
        magnitude_bps: u16,
        timestamp_ns: u64,
        seq_no: u64,
        volume_usd_est: u32,
        key: &[u8; 32],
    ) -> Self {
        let mut frame = Self {
            magic: HYDRA_MAGIC.to_le(),
            version: HYDRA_PROTOCOL_VERSION,
            source_node: source_node as u8,
            signal_type,
            asset_pair_id: asset_pair.0.to_le(),
            direction,
            magnitude_bps: magnitude_bps.to_le(),
            timestamp_ns: timestamp_ns.to_le(),
            seq_no: seq_no.to_le(),
            volume_usd_est: volume_usd_est.to_le(),
            reserved: [0u8; 16],
            blake3_mac: [0u8; 16],
        };

        let bytes = frame.to_bytes();
        frame.blake3_mac = blake3_keyed_mac_128(key, &bytes[..48]);
        frame
    }

    /// Serializes the 64-byte packed struct into a stack byte array with zero heap allocation and zero unsafe.
    #[inline(always)]
    pub fn to_bytes(&self) -> [u8; 64] {
        let mut bytes = [0u8; 64];
        bytes[0..4].copy_from_slice(&self.magic.to_le_bytes());
        bytes[4] = self.version;
        bytes[5] = self.source_node;
        bytes[6] = self.signal_type;
        bytes[7..9].copy_from_slice(&self.asset_pair_id.to_le_bytes());
        bytes[9] = self.direction as u8;
        bytes[10..12].copy_from_slice(&self.magnitude_bps.to_le_bytes());
        bytes[12..20].copy_from_slice(&self.timestamp_ns.to_le_bytes());
        bytes[20..28].copy_from_slice(&self.seq_no.to_le_bytes());
        bytes[28..32].copy_from_slice(&self.volume_usd_est.to_le_bytes());
        bytes[32..48].copy_from_slice(&self.reserved);
        bytes[48..64].copy_from_slice(&self.blake3_mac);
        bytes
    }

    /// Safely decodes a 64-byte raw slice into `ImpulseFrame` with zero unsafe code.
    #[inline(always)]
    pub fn from_raw_bytes(raw: &[u8; 64]) -> Self {
        Self {
            magic: u32::from_le_bytes(raw[0..4].try_into().unwrap()),
            version: raw[4],
            source_node: raw[5],
            signal_type: raw[6],
            asset_pair_id: u16::from_le_bytes(raw[7..9].try_into().unwrap()),
            direction: raw[9] as i8,
            magnitude_bps: u16::from_le_bytes(raw[10..12].try_into().unwrap()),
            timestamp_ns: u64::from_le_bytes(raw[12..20].try_into().unwrap()),
            seq_no: u64::from_le_bytes(raw[20..28].try_into().unwrap()),
            volume_usd_est: u32::from_le_bytes(raw[28..32].try_into().unwrap()),
            reserved: raw[32..48].try_into().unwrap(),
            blake3_mac: raw[48..64].try_into().unwrap(),
        }
    }

    /// Verifies the 64-byte frame's magic header, protocol version, 128-bit keyed BLAKE3 MAC,
    /// and freshness window (`now_ns - timestamp_ns <= max_age_ns`).
    #[inline(always)]
    pub fn verify_from_bytes(
        raw: &[u8; 64],
        key: &[u8; 32],
        now_ns: u64,
        max_age_ns: u64,
    ) -> Result<Self, FrameVerifyError> {
        let frame = Self::from_raw_bytes(raw);

        if u32::from_le(frame.magic) != HYDRA_MAGIC {
            return Err(FrameVerifyError::InvalidMagic);
        }
        if frame.version != HYDRA_PROTOCOL_VERSION {
            return Err(FrameVerifyError::UnsupportedVersion);
        }
        if !matches!(
            frame.signal_type,
            SIGNAL_FIAT_BREAKOUT | SIGNAL_CRYPTO_SWEEP | SIGNAL_GLOBAL_KILLSWITCH
        ) {
            return Err(FrameVerifyError::InvalidSignalType);
        }

        let expected_mac = blake3_keyed_mac_128(key, &raw[..48]);
        // Constant-time 16-byte MAC comparison
        let mut diff = 0u8;
        for i in 0..16 {
            diff |= expected_mac[i] ^ frame.blake3_mac[i];
        }
        if diff != 0 {
            return Err(FrameVerifyError::MacMismatch);
        }

        let ts = u64::from_le(frame.timestamp_ns);
        let age_ns = now_ns.saturating_sub(ts);
        if frame.signal_type != SIGNAL_GLOBAL_KILLSWITCH && age_ns > max_age_ns {
            return Err(FrameVerifyError::StaleFrame { age_ns });
        }

        Ok(frame)
    }
}

// =============================================================================
// Official Spec-Compliant Keyed BLAKE3 Single-Block Compression (48-byte Header)
// =============================================================================
const BLAKE3_CHUNK_START: u32 = 1 << 0;
const BLAKE3_CHUNK_END: u32 = 1 << 1;
const BLAKE3_ROOT: u32 = 1 << 3;
const BLAKE3_KEYED_HASH: u32 = 1 << 4;

const BLAKE3_IV: [u32; 8] = [
    0x6A09E667, 0xBB67AE85, 0x3C6EF372, 0xA54FF53A,
    0x510E527F, 0x9B05688C, 0x1F83D9AB, 0x5BE0CD19,
];

const MSG_PERMUTATION: [usize; 16] = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8];

#[inline(always)]
fn blake3_g(state: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize, mx: u32, my: u32) {
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(mx);
    state[d] = (state[d] ^ state[a]).rotate_right(16);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(12);
    state[a] = state[a].wrapping_add(state[b]).wrapping_add(my);
    state[d] = (state[d] ^ state[a]).rotate_right(8);
    state[c] = state[c].wrapping_add(state[d]);
    state[b] = (state[b] ^ state[c]).rotate_right(7);
}

#[inline(always)]
fn blake3_round(state: &mut [u32; 16], m: &[u32; 16]) {
    // Column step
    blake3_g(state, 0, 4, 8, 12, m[0], m[1]);
    blake3_g(state, 1, 5, 9, 13, m[2], m[3]);
    blake3_g(state, 2, 6, 10, 14, m[4], m[5]);
    blake3_g(state, 3, 7, 11, 15, m[6], m[7]);
    // Diagonal step
    blake3_g(state, 0, 5, 10, 15, m[8], m[9]);
    blake3_g(state, 1, 6, 11, 12, m[10], m[11]);
    blake3_g(state, 2, 7, 8, 13, m[12], m[13]);
    blake3_g(state, 3, 4, 9, 14, m[14], m[15]);
}

/// Computes the exact 128-bit (16-byte) Keyed BLAKE3 authentication tag for a `<= 64` byte frame header.
#[inline(always)]
pub fn blake3_keyed_mac_128(key: &[u8; 32], header_48: &[u8]) -> [u8; 16] {
    let mut key_words = [0u32; 8];
    for i in 0..8 {
        let off = i * 4;
        key_words[i] = u32::from_le_bytes([key[off], key[off + 1], key[off + 2], key[off + 3]]);
    }

    let mut block = [0u8; 64];
    let len = header_48.len().min(64);
    block[..len].copy_from_slice(&header_48[..len]);

    let mut m = [0u32; 16];
    for i in 0..16 {
        let off = i * 4;
        m[i] = u32::from_le_bytes([block[off], block[off + 1], block[off + 2], block[off + 3]]);
    }

    let flags = BLAKE3_KEYED_HASH | BLAKE3_CHUNK_START | BLAKE3_CHUNK_END | BLAKE3_ROOT;
    let mut state = [
        key_words[0],
        key_words[1],
        key_words[2],
        key_words[3],
        key_words[4],
        key_words[5],
        key_words[6],
        key_words[7],
        BLAKE3_IV[0],
        BLAKE3_IV[1],
        BLAKE3_IV[2],
        BLAKE3_IV[3],
        0,          // counter low
        0,          // counter high
        len as u32, // block_len
        flags,
    ];

    for r in 0..7 {
        blake3_round(&mut state, &m);
        if r < 6 {
            let mut permuted = [0u32; 16];
            for i in 0..16 {
                permuted[i] = m[MSG_PERMUTATION[i]];
            }
            m = permuted;
        }
    }

    let mut out = [0u8; 16];
    for i in 0..4 {
        let word = state[i] ^ state[i + 8];
        let bytes = word.to_le_bytes();
        out[i * 4..i * 4 + 4].copy_from_slice(&bytes);
    }
    out
}

/// Non-blocking UDP Mesh Transceiver connecting Tokyo, London, and Frankfurt nodes.
pub struct UdpMeshNode {
    socket: UdpSocket,
    role: NodeRole,
    peers: [Option<SocketAddr>; 2],
    mac_key: [u8; 32],
    seq_no: u64,
    last_seen_seq: [u64; 4],
}

impl UdpMeshNode {
    pub fn bind(
        bind_addr: SocketAddr,
        role: NodeRole,
        peers: [Option<SocketAddr>; 2],
        mac_key: [u8; 32],
    ) -> io::Result<Self> {
        let socket = UdpSocket::bind(bind_addr)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            role,
            peers,
            mac_key,
            seq_no: 0,
            last_seen_seq: [0u64; 4],
        })
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    /// Broadcasts a Lead-Lag sweep impulse (`magnitude_bps >= 15`) to remote peers.
    #[inline(always)]
    pub fn broadcast_sweep_impulse(
        &mut self,
        signal_type: u8,
        asset_pair: AssetPairId,
        direction: i8,
        magnitude_bps: u16,
        timestamp_ns: u64,
        volume_usd_est: u32,
    ) -> usize {
        if signal_type != SIGNAL_GLOBAL_KILLSWITCH && magnitude_bps < MIN_SWEEP_MAGNITUDE_BPS {
            return 0;
        }
        self.seq_no = self.seq_no.wrapping_add(1);
        let frame = ImpulseFrame::new_signed(
            self.role,
            signal_type,
            asset_pair,
            direction,
            magnitude_bps,
            timestamp_ns,
            self.seq_no,
            volume_usd_est,
            &self.mac_key,
        );
        let raw = frame.to_bytes();
        let mut sent = 0usize;
        for peer in self.peers.iter().flatten() {
            if self.socket.send_to(&raw, peer).is_ok() {
                sent += 1;
            }
        }
        sent
    }

    /// Immediately broadcasts a `GlobalKillSwitch` (`signal_type == 3`) packet to halt all nodes within 1 RTT.
    #[inline(always)]
    pub fn broadcast_global_killswitch(&mut self, timestamp_ns: u64) -> usize {
        self.broadcast_sweep_impulse(
            SIGNAL_GLOBAL_KILLSWITCH,
            AssetPairId(0),
            0,
            u16::MAX,
            timestamp_ns,
            0,
        )
    }

    /// Polls for an incoming 64-byte `ImpulseFrame` without blocking or allocating,
    /// enforcing BLAKE3 MAC verification, freshness window, and monotonic sequence replay protection.
    #[inline(always)]
    pub fn poll_incoming(&mut self, now_ns: u64) -> Option<Result<ImpulseFrame, FrameVerifyError>> {
        let mut buf = [0u8; 64];
        match self.socket.recv_from(&mut buf) {
            Ok((64, _src)) => {
                match ImpulseFrame::verify_from_bytes(&buf, &self.mac_key, now_ns, MAX_IMPULSE_AGE_NS) {
                    Ok(frame) => {
                        let src_idx = (frame.source_node as usize).min(3);
                        let seq = u64::from_le(frame.seq_no);
                        if seq <= self.last_seen_seq[src_idx] {
                            Some(Err(FrameVerifyError::ReplayedSequence {
                                last_seen: self.last_seen_seq[src_idx],
                                received: seq,
                            }))
                        } else {
                            self.last_seen_seq[src_idx] = seq;
                            Some(Ok(frame))
                        }
                    }
                    Err(e) => Some(Err(e)),
                }
            }
            _ => None,
        }
    }
}
