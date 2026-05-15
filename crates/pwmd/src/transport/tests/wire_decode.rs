//! serde_json decoding checks for peer wire payloads (`PeerWireMsg`).

use super::super::*;
use serde_json::json;

/// serde_json decoding keeps legacy decimal-string `u128` account views payloads valid.
#[test]
fn decode_acct_views_u128_ok() {
    let payload = serde_json::to_vec(&json!({
        "type": "account_views",
        "rows": [{
            "id": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            "domain_hi": 16,
            "balance_pwm": "340282366920938463463374607431768211455",
            "initialized": true,
            "nonce": 9,
            "observed_at_ms": 123
        }]
    }))
    .expect("encode account views payload");

    match decode_wire_msg_payload(&payload).expect("decode account views payload") {
        PeerWireMsg::AccountViews { rows } => {
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].balance_pwm, u128::MAX);
        }
        other => panic!("unexpected frame: {other:?}"),
    }
}

/// serde_json decoding accepts canonical `0x...` u128 strings in account views payloads.
#[test]
fn dec_acct_u128_hex_ok() {
    let payload = serde_json::to_vec(&json!({
        "type": "account_views",
        "rows": [{
            "id": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            "domain_hi": 16,
            "balance_pwm": "0xffffffffffffffffffffffffffffffff",
            "initialized": true,
            "nonce": 9,
            "observed_at_ms": 123
        }]
    }))
    .expect("encode account views payload");

    match decode_wire_msg_payload(&payload).expect("decode account views payload") {
        PeerWireMsg::AccountViews { rows } => {
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].balance_pwm, u128::MAX);
        }
        other => panic!("unexpected frame: {other:?}"),
    }
}

/// serde_json decoding accepts PeerWireMsg::CrossShardFacts with non-empty u128 amount fields.
#[test]
fn decode_xshard_facts_u128_ok() {
    let payload = serde_json::to_vec(&json!({
        "type": "cross_shard_facts",
        "facts": [{
            "export_id": [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            "source_domain_hi": 16,
            "target_domain_hi": 32,
            "amount": 42,
            "status": "exported",
            "first_height": 1,
            "last_height": 2,
            "to": [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]
        }]
    }))
    .expect("encode cross shard payload");

    match decode_wire_msg_payload(&payload).expect("decode cross shard payload") {
        PeerWireMsg::CrossShardFacts { facts } => {
            assert_eq!(facts.len(), 1);
            assert_eq!(facts[0].amount, 42u128);
        }
        other => panic!("unexpected frame: {other:?}"),
    }
}

/// serde_json decoding accepts canonical `0x...` u128 strings in cross-shard facts payloads.
#[test]
fn dec_xfact_u128_hex_ok() {
    let payload = serde_json::to_vec(&json!({
        "type": "cross_shard_facts",
        "facts": [{
            "export_id": [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            "source_domain_hi": 16,
            "target_domain_hi": 32,
            "amount": "0xffffffffffffffffffffffffffffffff",
            "status": "exported",
            "first_height": 1,
            "last_height": 2,
            "to": [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]
        }]
    }))
    .expect("encode cross shard payload");

    match decode_wire_msg_payload(&payload).expect("decode cross shard payload") {
        PeerWireMsg::CrossShardFacts { facts } => {
            assert_eq!(facts.len(), 1);
            assert_eq!(facts[0].amount, u128::MAX);
        }
        other => panic!("unexpected frame: {other:?}"),
    }
}

/// serde_json encoding emits canonical `0x...` for large u128 wire amounts.
#[test]
fn enc_xfact_u128_hex() {
    let msg = PeerWireMsg::CrossShardFacts {
        facts: vec![crate::ledger::CrossShardFact {
            export_id: [0xAB; 32],
            source_domain_hi: 0x10,
            target_domain_hi: 0x20,
            amount: u128::MAX,
            status: crate::ledger::CrossShardStatus::Exported,
            first_height: 1,
            last_height: 1,
            source: None,
            to: [0xCD; 32],
            intent_id: None,
            origin: crate::ledger::CrossShardOrigin::Local,
        }],
    };
    let payload = serde_json::to_value(&msg).expect("serialize wire msg");
    assert_eq!(
        payload["facts"][0]["amount"],
        "0xffffffffffffffffffffffffffffffff"
    );
}

/// serde_json decoding rejects negative u128 amounts inside cross_shard_facts payloads.
#[test]
fn decode_rejects_neg_u128() {
    let payload = serde_json::to_vec(&json!({
        "type": "cross_shard_facts",
        "facts": [{
            "export_id": [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            "source_domain_hi": 16,
            "target_domain_hi": 32,
            "amount": -1,
            "status": "exported",
            "first_height": 1,
            "last_height": 2,
            "to": [2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]
        }]
    }))
    .expect("encode negative payload");

    let err = decode_wire_msg_payload(&payload).expect_err("negative u128 must fail");
    assert!(err.contains("wire_decode_failed"));
    assert!(err.contains("u128"));
}

/// serde_json decoding accepts sync-v1 headers request skeleton frames.
#[test]
fn decode_sync_headers_req_ok() {
    let payload = serde_json::to_vec(&json!({
        "type": "sync_headers_req",
        "hdr": {
            "shard_id": 16,
            "peer_session_id": "sess-a",
            "seq_no": 7,
            "timestamp_ms": 12345
        },
        "from_height": 100,
        "limit": 32
    }))
    .expect("encode sync headers req");

    match decode_wire_msg_payload(&payload).expect("decode sync headers req") {
        PeerWireMsg::SyncHeadersReq {
            hdr,
            from_height,
            limit,
        } => {
            assert_eq!(hdr.shard_id, 16);
            assert_eq!(hdr.peer_session_id, "sess-a");
            assert_eq!(from_height, 100);
            assert_eq!(limit, 32);
        }
        other => panic!("unexpected frame: {other:?}"),
    }
}

/// serde_json decoding accepts sync tip announce frames.
#[test]
fn decode_sync_tip_ok() {
    let payload = serde_json::to_vec(&json!({
        "type": "sync_tip_announce",
        "hdr": {
            "shard_id": 16,
            "peer_session_id": "sess-a",
            "seq_no": 8,
            "timestamp_ms": 12346
        },
        "head_height": 101,
        "head_hash": "11".repeat(32),
        "finalized_height": 100
    }))
    .expect("encode sync tip");

    match decode_wire_msg_payload(&payload).expect("decode sync tip") {
        PeerWireMsg::SyncTipAnnounce {
            hdr,
            head_height,
            head_hash,
            finalized_height,
            finalized_hash,
        } => {
            assert_eq!(hdr.shard_id, 16);
            assert_eq!(head_height, 101);
            assert_eq!(head_hash, "11".repeat(32));
            assert_eq!(finalized_height, 100);
            assert_eq!(finalized_hash, None);
        }
        other => panic!("unexpected frame: {other:?}"),
    }
}

/// serde_json decoding accepts epoch catch-up chunk frames.
#[test]
fn decode_sync_cup_chunk_ok() {
    let payload = serde_json::to_vec(&json!({
        "type": "sync_catchup_chunk",
        "hdr": {
            "shard_id": 16,
            "peer_session_id": "sess-cup",
            "seq_no": 13,
            "timestamp_ms": 12350
        },
        "chunk": {
            "epoch_id": 1,
            "chunk_index": 0,
            "first_prev_hash": "aa".repeat(32),
            "last_hash": "bb".repeat(32),
            "headers": [{
                "height": 1001,
                "hash": "bb".repeat(32),
                "prev_hash": "aa".repeat(32)
            }],
            "blocks": [{
                "height": 1001,
                "hash": "bb".repeat(32),
                "block": null
            }]
        }
    }))
    .expect("encode sync catchup chunk");

    match decode_wire_msg_payload(&payload).expect("decode sync catchup chunk") {
        PeerWireMsg::SyncCatchupChunk { chunk, .. } => {
            assert_eq!(chunk.epoch_id, 1);
            assert_eq!(chunk.chunk_index, 0);
            assert_eq!(chunk.headers.len(), 1);
            assert_eq!(chunk.blocks.len(), 1);
        }
        other => panic!("unexpected frame: {other:?}"),
    }
}

/// serde_json decoding keeps legacy hello payloads valid without sync_profile field.
#[test]
fn decode_legacy_hello_ok() {
    let payload = serde_json::to_vec(&json!({
        "type": "hello",
        "node_hello": {
            "network_id": "testnet-qa",
            "genesis_hash": "aa",
            "cluster": { "domain_hi": 16, "cluster_id": "c1" },
            "node": { "node_id": "n1", "pubkey": [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0] },
            "capabilities": {
                "protocol_version": "0.1.0",
                "tx_features": ["local_transfer_v1"],
                "services": ["mempool", "sync"]
            },
            "nonce": [1,2,3],
            "timestamp_ms": 77,
            "signature": [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]
        }
    }))
    .expect("encode legacy hello");

    match decode_wire_msg_payload(&payload).expect("decode legacy hello") {
        PeerWireMsg::Hello { node_hello } => {
            assert!(node_hello.capabilities.sync_profile.is_none());
            assert_eq!(
                node_hello.capabilities.sync_mode(),
                crate::handshake::SyncMode::LegacyObserve
            );
        }
        other => panic!("unexpected frame: {other:?}"),
    }
}

/// serde_json decoding accepts RFC16 cluster propose wire messages.
#[test]
fn decode_cluster_propose_ok() {
    let payload = serde_json::to_vec(&json!({
        "type": "cluster_propose",
        "msg": {
            "height": 42,
            "round": 1,
            "vote_object": "vo1:aa",
            "candidate_hash": "aa".repeat(32),
            "candidate_ref": "prev:bb"
        }
    }))
    .expect("encode cluster propose");
    match decode_wire_msg_payload(&payload).expect("decode cluster propose") {
        PeerWireMsg::ClusterPropose { msg } => {
            assert_eq!(msg.height, 42);
            assert_eq!(msg.round, 1);
            assert_eq!(msg.vote_object, "vo1:aa");
            assert_eq!(msg.candidate_ref.as_deref(), Some("prev:bb"));
        }
        other => panic!("unexpected frame: {other:?}"),
    }
}

/// serde_json roundtrip keeps RFC16 cluster attest payload binding fields intact.
#[test]
fn roundtrip_cluster_attest_ok() {
    let payload = serde_json::to_vec(&json!({
        "type": "cluster_attest",
        "msg": {
            "height": 43,
            "round": 2,
            "vote_object": "vo1:cc",
            "candidate_hash": "cc".repeat(32),
            "signature": "sig-01",
            "candidate_ref": "prev:dd"
        }
    }))
    .expect("encode cluster attest");
    let decoded = decode_wire_msg_payload(&payload).expect("decode cluster attest");
    match decoded {
        PeerWireMsg::ClusterAttest { msg } => {
            assert_eq!(msg.height, 43);
            assert_eq!(msg.round, 2);
            assert_eq!(msg.vote_object, "vo1:cc");
            assert_eq!(msg.candidate_hash, "cc".repeat(32));
            assert_eq!(msg.signature, "sig-01");
            assert_eq!(msg.candidate_ref.as_deref(), Some("prev:dd"));
        }
        other => panic!("unexpected frame: {other:?}"),
    }
}
