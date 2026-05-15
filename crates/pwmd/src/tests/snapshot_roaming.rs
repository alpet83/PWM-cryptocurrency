//! Snapshot serde tests spanning roaming pools and cross-shard rows.

use super::helpers::*;
use super::prelude::*;

#[test]
fn snapshot_roundtrip_blocks_and_state() {
    fn assert_hex_string(value: &serde_json::Value, len: usize, field: &str) {
        let s = value
            .as_str()
            .unwrap_or_else(|| panic!("{field} must be a string"));
        assert_eq!(s.len(), len, "{field} length");
        assert!(
            s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')),
            "{field} must be lowercase hex"
        );
    }

    let (cfg, sks) = dev_net();
    let mut chain = Chain::boot(cfg.clone(), sks);
    chain.seal(vec![]).expect("seal #1");
    chain.seal(vec![]).expect("seal #2");
    let inner = Inner {
        chain,
        pool: Mpool::new(16),
        roaming_pool: crate::roaming::RoamingPool::default(),
        cross_shard: crate::ledger::CrossShardLedger::default(),
        federation: Default::default(),
        peer_account_views: std::collections::HashMap::new(),
        recent_flow: std::collections::VecDeque::new(),
    };
    let p = temp_path("snapshot_roundtrip");
    save_snapshot(&p, &inner).expect("save");
    let raw = std::fs::read_to_string(&p).expect("read");
    let v: serde_json::Value = serde_json::from_str(&raw).expect("json");
    assert_eq!(v["version"].as_u64(), Some(SNAPSHOT_VERSION as u64));
    assert_hex_string(&v["genesis_accounts"][0]["acct"], 64, "genesis acct");
    assert_hex_string(&v["genesis_accounts"][0]["pubkey"], 64, "genesis pubkey");
    assert_hex_string(&v["blocks"][0]["hdr"]["prev_hash"], 64, "block prev_hash");
    assert_hex_string(&v["blocks"][0]["hdr"]["tx_root"], 64, "block tx_root");
    assert_hex_string(&v["blocks"][0]["hdr"]["state_root"], 64, "block state_root");
    assert_hex_string(&v["blocks"][0]["hdr"]["sig"], 128, "block sig");
    assert_hex_string(&v["state"]["accounts"][0]["id"], 64, "state account id");
    assert_hex_string(
        &v["state"]["accounts"][0]["account"]["signing_pubkey"],
        64,
        "state signing pubkey",
    );
    assert!(v["state"]["fee_pool"].as_str().is_some());
    assert!(v["state"]["accounts"][0]["account"]["balance_pwm"]
        .as_str()
        .is_some());
    let snap = load_snapshot(&p, &cfg).expect("load").expect("exists");
    assert_eq!(snap.blocks.len(), 2);
    assert_eq!(digest(&snap.state), digest(&inner.chain.st));
    let _ = std::fs::remove_file(&p);
}

/// Snapshot roundtrip keeps CrossShardLedger summary counters after save/load.
#[test]
fn snap_rt_xshard_ok() {
    let (cfg, sks) = dev_net();
    let chain = Chain::boot(cfg.clone(), sks);
    let mut cross_shard = crate::ledger::CrossShardLedger::default();
    cross_shard.record_handoff(
        [0xCD; 32],
        0x10,
        fake_account_id_with_domain(0x1001),
        fake_account_id_with_domain(0x2001),
        0x2001,
        77,
        4,
        None,
    );
    let inner = Inner {
        chain,
        pool: Mpool::new(16),
        roaming_pool: crate::roaming::RoamingPool::default(),
        cross_shard,
        federation: Default::default(),
        peer_account_views: std::collections::HashMap::new(),
        recent_flow: std::collections::VecDeque::new(),
    };
    let p = temp_path("snapshot_cross_shard_summary");
    save_snapshot(&p, &inner).expect("save");
    let snap = load_snapshot(&p, &cfg).expect("load").expect("exists");
    let summary = snap.cross_shard.summary();
    assert_eq!(summary.total_exported_count, 1);
    assert_eq!(summary.total_exported_amount, 77);
    assert_eq!(summary.pending_count, 1);
    let _ = std::fs::remove_file(&p);
}

/// Snapshot reload preserves balances after sealing a transfer to an initialized recipient account.
#[test]
fn snap_rt_xfer_init_rcv() {
    let (cfg, sks) = dev_net();
    let mut chain = Chain::boot(cfg.clone(), sks);

    let sk_v = chain.val_sks[0].clone();
    let aid_v = cfg.accounts[0].acct;
    let sender_dom = domain_of_account_id(&aid_v);
    let (sk_r, i_r, recipient) =
        user_sk_matching_domain_hi([0x51; 32], sender_dom.to_be_bytes()[0]);
    let recipient_dom = domain_of_account_id(&recipient);
    let init = SignedTx::sign_body(
        &sk_r,
        recipient_dom,
        i_r,
        0,
        TxBody::Init { index: 0, flags: 0 },
    );
    chain.seal(vec![init]).expect("seal recipient init");

    let tx = SignedTx::sign_body(
        &sk_v,
        sender_dom,
        0,
        0,
        TxBody::Transfer {
            to: recipient,
            amount: 10,
            fee: 1,
        },
    );
    chain.seal(vec![tx]).expect("seal transfer");

    let inner = Inner {
        chain,
        pool: Mpool::new(16),
        roaming_pool: crate::roaming::RoamingPool::default(),
        cross_shard: crate::ledger::CrossShardLedger::default(),
        federation: Default::default(),
        peer_account_views: std::collections::HashMap::new(),
        recent_flow: std::collections::VecDeque::new(),
    };
    let p = temp_path("snapshot_transfer_initialized_recipient");
    save_snapshot(&p, &inner).expect("save");
    let snap = load_snapshot(&p, &cfg).expect("load").expect("exists");
    assert_eq!(
        snap.state.get(&recipient).expect("recipient").balance_pwm,
        10
    );
    let _ = std::fs::remove_file(&p);
}

/// Snapshot reload retains export provenance after an export-only sealed transaction.
#[test]
fn snap_rt_exp_only_ok() {
    let (cfg, sks) = dev_net();
    let mut chain = Chain::boot(cfg.clone(), sks);

    let sk_v = &chain.val_sks[0];
    let aid_v = cfg.accounts[0].acct;
    let sender_dom = domain_of_account_id(&aid_v);
    let sender_hi = sender_dom.to_be_bytes()[0];
    let target_hi = sender_hi.wrapping_add(1);
    let target_domain: u16 = ((target_hi as u16) << 8) | 0x01;
    let recipient = fake_account_id_with_domain(target_domain);
    assert_ne!(
        domain_of_account_id(&recipient).to_be_bytes()[0],
        sender_hi,
        "export must target different hi-byte domain"
    );

    let tx = SignedTx::sign_body(
        sk_v,
        sender_dom,
        0,
        0,
        TxBody::Export {
            to: recipient,
            target_domain,
            amount: 10,
            fee: 1,
        },
    );
    let export_id = tx.export_id().expect("export id");
    chain.seal(vec![tx]).expect("seal export");

    let inner = Inner {
        chain,
        pool: Mpool::new(16),
        roaming_pool: crate::roaming::RoamingPool::default(),
        cross_shard: crate::ledger::CrossShardLedger::default(),
        federation: Default::default(),
        peer_account_views: std::collections::HashMap::new(),
        recent_flow: std::collections::VecDeque::new(),
    };
    let p = temp_path("snapshot_export_only");
    save_snapshot(&p, &inner).expect("save");
    let snap = load_snapshot(&p, &cfg).expect("load").expect("exists");
    assert!(
        snap.state.exported_registry.contains_key(&export_id),
        "export provenance must survive snapshot reload"
    );
    let _ = std::fs::remove_file(&p);
}

/// Snapshot restores imported_set replay guards and exported_registry provenance for duplicate imports.
#[test]
fn snap_rt_imp_guard_pv() {
    let (cfg, sks) = dev_net();
    let mut chain = Chain::boot(cfg.clone(), sks);
    let sk_v = &chain.val_sks[0];
    let aid_v = cfg.accounts[0].acct;
    let dom_v = domain_of_account_id(&aid_v);
    let (sk_b, i_b, aid_b) = user_sk(&[0x44; 32]);
    let dom_b = domain_of_account_id(&aid_b);

    let init = SignedTx::sign_body(&sk_b, dom_b, i_b, 0, TxBody::Init { index: 0, flags: 0 });
    let xfer = SignedTx::sign_body(
        sk_v,
        dom_v,
        0,
        0,
        TxBody::Transfer {
            to: aid_b,
            amount: pwm_core::tx::MIN_IMPORT_FEE_UNITS,
            fee: 1,
        },
    );
    let export = SignedTx::sign_body(
        sk_v,
        dom_v,
        0,
        1,
        TxBody::Export {
            to: aid_b,
            target_domain: dom_b,
            amount: 31,
            fee: 1,
        },
    );
    let export_id = export.export_id().expect("export id");
    let import = SignedTx::sign_body(
        &sk_b,
        dom_b,
        i_b,
        1,
        TxBody::Import {
            to: aid_b,
            amount: 31,
            export_id,
        },
    );
    chain
        .seal(vec![init, xfer, export, import])
        .expect("seal import flow");

    let inner = Inner {
        chain,
        pool: Mpool::new(16),
        roaming_pool: crate::roaming::RoamingPool::default(),
        cross_shard: crate::ledger::CrossShardLedger::default(),
        federation: Default::default(),
        peer_account_views: std::collections::HashMap::new(),
        recent_flow: std::collections::VecDeque::new(),
    };
    let p = temp_path("snapshot_import_guard_roundtrip");
    save_snapshot(&p, &inner).expect("save");
    let mut snap = load_snapshot(&p, &cfg).expect("load").expect("exists");
    assert!(snap.state.imported_set.contains(&export_id));
    assert!(snap.state.exported_registry.contains_key(&export_id));

    let duplicate = SignedTx::sign_body(
        &sk_b,
        dom_b,
        i_b,
        snap.state.get(&aid_b).expect("restored signer").nonce,
        TxBody::Import {
            to: aid_b,
            amount: 31,
            export_id,
        },
    );
    let err = snap
        .state
        .apply_tx(&duplicate)
        .expect_err("duplicate must be rejected after snapshot restore");
    assert!(matches!(err, TxError::DuplicateImport));
    let _ = std::fs::remove_file(&p);
}

/// Snapshot replay remains deterministic when Import carries embedded provenance.
#[test]
fn snap_rt_handoff_import_ok() {
    use pwm_core::state::ExportProvenance;

    let (cfg, sks) = dev_net();
    let mut chain = Chain::boot(cfg.clone(), sks);
    let sk_v = &chain.val_sks[0];
    let aid_v = cfg.accounts[0].acct;
    let dom_v = domain_of_account_id(&aid_v);
    let (sk_b, i_b, aid_b) = user_sk(&[0x66; 32]);
    let dom_b = domain_of_account_id(&aid_b);

    let init = SignedTx::sign_body(&sk_b, dom_b, i_b, 0, TxBody::Init { index: 7, flags: 0 });
    let xfer = SignedTx::sign_body(
        sk_v,
        dom_v,
        0,
        0,
        TxBody::Transfer {
            to: aid_b,
            amount: pwm_core::tx::MIN_IMPORT_FEE_UNITS,
            fee: 1,
        },
    );

    let export_id = [0xCE; 32];
    let amount = 25u128;
    let mut import = SignedTx::sign_body(
        &sk_b,
        dom_b,
        i_b,
        1,
        TxBody::Import {
            to: aid_b,
            amount,
            export_id,
        },
    );
    import.set_import_provenance_signed(
        &sk_b,
        Some(ExportProvenance {
            to: aid_b,
            target_domain: dom_b,
            amount,
        }),
    );
    chain
        .seal(vec![init, xfer, import])
        .expect("seal init xfer import");

    let inner = Inner {
        chain,
        pool: Mpool::new(16),
        roaming_pool: crate::roaming::RoamingPool::default(),
        cross_shard: crate::ledger::CrossShardLedger::default(),
        federation: Default::default(),
        peer_account_views: std::collections::HashMap::new(),
        recent_flow: std::collections::VecDeque::new(),
    };
    let p = temp_path("snapshot_handoff_import_rt");
    save_snapshot(&p, &inner).expect("save");
    load_snapshot(&p, &cfg)
        .expect("validate and load")
        .expect("snapshot exists");
    let _ = std::fs::remove_file(&p);
}

/// Snapshot roundtrip restores roaming intent metadata plus active export locks.
#[test]
fn snap_rt_rov_lock_ok() {
    let (cfg, sks) = dev_net();
    let mut chain = Chain::boot(cfg.clone(), sks.clone());
    let sk = &sks[0];
    let source = cfg.accounts[0].acct;
    let source_dom = domain_of_account_id(&source);
    let recipient = valid_cross_domain_recipient(source_dom.to_be_bytes()[0]);
    let target_domain = domain_of_account_id(&recipient);
    let export = SignedTx::sign_body(
        sk,
        source_dom,
        0,
        0,
        TxBody::Export {
            to: recipient,
            target_domain,
            amount: 23,
            fee: 1,
        },
    );
    chain.seal(vec![]).expect("seal");
    let mut roaming_pool = crate::roaming::RoamingPool::default();
    let (intent_id, duplicate) = roaming_pool
        .register_export(&export, chain.tip_h(), 5)
        .expect("register export intent");
    assert!(!duplicate);
    roaming_pool.mark_exported(intent_id);
    let inner = Inner {
        chain,
        pool: Mpool::new(16),
        roaming_pool,
        cross_shard: crate::ledger::CrossShardLedger::default(),
        federation: Default::default(),
        peer_account_views: std::collections::HashMap::new(),
        recent_flow: std::collections::VecDeque::new(),
    };
    let p = temp_path("snapshot_roaming_roundtrip");
    save_snapshot(&p, &inner).expect("save");
    let snap = load_snapshot(&p, &cfg).expect("load").expect("exists");
    let (_, _, restored_roaming, _) = snap.into_runtime().expect("restore roaming");
    let restored = restored_roaming.get(&intent_id).expect("restored intent");
    assert_eq!(restored.status, crate::roaming::IntentStatus::Exported);
    assert_eq!(restored.export_id, export.export_id().expect("export id"));
    let lock = restored_roaming
        .active_locks_snapshot()
        .into_iter()
        .find(|(acct, _)| acct == &source)
        .expect("restored lock");
    assert_eq!(lock.1, intent_id);
    let _ = std::fs::remove_file(&p);
}

/// Snapshot preserves relayed roaming intent status and matching account locks.
#[test]
fn snap_rt_rov_rel_ok() {
    let (cfg, sks) = dev_net();
    let mut chain = Chain::boot(cfg.clone(), sks.clone());
    let sk = &sks[0];
    let source = cfg.accounts[0].acct;
    let source_dom = domain_of_account_id(&source);
    let recipient = valid_cross_domain_recipient(source_dom.to_be_bytes()[0]);
    let target_domain = domain_of_account_id(&recipient);
    let export = SignedTx::sign_body(
        sk,
        source_dom,
        0,
        0,
        TxBody::Export {
            to: recipient,
            target_domain,
            amount: 41,
            fee: 1,
        },
    );
    chain.seal(vec![]).expect("seal");
    let mut roaming_pool = crate::roaming::RoamingPool::default();
    let (intent_id, duplicate) = roaming_pool
        .register_export(&export, chain.tip_h(), 5)
        .expect("register export intent");
    assert!(!duplicate);
    roaming_pool.mark_exported(intent_id);
    roaming_pool.mark_relayed(intent_id);
    let inner = Inner {
        chain,
        pool: Mpool::new(16),
        roaming_pool,
        cross_shard: crate::ledger::CrossShardLedger::default(),
        federation: Default::default(),
        peer_account_views: std::collections::HashMap::new(),
        recent_flow: std::collections::VecDeque::new(),
    };
    let p = temp_path("snapshot_roaming_relayed_roundtrip");
    save_snapshot(&p, &inner).expect("save");
    let snap = load_snapshot(&p, &cfg).expect("load").expect("exists");
    let (_, _, restored_roaming, _) = snap.into_runtime().expect("restore roaming");
    let restored = restored_roaming.get(&intent_id).expect("restored intent");
    assert_eq!(restored.status, crate::roaming::IntentStatus::Relayed);
    let lock = restored_roaming
        .active_locks_snapshot()
        .into_iter()
        .find(|(acct, _)| acct == &source)
        .expect("restored lock");
    assert_eq!(lock.1, intent_id);
    let _ = std::fs::remove_file(&p);
}

#[test]
fn snapshot_rejects_mismatched_genesis() {
    let (cfg, sks) = dev_net();
    let chain = Chain::boot(cfg.clone(), sks);
    let inner = Inner {
        chain,
        pool: Mpool::new(16),
        roaming_pool: crate::roaming::RoamingPool::default(),
        cross_shard: crate::ledger::CrossShardLedger::default(),
        federation: Default::default(),
        peer_account_views: std::collections::HashMap::new(),
        recent_flow: std::collections::VecDeque::new(),
    };
    let p = temp_path("snapshot_mismatch");
    save_snapshot(&p, &inner).expect("save");
    let mut bad_cfg = cfg.clone();
    bad_cfg.funding.accounts[0].der_idx += 1;
    bad_cfg.accounts = bad_cfg.funding.accounts.clone();
    let err = load_snapshot(&p, &bad_cfg).expect_err("must reject mismatched genesis");
    assert!(err.contains("snapshot genesis mismatch"));
    let _ = std::fs::remove_file(&p);
}

/// Legacy schema v0 snapshots migrate forward without losing critical chain fields.
#[test]
fn snap_v0_legacy_mig_ok() {
    let (cfg, sks) = dev_net();
    let mut chain = Chain::boot(cfg.clone(), sks);
    chain.seal(vec![]).expect("seal");
    let legacy = SnapshotData {
        version: 1,
        genesis_accounts: snapshot_genesis_accounts(&cfg),
        blocks: chain.blocks.iter().cloned().collect(),
        state: chain.st.clone(),
        roaming: SnapshotRoamingWire::default(),
        cross_shard: crate::ledger::CrossShardLedger::default(),
        blocks_stored: BlocksStored::Inline,
        checkpoint_height: 0,
    };
    let inner = Inner {
        chain,
        pool: Mpool::new(16),
        roaming_pool: crate::roaming::RoamingPool::default(),
        cross_shard: crate::ledger::CrossShardLedger::default(),
        federation: Default::default(),
        peer_account_views: std::collections::HashMap::new(),
        recent_flow: std::collections::VecDeque::new(),
    };
    let p = temp_path("snapshot_legacy_v0");
    let mut v: serde_json::Value =
        serde_json::from_str(&serde_json::to_string_pretty(&legacy).expect("encode"))
            .expect("json");
    let o = v.as_object_mut().expect("object");
    o.remove("version");
    o.remove("genesis_accounts");
    o.insert(
        "hints".to_string(),
        serde_json::json!({"operator": "ignore-me"}),
    );
    std::fs::write(&p, serde_json::to_string_pretty(&v).expect("encode")).expect("write");
    let snap = load_snapshot(&p, &cfg).expect("load").expect("exists");
    assert_eq!(snap.version, SNAPSHOT_VERSION);
    assert_eq!(snap.genesis_accounts, snapshot_genesis_accounts(&cfg));
    assert_eq!(snap.blocks.len(), 1);
    save_snapshot(&p, &inner).expect("save v2");
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&p).expect("read v2")).expect("json");
    assert_eq!(saved["version"].as_u64(), Some(SNAPSHOT_VERSION as u64));
    assert!(saved["blocks"][0]["hdr"]["prev_hash"].as_str().is_some());
    let _ = std::fs::remove_file(&p);
}

/// v1-format snapshots load successfully and re-save using the current SNAPSHOT_VERSION encoding.
#[test]
fn snap_v1_read_save_v2() {
    let (cfg, sks) = dev_net();
    let mut chain = Chain::boot(cfg.clone(), sks);
    chain.seal(vec![]).expect("seal");
    let legacy = SnapshotData {
        version: 1,
        genesis_accounts: snapshot_genesis_accounts(&cfg),
        blocks: chain.blocks.iter().cloned().collect(),
        state: chain.st.clone(),
        roaming: SnapshotRoamingWire::default(),
        cross_shard: crate::ledger::CrossShardLedger::default(),
        blocks_stored: BlocksStored::Inline,
        checkpoint_height: 0,
    };
    let inner = Inner {
        chain,
        pool: Mpool::new(16),
        roaming_pool: crate::roaming::RoamingPool::default(),
        cross_shard: crate::ledger::CrossShardLedger::default(),
        federation: Default::default(),
        peer_account_views: std::collections::HashMap::new(),
        recent_flow: std::collections::VecDeque::new(),
    };
    let p = temp_path("snapshot_v1_to_v2");
    std::fs::write(&p, serde_json::to_string_pretty(&legacy).expect("encode")).expect("write");
    let snap = load_snapshot(&p, &cfg).expect("load").expect("exists");
    assert_eq!(snap.version, SNAPSHOT_VERSION);
    save_snapshot(&p, &inner).expect("save v2");
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&p).expect("read")).expect("json");
    assert_eq!(saved["version"].as_u64(), Some(SNAPSHOT_VERSION as u64));
    assert!(saved["genesis_accounts"][0]["acct"].as_str().is_some());
    assert!(saved["state"]["accounts"][0]["account"]["balance_pwm"]
        .as_str()
        .is_some());
    let _ = std::fs::remove_file(&p);
}

/// v2 snapshot parsing rejects malformed hex strings and invalid decimal encodings.
#[test]
fn snap_v2_bad_num_fail() {
    let (cfg, sks) = dev_net();
    let mut chain = Chain::boot(cfg.clone(), sks);
    chain.seal(vec![]).expect("seal");
    let inner = Inner {
        chain,
        pool: Mpool::new(16),
        roaming_pool: crate::roaming::RoamingPool::default(),
        cross_shard: crate::ledger::CrossShardLedger::default(),
        federation: Default::default(),
        peer_account_views: std::collections::HashMap::new(),
        recent_flow: std::collections::VecDeque::new(),
    };
    let p = temp_path("snapshot_v2_bad_hex_dec");
    save_snapshot(&p, &inner).expect("save");
    let raw = std::fs::read_to_string(&p).expect("read");

    for (bad, want) in [
        (
            "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF",
            "lowercase",
        ),
        ("abc", "invalid hex length"),
        (
            "0x0000000000000000000000000000000000000000000000000000000000000000",
            "invalid hex length",
        ),
    ] {
        let mut v: serde_json::Value = serde_json::from_str(&raw).expect("json");
        v["genesis_accounts"][0]["acct"] = serde_json::json!(bad);
        std::fs::write(&p, serde_json::to_string_pretty(&v).expect("encode"))
            .expect("write bad hex");
        let err = load_snapshot(&p, &cfg).expect_err("must reject bad hex");
        assert!(err.contains("genesis_accounts[0].acct"), "{err}");
        assert!(err.contains(want), "{err}");
    }

    for (bad, want) in [
        ("+1", "digits only"),
        ("01", "leading zeros"),
        ("340282366920938463463374607431768211456", "invalid u128"),
    ] {
        let mut v: serde_json::Value = serde_json::from_str(&raw).expect("json");
        v["state"]["fee_pool"] = serde_json::json!(bad);
        std::fs::write(&p, serde_json::to_string_pretty(&v).expect("encode"))
            .expect("write bad dec");
        let err = load_snapshot(&p, &cfg).expect_err("must reject bad decimal");
        assert!(err.contains("state.fee_pool"), "{err}");
        assert!(err.contains(want), "{err}");
    }
    let _ = std::fs::remove_file(&p);
}

/// Snapshot load rejects non-canonical incomplete-contract rows that violate allowed forms.
#[test]
fn snap_bad_ctr_nc() {
    let (cfg, sks) = dev_net();
    let chain = Chain::boot(cfg.clone(), sks);
    let inner = Inner {
        chain,
        pool: Mpool::new(16),
        roaming_pool: crate::roaming::RoamingPool::default(),
        cross_shard: crate::ledger::CrossShardLedger::default(),
        federation: Default::default(),
        peer_account_views: std::collections::HashMap::new(),
        recent_flow: std::collections::VecDeque::new(),
    };
    let p = temp_path("snapshot_incomplete_contract");
    save_snapshot(&p, &inner).expect("save");
    let raw = std::fs::read_to_string(&p).expect("read");
    let mut v: serde_json::Value = serde_json::from_str(&raw).expect("json");
    v.as_object_mut()
        .expect("object")
        .remove("genesis_accounts");
    std::fs::write(&p, serde_json::to_string_pretty(&v).expect("encode")).expect("write");
    let err = load_snapshot(&p, &cfg).expect_err("must reject");
    assert!(err.contains("canonical snapshot requires both 'version' and 'genesis_accounts'"));
    let _ = std::fs::remove_file(&p);
}

/// Non-canonical derived snapshot fields are ignored without failing the overall load.
#[test]
fn snap_ignore_der_nc() {
    let (cfg, sks) = dev_net();
    let mut chain = Chain::boot(cfg.clone(), sks);
    chain.seal(vec![]).expect("seal");
    let want_state_digest = digest(&chain.st);
    let inner = Inner {
        chain,
        pool: Mpool::new(16),
        roaming_pool: crate::roaming::RoamingPool::default(),
        cross_shard: crate::ledger::CrossShardLedger::default(),
        federation: Default::default(),
        peer_account_views: std::collections::HashMap::new(),
        recent_flow: std::collections::VecDeque::new(),
    };
    let p = temp_path("snapshot_ignores_derived");
    save_snapshot(&p, &inner).expect("save");
    let raw = std::fs::read_to_string(&p).expect("read");
    let mut v: serde_json::Value = serde_json::from_str(&raw).expect("json");
    let o = v.as_object_mut().expect("object");
    o.insert(
        "pretty".to_string(),
        serde_json::json!({"tip": "human-readable-only"}),
    );
    o["state"].as_object_mut().expect("state object").insert(
        "hints".to_string(),
        serde_json::json!({"operator": "ignore-me"}),
    );
    std::fs::write(&p, serde_json::to_string_pretty(&v).expect("encode")).expect("write");
    let snap = load_snapshot(&p, &cfg).expect("load").expect("exists");
    assert_eq!(digest(&snap.state), want_state_digest);
    assert_eq!(snap.blocks.len(), 1);
    let _ = std::fs::remove_file(&p);
}

/// Snapshot validation rejects broken block prev_hash chains.
#[test]
fn snap_reject_prev_hash() {
    let (cfg, sks) = dev_net();
    let mut chain = Chain::boot(cfg.clone(), sks);
    chain.seal(vec![]).expect("seal #1");
    chain.seal(vec![]).expect("seal #2");
    let inner = Inner {
        chain,
        pool: Mpool::new(16),
        roaming_pool: crate::roaming::RoamingPool::default(),
        cross_shard: crate::ledger::CrossShardLedger::default(),
        federation: Default::default(),
        peer_account_views: std::collections::HashMap::new(),
        recent_flow: std::collections::VecDeque::new(),
    };
    let p = temp_path("snapshot_bad_prev");
    save_snapshot(&p, &inner).expect("save");
    let raw = std::fs::read_to_string(&p).expect("read");
    let mut v: serde_json::Value = serde_json::from_str(&raw).expect("json");
    let blocks = v["blocks"].as_array_mut().expect("blocks");
    let bad = blocks[1]["hdr"]["prev_hash"]
        .as_str()
        .expect("prev_hash hex");
    let mut prev_bytes = hex::decode(bad).expect("prev_hash hex decodes");
    // Must always change bytes: prefix `ff` + tail is a no-op when the hash already starts with ff.
    prev_bytes[15] ^= 0xAB;
    blocks[1]["hdr"]["prev_hash"] = serde_json::json!(hex::encode(prev_bytes));
    std::fs::write(&p, serde_json::to_string_pretty(&v).expect("encode")).expect("write");
    let err = load_snapshot(&p, &cfg).expect_err("must reject");
    assert!(err.contains("prev_hash"));
    let _ = std::fs::remove_file(&p);
}

/// Tampered block headers fail snapshot integrity checks.
#[test]
fn snap_bad_hdr() {
    let (cfg, sks) = dev_net();
    let mut chain = Chain::boot(cfg.clone(), sks);
    chain.seal(vec![]).expect("seal #1");
    chain.seal(vec![]).expect("seal #2");
    let inner = Inner {
        chain,
        pool: Mpool::new(16),
        roaming_pool: crate::roaming::RoamingPool::default(),
        cross_shard: crate::ledger::CrossShardLedger::default(),
        federation: Default::default(),
        peer_account_views: std::collections::HashMap::new(),
        recent_flow: std::collections::VecDeque::new(),
    };
    let p = temp_path("snapshot_tampered_header");
    save_snapshot(&p, &inner).expect("save");
    let raw = std::fs::read_to_string(&p).expect("read");
    let mut v: serde_json::Value = serde_json::from_str(&raw).expect("json");
    let bad = v["blocks"][1]["hdr"]["state_root"]
        .as_str()
        .expect("state_root hex");
    v["blocks"][1]["hdr"]["state_root"] = serde_json::json!(format!("2a{}", &bad[2..]));
    std::fs::write(&p, serde_json::to_string_pretty(&v).expect("encode")).expect("write");
    let err = load_snapshot(&p, &cfg).expect_err("must reject");
    assert!(err.contains("invalid producer signature"));
    let _ = std::fs::remove_file(&p);
}

/// Replay mismatch diagnostics include first bad height, tx kind, and likely class.
#[test]
fn snap_diag_first_bad_height() {
    let (cfg, sks) = dev_net();
    let mut chain = Chain::boot(cfg.clone(), sks);
    chain.seal(vec![]).expect("seal #1");
    chain.seal(vec![]).expect("seal #2");
    let inner = Inner {
        chain,
        pool: Mpool::new(16),
        roaming_pool: crate::roaming::RoamingPool::default(),
        cross_shard: crate::ledger::CrossShardLedger::default(),
        federation: Default::default(),
        peer_account_views: std::collections::HashMap::new(),
        recent_flow: std::collections::VecDeque::new(),
    };
    let p = temp_path("snapshot_first_bad_height_diag");
    save_snapshot(&p, &inner).expect("save");
    let mut bad_cfg = cfg.clone();
    bad_cfg.block_reward = bad_cfg.block_reward.saturating_add(1);
    let err = load_snapshot(&p, &bad_cfg).expect_err("must reject replay mismatch");
    assert!(err.contains("first_bad_height=1"), "{err}");
    assert!(err.contains("block_idx=0"), "{err}");
    assert!(err.contains("tx_kind=empty"), "{err}");
    assert!(err.contains("class=state_root_divergence"), "{err}");
    let _ = std::fs::remove_file(&p);
}

/// Duplicate state account ids are rejected during snapshot ingestion.
#[test]
fn snap_reject_dup_acct() {
    let (cfg, sks) = dev_net();
    let mut chain = Chain::boot(cfg.clone(), sks);
    chain.seal(vec![]).expect("seal");
    let inner = Inner {
        chain,
        pool: Mpool::new(16),
        roaming_pool: crate::roaming::RoamingPool::default(),
        cross_shard: crate::ledger::CrossShardLedger::default(),
        federation: Default::default(),
        peer_account_views: std::collections::HashMap::new(),
        recent_flow: std::collections::VecDeque::new(),
    };
    let p = temp_path("snapshot_duplicate_state_ids");
    save_snapshot(&p, &inner).expect("save");
    let raw = std::fs::read_to_string(&p).expect("read");
    let mut v: serde_json::Value = serde_json::from_str(&raw).expect("json");
    let accounts = v["state"]["accounts"]
        .as_array_mut()
        .expect("state.accounts");
    let first = accounts.first().expect("at least one account").clone();
    accounts.push(first);
    std::fs::write(&p, serde_json::to_string_pretty(&v).expect("encode")).expect("write");
    let err = load_snapshot(&p, &cfg).expect_err("must reject duplicate state account ids");
    assert!(err.contains("duplicate account id"));
    let _ = std::fs::remove_file(&p);
}

/// Orphan mark quota identifiers fail snapshot validation.
#[test]
fn snap_or_mk_quota() {
    let (cfg, sks) = dev_net();
    let mut chain = Chain::boot(cfg.clone(), sks);
    chain.seal(vec![]).expect("seal");
    let inner = Inner {
        chain,
        pool: Mpool::new(16),
        roaming_pool: crate::roaming::RoamingPool::default(),
        cross_shard: crate::ledger::CrossShardLedger::default(),
        federation: Default::default(),
        peer_account_views: std::collections::HashMap::new(),
        recent_flow: std::collections::VecDeque::new(),
    };
    let p = temp_path("snapshot_orphan_quota_ids");
    save_snapshot(&p, &inner).expect("save");
    let raw = std::fs::read_to_string(&p).expect("read");
    let mut v: serde_json::Value = serde_json::from_str(&raw).expect("json");
    v["state"]["marks_quota"] = serde_json::json!([]);
    let quota = v["state"]["marks_quota"]
        .as_array_mut()
        .expect("state.marks_quota");
    quota.push(serde_json::json!({
        "id": "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        "quota": "1"
    }));
    std::fs::write(&p, serde_json::to_string_pretty(&v).expect("encode")).expect("write");
    let err = load_snapshot(&p, &cfg).expect_err("must reject orphan marks_quota ids");
    assert!(
        err.contains("marks_quota id"),
        "unexpected error text: {err}"
    );
    let _ = std::fs::remove_file(&p);
}

/// Legacy mark quota rows must mirror account.marks exactly.
#[test]
fn snap_reject_quota_mismatch() {
    let (cfg, sks) = dev_net();
    let mut chain = Chain::boot(cfg.clone(), sks);
    chain.seal(vec![]).expect("seal");
    let inner = Inner {
        chain,
        pool: Mpool::new(16),
        roaming_pool: crate::roaming::RoamingPool::default(),
        cross_shard: crate::ledger::CrossShardLedger::default(),
        federation: Default::default(),
        peer_account_views: std::collections::HashMap::new(),
        recent_flow: std::collections::VecDeque::new(),
    };
    let p = temp_path("snapshot_quota_mismatch");
    save_snapshot(&p, &inner).expect("save");
    let raw = std::fs::read_to_string(&p).expect("read");
    let mut v: serde_json::Value = serde_json::from_str(&raw).expect("json");
    let first_id = v["state"]["accounts"][0]["id"]
        .as_str()
        .expect("state.accounts[0].id")
        .to_string();
    let marks = v["state"]["accounts"][0]["account"]["marks"]
        .as_str()
        .expect("state.accounts[0].account.marks")
        .parse::<u128>()
        .expect("marks u128");
    v["state"]["marks_quota"] = serde_json::json!([{
        "id": first_id,
        "quota": (marks.saturating_add(1)).to_string()
    }]);
    std::fs::write(&p, serde_json::to_string_pretty(&v).expect("encode")).expect("write");
    let err = load_snapshot(&p, &cfg).expect_err("must reject marks_quota mismatch");
    assert!(
        err.contains("marks_quota mismatch"),
        "unexpected error text: {err}"
    );
    let _ = std::fs::remove_file(&p);
}
