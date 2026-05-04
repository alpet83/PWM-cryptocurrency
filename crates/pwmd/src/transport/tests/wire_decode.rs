//! serde_json decoding checks for peer wire payloads (`PeerWireMsg`).

use super::super::*;
use serde_json::json;

/// serde_json decoding accepts PeerWireMsg::AccountViews rows with canonical non-empty u128 strings.
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
