//! serde-framed peer wire messages: hello, heartbeat, and gossip payloads.

use crate::handshake::NodeHello;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum PeerWireMsg {
    Hello {
        node_hello: NodeHello,
    },
    HelloAck {
        accepted: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        node_hello: Option<NodeHello>,
    },
    Heartbeat {
        unix_ms: u64,
        #[serde(default)]
        chain_tip_height: Option<u64>,
        #[serde(default)]
        federation_shard_id: Option<String>,
        /// Trusted-session only: relay snapshot of peer's federation table (see `FedGossipWireRow`).
        #[serde(default)]
        federation_gossip: Option<Vec<crate::federation::FedGossipWireRow>>,
    },
    HeartbeatAck {
        unix_ms: u64,
    },
    CrossShardFacts {
        facts: Vec<crate::ledger::CrossShardFact>,
    },
    AccountViews {
        rows: Vec<crate::state::PeerAccountViewWire>,
    },
}

pub(crate) async fn write_wire_msg(
    stream: &mut tokio::net::TcpStream,
    msg: &PeerWireMsg,
    timeout_ms: u64,
) -> Result<(), String> {
    let payload = serde_json::to_vec(msg).map_err(|e| format!("wire_encode_failed: {e}"))?;
    let len = u32::try_from(payload.len()).map_err(|_| "wire_payload_too_large".to_string())?;
    let mut framed = Vec::with_capacity(4 + payload.len());
    framed.extend_from_slice(&len.to_be_bytes());
    framed.extend_from_slice(&payload);
    tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms.max(1)),
        tokio::io::AsyncWriteExt::write_all(stream, &framed),
    )
    .await
    .map_err(|_| "wire_write_timeout".to_string())?
    .map_err(|e| format!("wire_write_failed: {e}"))
}

pub(crate) async fn read_wire_msg(
    stream: &mut tokio::net::TcpStream,
    timeout_ms: u64,
) -> Result<PeerWireMsg, String> {
    let mut len_buf = [0u8; 4];
    tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms.max(1)),
        tokio::io::AsyncReadExt::read_exact(stream, &mut len_buf),
    )
    .await
    .map_err(|_| "wire_read_len_timeout".to_string())?
    .map_err(|e| format!("wire_read_len_failed: {e}"))?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len == 0 || len > 1024 * 1024 {
        return Err(format!("wire_invalid_frame_len={len}"));
    }
    let mut payload = vec![0u8; len];
    tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms.max(1)),
        tokio::io::AsyncReadExt::read_exact(stream, &mut payload),
    )
    .await
    .map_err(|_| "wire_read_payload_timeout".to_string())?
    .map_err(|e| format!("wire_read_payload_failed: {e}"))?;
    decode_wire_msg_payload(&payload)
}

pub(crate) fn decode_wire_msg_payload(payload: &[u8]) -> Result<PeerWireMsg, String> {
    serde_json::from_slice::<PeerWireMsg>(payload).map_err(|e| format!("wire_decode_failed: {e}"))
}
