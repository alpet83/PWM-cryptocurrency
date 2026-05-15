//! serde-framed peer wire messages: hello, heartbeat, gossip, and sync-v1 skeleton frames.

use crate::handshake::NodeHello;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SyncWireHdr {
    pub shard_id: u8,
    pub peer_session_id: String,
    pub seq_no: u64,
    pub timestamp_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SyncProfileWire {
    pub sync_wire_version: u16,
    pub max_headers_per_msg: u16,
    pub max_blocks_per_msg: u16,
    pub max_txs_per_msg: u16,
    pub supports_epoch_catchup: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SyncHeaderWire {
    pub height: u64,
    pub hash: String,
    pub prev_hash: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SyncBlockWire {
    pub height: u64,
    pub hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<pwm_core::block::Block>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SyncCatchupChunkWire {
    pub epoch_id: u64,
    pub chunk_index: u32,
    pub first_prev_hash: String,
    pub last_hash: String,
    pub headers: Vec<SyncHeaderWire>,
    pub blocks: Vec<SyncBlockWire>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ClusterProposeWire {
    pub height: u64,
    pub round: u32,
    pub vote_object: String,
    pub candidate_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_ref: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ClusterAttestWire {
    pub height: u64,
    pub round: u32,
    pub vote_object: String,
    pub candidate_hash: String,
    pub signature: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_ref: Option<String>,
}

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
        lease_owner_id: Option<String>,
        #[serde(default)]
        lease_term: Option<u64>,
        #[serde(default)]
        lease_expires_at_ms: Option<u64>,
        #[serde(default)]
        lease_last_tip: Option<u64>,
        #[serde(default)]
        lease_fence: Option<u64>,
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
    SyncProfileAnnounce {
        hdr: SyncWireHdr,
        profile: SyncProfileWire,
    },
    SyncTipAnnounce {
        hdr: SyncWireHdr,
        head_height: u64,
        head_hash: String,
        finalized_height: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        finalized_hash: Option<String>,
    },
    SyncHeadersReq {
        hdr: SyncWireHdr,
        from_height: u64,
        limit: u16,
    },
    SyncHeadersBatch {
        hdr: SyncWireHdr,
        headers: Vec<SyncHeaderWire>,
    },
    SyncBlocksReq {
        hdr: SyncWireHdr,
        block_hashes: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        block_heights: Option<Vec<u64>>,
    },
    SyncBlocksBatch {
        hdr: SyncWireHdr,
        blocks: Vec<SyncBlockWire>,
    },
    SyncTxAnnounce {
        hdr: SyncWireHdr,
        tx_ids: Vec<String>,
    },
    SyncTxReq {
        hdr: SyncWireHdr,
        tx_ids: Vec<String>,
    },
    SyncTxBatch {
        hdr: SyncWireHdr,
        txs: Vec<pwm_core::SignedTx>,
    },
    SyncNack {
        hdr: SyncWireHdr,
        reason_code: String,
        retry_after_ms: u32,
    },
    SyncCatchupReq {
        hdr: SyncWireHdr,
        start_height: u64,
        end_height: u64,
        epoch_id: u64,
        anchor_hash: String,
    },
    SyncCatchupChunk {
        hdr: SyncWireHdr,
        chunk: SyncCatchupChunkWire,
    },
    SyncCatchupDone {
        hdr: SyncWireHdr,
        epoch_id: u64,
        last_height: u64,
        last_hash: String,
    },
    ClusterPropose {
        msg: ClusterProposeWire,
    },
    ClusterAttest {
        msg: ClusterAttestWire,
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
