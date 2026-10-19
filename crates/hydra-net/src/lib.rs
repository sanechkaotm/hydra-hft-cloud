pub mod udp_mesh;
pub mod ws_client;

pub use udp_mesh::{
    FrameVerifyError, HYDRA_MAGIC, HYDRA_PROTOCOL_VERSION, ImpulseFrame, MAX_IMPULSE_AGE_NS,
    MIN_SWEEP_MAGNITUDE_BPS, SIGNAL_CRYPTO_SWEEP, SIGNAL_FIAT_BREAKOUT, SIGNAL_GLOBAL_KILLSWITCH,
    UdpMeshNode, blake3_keyed_mac_128,
};
pub use ws_client::{WsConnectionState, WsOpcode, WsStreamManager, ZeroCopyWsFrame};

#[cfg(test)]
mod tests {
    use super::*;
    use hydra_core::{AssetPairId, NodeRole};

    #[test]
    fn test_64_byte_impulse_frame_blake3_mac_and_stale_guard() {
        assert_eq!(std::mem::size_of::<ImpulseFrame>(), 64);

        let key = *b"HYDRA_HFT_V4_MESH_MAC_SECRET_KEY";
        let ts_ns = 1_700_000_000_000_000_000u64;

        let frame = ImpulseFrame::new_signed(
            NodeRole::Tokyo,
            SIGNAL_FIAT_BREAKOUT,
            AssetPairId::USDT_TRY,
            1,
            35, // 35 bps aggressive sweep
            ts_ns,
            42,
            250_000,
            &key,
        );

        let raw = frame.to_bytes();

        // 1. Valid frame verified 85ms later (< 250ms budget)
        let verified = ImpulseFrame::verify_from_bytes(
            &raw,
            &key,
            ts_ns + 85_000_000,
            MAX_IMPULSE_AGE_NS,
        )
        .expect("Valid 64-byte frame must pass BLAKE3 MAC and freshness checks");

        let mag = u16::from_le(verified.magnitude_bps);
        assert_eq!(mag, 35);
        assert_eq!(verified.source_node, NodeRole::Tokyo as u8);

        // 2. Tampering with 1 bit in payload must fail BLAKE3 MAC verification
        let mut tampered = raw;
        tampered[9] ^= 0x01;
        assert_eq!(
            ImpulseFrame::verify_from_bytes(&tampered, &key, ts_ns + 10_000_000, MAX_IMPULSE_AGE_NS)
                .unwrap_err(),
            FrameVerifyError::MacMismatch
        );

        // 3. Stale packet (> 250ms old) must be rejected
        assert_eq!(
            ImpulseFrame::verify_from_bytes(&raw, &key, ts_ns + 300_000_000, MAX_IMPULSE_AGE_NS)
                .unwrap_err(),
            FrameVerifyError::StaleFrame {
                age_ns: 300_000_000
            }
        );
    }

    #[test]
    fn test_udp_mesh_tokyo_to_london_loopback_broadcast() {
        let key = *b"HYDRA_HFT_V4_MESH_MAC_SECRET_KEY";
        let bind_any: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();

        let mut london = UdpMeshNode::bind(bind_any, NodeRole::London, [None, None], key).unwrap();
        let london_addr = london.local_addr().unwrap();

        let mut tokyo = UdpMeshNode::bind(
            bind_any,
            NodeRole::Tokyo,
            [Some(london_addr), None],
            key,
        )
        .unwrap();

        let ts_ns = 1_700_000_000_500_000_000u64;
        let sent = tokyo.broadcast_sweep_impulse(
            SIGNAL_CRYPTO_SWEEP,
            AssetPairId::BTC_USDT,
            1,
            25, // 25 bps >= 15 bps threshold
            ts_ns,
            500_000,
        );
        assert_eq!(sent, 1);

        let rx = london
            .poll_incoming(ts_ns + 50_000_000)
            .expect("Expected UDP datagram on London socket")
            .expect("Expected valid BLAKE3 MAC");
        let mag = u16::from_le(rx.magnitude_bps);
        assert_eq!(mag, 25);
        assert_eq!(rx.signal_type, SIGNAL_CRYPTO_SWEEP);

        // Replaying the exact same packet must be rejected as ReplayedSequence
        let raw_replayed = rx.to_bytes();
        let socket = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        socket.send_to(&raw_replayed, london_addr).unwrap();

        let replay_err = london
            .poll_incoming(ts_ns + 60_000_000)
            .expect("Expected datagram")
            .unwrap_err();
        assert_eq!(
            replay_err,
            FrameVerifyError::ReplayedSequence {
                last_seen: 1,
                received: 1
            }
        );
    }

    #[test]
    fn test_zero_copy_ws_frame_codec() {
        let payload = b"{\"e\":\"depthUpdate\",\"U\":100,\"u\":105}";
        // Build unmasked server text frame
        let mut server_frame = [0u8; 64];
        server_frame[0] = 0x81; // FIN + Text
        server_frame[1] = payload.len() as u8;
        server_frame[2..2 + payload.len()].copy_from_slice(payload);

        let decoded = WsStreamManager::decode_server_frame(&server_frame[..2 + payload.len()])
            .expect("Valid server frame");
        assert!(decoded.fin);
        assert_eq!(decoded.opcode, WsOpcode::Text);
        assert_eq!(decoded.payload, payload);
    }
}
