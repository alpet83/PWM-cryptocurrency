//! Shared fixtures for pwmd crate tests (keys, routers, snapshots).

use super::prelude::*;

pub(crate) fn user_sk(seed: &[u8; 32]) -> (SigningKey, u32, pwm_core::AccountId) {
    let sk_bytes = derive_ed25519_private_key(seed, &[0, 0]);
    let sk = SigningKey::from_bytes(&sk_bytes);
    let i = 0u32;
    let pk = sk.verifying_key().to_bytes();
    let aid = account_id_from_parts(&pk, i);
    (sk, i, aid)
}

/// Credits liquid PWM so `Import` can debit `MIN_IMPORT_FEE_UNITS` (v2 import fee floor).
pub(crate) fn credit_min_import_fee_tests(st: &mut pwm_core::State, aid: &pwm_core::AccountId) {
    use pwm_core::tx::MIN_IMPORT_FEE_UNITS;
    let acc = st.accounts.get_mut(aid).expect("acct must exist");
    acc.balance_pwm = acc.balance_pwm.saturating_add(MIN_IMPORT_FEE_UNITS);
}

pub(crate) fn routable_user_sk(mut seed: [u8; 32]) -> (SigningKey, u32, pwm_core::AccountId) {
    for _ in 0..4096 {
        let (sk, i, aid) = user_sk(&seed);
        if shard_for_phase1_account(&aid).is_ok() {
            return (sk, i, aid);
        }
        seed[0] = seed[0].wrapping_add(1);
    }
    panic!("failed to find routable user_sk seed");
}

pub(crate) fn user_sk_matching_domain_hi(
    seed_base: [u8; 32],
    want_hi: u8,
) -> (SigningKey, u32, pwm_core::AccountId) {
    for n in 0u32..200_000 {
        let mut seed = seed_base;
        let bump = n.to_le_bytes();
        seed[0] = seed[0].wrapping_add(bump[0]);
        seed[1] ^= bump[1];
        seed[2] = seed[2].wrapping_add(bump[2]);
        seed[3] ^= bump[3];
        let (sk, i, aid) = user_sk(&seed);
        let dom = domain_of_account_id(&aid);
        if dom.to_be_bytes()[0] == want_hi {
            if validate_recipient_address_policy(&aid).is_err() {
                continue;
            }
            let probe = SignedTx::sign_body(&sk, dom, i, 0, TxBody::Init { index: 0, flags: 0 });
            if validate_tx_shape(&probe).is_ok() {
                return (sk, i, aid);
            }
        }
    }
    panic!("failed to find user_sk seed for domain_hi=0x{want_hi:02X}");
}

pub(crate) fn routable_user_sk_for_app(
    seed_base: [u8; 32],
    app: &App,
) -> (SigningKey, u32, pwm_core::AccountId) {
    user_sk_matching_domain_hi(seed_base, app.identity.cluster_domain_hi)
}

pub(crate) fn router_dev(app: App) -> Router {
    router(app, CorsLayer::permissive())
}

pub(crate) fn mk_app_explicit_shard(shard: DevLane) -> App {
    let (cfg, sks) = dev_net();
    let identity = RuntimeIdentity {
        network_id: "devnet".to_string(),
        cluster_domain_hi: match shard {
            DevLane::Lane0 => 0x10,
            DevLane::Lane1 => 0x20,
        },
        cluster_id: "explicit-cluster".to_string(),
        node_id: "explicit-node".to_string(),
        mode: RuntimeIdentityMode::Explicit,
    };
    crate::bootstrap::app_from_chain_boot(cfg, sks, None, shard, Some(identity))
}

pub(crate) fn app_with_identity(
    shard: DevLane,
    network_id: &str,
    domain_hi: u8,
    cluster_id: &str,
    node_id: &str,
) -> App {
    let (cfg, sks) = dev_net();
    let identity = RuntimeIdentity {
        network_id: network_id.to_string(),
        cluster_domain_hi: domain_hi,
        cluster_id: cluster_id.to_string(),
        node_id: node_id.to_string(),
        mode: RuntimeIdentityMode::Explicit,
    };
    crate::bootstrap::app_from_chain_boot(cfg, sks, None, shard, Some(identity))
}

pub(crate) fn app_for_domain(shard: DevLane, domain_hi: u8) -> App {
    app_with_identity(shard, "devnet", domain_hi, "test-cluster", "test-node")
}

pub(crate) fn app_for_sender(aid: &pwm_core::AccountId) -> App {
    let shard = shard_for_phase1_account(aid).expect("sender shard");
    let hi = domain_of_account_id(aid).to_be_bytes()[0];
    app_for_domain(shard, hi)
}

pub(crate) fn app_for_devnet_sender(shard: DevLane) -> App {
    let (cfg, _) = dev_net();
    let hi = domain_of_account_id(&cfg.accounts[0].acct).to_be_bytes()[0];
    app_for_domain(shard, hi)
}

pub(crate) fn temp_path(name: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("pwmd_{name}_{ts}.json"))
}

pub(crate) fn fake_account_id_with_domain(domain: u16) -> pwm_core::AccountId {
    let mut id = [0u8; 32];
    let [hi, lo] = domain.to_be_bytes();
    id[0] = hi;
    id[1] = lo;
    id[31] = 1;
    id
}

pub(crate) fn valid_cross_domain_recipient(sender_hi: u8) -> pwm_core::AccountId {
    (0u16..4096)
        .find_map(|n| {
            let seed = n.to_le_bytes();
            let mut s = [0u8; 32];
            s[..2].copy_from_slice(&seed);
            let (_, _, candidate) = user_sk(&s);
            if validate_recipient_address_policy(&candidate).is_err() {
                return None;
            }
            let target = domain_of_account_id(&candidate);
            if target.to_be_bytes()[0] == sender_hi {
                return None;
            }
            Some(candidate)
        })
        .expect("must find valid recipient in different domain")
}

pub(crate) fn sample_hello(local: &App, node_id: &str, domain_hi: u8, nonce: Vec<u8>) -> NodeHello {
    let sk = SigningKey::from_bytes(&[9u8; 32]);
    let (network_id, genesis_hash) = {
        let hs = local.handshake.try_read().expect("handshake read");
        (
            hs.validation_ctx.expected_network_id.clone(),
            hs.validation_ctx.expected_genesis_hash.clone(),
        )
    };
    let bridge_commitment = {
        let g = local.inner.try_read().expect("inner read");
        crate::bridge_trust::bridge_commitment_hex(&g.chain.st)
    };
    let mut hello = NodeHello {
        network_id,
        genesis_hash,
        cluster: NodeHelloCluster {
            domain_hi,
            cluster_id: local.identity.cluster_id.clone(),
        },
        node: NodeHelloNode {
            node_id: node_id.to_string(),
            pubkey: sk.verifying_key().to_bytes(),
        },
        capabilities: NodeHelloCapabilities {
            protocol_version: "0.1.0".to_string(),
            tx_features: vec!["local_transfer_v1".to_string()],
            services: vec!["mempool".to_string()],
            sync_profile: None,
            deployment_profile: crate::handshake::DeploymentProfile::SingleSealer,
            seal_role: crate::handshake::SealRole::Active,
            validator_identity_hash: Some("vh-helper".to_string()),
            node_instance_id: Some("inst-helper".to_string()),
            lease_owner_id: None,
            lease_term: None,
            lease_expires_at_ms: None,
            lease_last_tip: None,
            lease_fence: None,
            cluster_attest_enabled: false,
            cluster_role: crate::handshake::ClusterRole::None,
            cluster_members: Vec::new(),
            cluster_quorum_k: None,
            cluster_quorum_n: None,
        },
        nonce,
        timestamp_ms: current_time_ms().expect("clock"),
        signature: Vec::new(),
        chain_tip_height: None,
        federation_shard_id: None,
        bridge_commitment: Some(bridge_commitment),
    };
    hello.sign(&sk).expect("sign");
    hello
}

pub(crate) async fn trust_source_peer(target: &App, source: &App) {
    trust_peer_for_test(target, source).await;
}

pub(crate) async fn assert_recipient_prefilter_rejects(
    recipient: pwm_core::AccountId,
    expected_substrings: &[&str],
) {
    let (cfg, sks) = dev_net();
    let sk = &sks[0];
    let sender_dom = domain_of_account_id(&cfg.accounts[0].acct);
    let tx = SignedTx::sign_body(
        sk,
        sender_dom,
        0,
        0,
        TxBody::Transfer {
            to: recipient,
            amount: 1,
            fee: 0,
        },
    );
    let svc = router_dev(app_from_devnet(DevLane::Lane0)).into_service();
    let res = svc
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&tx).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    for s in expected_substrings {
        assert!(
            text.contains(s),
            "expected `{s}` in prefilter error body, got: {text}"
        );
    }
}

pub(crate) async fn assert_rcv_px_expl(
    recipient: pwm_core::AccountId,
    expected_substrings: &[&str],
) {
    let (cfg, sks) = dev_net();
    let sk = &sks[0];
    let sender_dom = domain_of_account_id(&cfg.accounts[0].acct);
    let tx = SignedTx::sign_body(
        sk,
        sender_dom,
        0,
        0,
        TxBody::Transfer {
            to: recipient,
            amount: 1,
            fee: 0,
        },
    );
    let svc = router_dev(mk_app_explicit_shard(DevLane::Lane0)).into_service();
    let res = svc
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&tx).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    for s in expected_substrings {
        assert!(
            text.contains(s),
            "expected `{s}` in explicit prefilter error body, got: {text}"
        );
    }
}

pub(crate) async fn assert_export_prefilter_rejects(
    recipient: pwm_core::AccountId,
    expected_substrings: &[&str],
) {
    let (cfg, sks) = dev_net();
    let sk = &sks[0];
    let sender_dom = domain_of_account_id(&cfg.accounts[0].acct);
    let sender_hi = sender_dom.to_be_bytes()[0];
    let target_hi = sender_hi.wrapping_add(1);
    let target_domain = ((target_hi as u16) << 8) | 0x01;
    let tx = SignedTx::sign_body(
        sk,
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
    let svc = router_dev(app_from_devnet(DevLane::Lane0)).into_service();
    let res = svc
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&tx).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    for s in expected_substrings {
        assert!(
            text.contains(s),
            "expected `{s}` in export prefilter error body, got: {text}"
        );
    }
}

pub(crate) fn readiness_reject_fields(body: &[u8]) -> (String, String, String) {
    let v: serde_json::Value = serde_json::from_slice(body).expect("readiness reject json");
    let code = v["code"].as_str().expect("code string").to_string();
    let hint = v["hint"].as_str().expect("hint string").to_string();
    let message = v["message"].as_str().expect("message string").to_string();
    (code, hint, message)
}

pub(crate) fn sample_genesis_v4_bundle(passphrase: &str) -> serde_json::Value {
    let validator_seed = [99u8; 32];
    let sk_bytes = derive_ed25519_private_key(&validator_seed, &[1_000_000, 1]);
    let sk = SigningKey::from_bytes(&sk_bytes);
    let pubkey = sk.verifying_key().to_bytes();
    let acct = account_id_from_parts(&pubkey, 1);
    let sealed = pwm_core::seal_wallet_secret_plaintext(&validator_seed, passphrase).expect("seal");
    serde_json::json!({
        "schema_version": 4,
        "gen_cfg": {
            "funding": { "accounts": [{
                    "acct_hex": hex::encode(acct),
                    "pubkey_hex": hex::encode(pubkey),
                    "der_idx": 1,
                    "bal": "777"
            }]},
            "validators": { "set": [{
                    "acct_hex": hex::encode(acct),
                    "pubkey_hex": hex::encode(pubkey),
                    "der_idx": 1
            }]},
            "reward_policy": { "mode": "to_producer_account" },
            "block_reward": "9",
            "marks_coeff": "10"
        },
        "validator_keys": [{
            "derivation_path": "m/1000000'/1'",
            "enc_seed": {
                "kdf": {
                    "name": sealed.kdf,
                    "iters": sealed.kdf_iters,
                    "salt_b64": sealed.kdf_salt_b64
                },
                "aead": {
                    "name": "chacha20poly1305",
                    "nonce_b64": sealed.aead_nonce_b64,
                    "ciphertext_b64": sealed.encrypted_payload_b64
                }
            }
        }]
    })
}

pub(crate) async fn seed_handoff_provenance_for_import(
    target_app: &App,
    amount: u128,
) -> (
    ed25519_dalek::SigningKey,
    u32,
    pwm_core::AccountId,
    [u8; 32],
    pwm_core::state::ExportProvenance,
) {
    let source_app = app_from_devnet(DevLane::Lane1);
    let (import_sk, import_i, import_aid) = routable_user_sk_for_app([0x39; 32], target_app);
    let import_dom = domain_of_account_id(&import_aid);
    let (source_sk, source_i, source_aid) = routable_user_sk_for_app([0x4A; 32], &source_app);
    let source_dom = domain_of_account_id(&source_aid);
    let init = SignedTx::sign_body(
        &import_sk,
        import_dom,
        import_i,
        0,
        TxBody::Init { index: 1, flags: 0 },
    );
    let export_nonce = {
        let mut g = source_app.inner.write().await;
        if !g.chain.st.accounts.contains_key(&source_aid) {
            let init_source = SignedTx::sign_body(
                &source_sk,
                source_dom,
                source_i,
                0,
                TxBody::Init {
                    index: source_i,
                    flags: 0,
                },
            );
            g.chain
                .st
                .apply_tx(&init_source)
                .expect("init source sender");
        }
        let src = g
            .chain
            .st
            .accounts
            .get_mut(&source_aid)
            .expect("source sender");
        src.balance_pwm = src.balance_pwm.saturating_add(amount.saturating_add(10));
        src.nonce
    };
    let export = SignedTx::sign_body(
        &source_sk,
        source_dom,
        source_i,
        export_nonce,
        TxBody::Export {
            to: import_aid,
            target_domain: import_dom,
            amount,
            fee: 0,
        },
    );
    let export_id = export.export_id().expect("export id");
    {
        let mut g = target_app.inner.write().await;
        g.chain.st.apply_tx(&init).expect("init importer");
        credit_min_import_fee_tests(&mut g.chain.st, &import_aid);
    }
    let source_svc = router_dev(source_app.clone()).into_service();
    let ready_rs = source_svc
        .clone()
        .oneshot(
            Request::post("/v1/export-readiness")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({ "tx": export, "ttl_sec": 30 }))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ready_rs.status(), StatusCode::OK);
    let create_body = serde_json::json!({
        "tx": export,
        "ttl_blocks": 3
    });
    let create = source_svc
        .clone()
        .oneshot(
            Request::post("/v1/roaming-intents")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&create_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let create_status = create.status();
    let create_body = to_bytes(create.into_body(), 64 * 1024).await.unwrap();
    let create_text = String::from_utf8_lossy(&create_body);
    assert_eq!(create_status, StatusCode::OK, "body={create_text}");
    let create_json: serde_json::Value = serde_json::from_slice(&create_body).unwrap();
    let intent_id = create_json["intent_id"].as_str().expect("intent_id");
    let finalize = source_svc
        .oneshot(
            Request::post(format!("/v1/roaming-intents/{intent_id}/finalize"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(finalize.status(), StatusCode::OK);
    let finalize_body = to_bytes(finalize.into_body(), 64 * 1024).await.unwrap();
    let finalize_json: serde_json::Value = serde_json::from_slice(&finalize_body).unwrap();
    assert_eq!(finalize_json["status"], "exported");
    let handoff = finalize_json["handoff"].clone();
    assert_eq!(handoff["export_id"], hex::encode(export_id));
    trust_source_peer(target_app, &source_app).await;
    let target_svc = router_dev(target_app.clone()).into_service();
    let register = target_svc
        .oneshot(
            Request::post("/v1/export-provenance")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&handoff).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(register.status(), StatusCode::OK);
    (
        import_sk,
        import_i,
        import_aid,
        export_id,
        pwm_core::state::ExportProvenance {
            to: import_aid,
            target_domain: import_dom,
            amount,
        },
    )
}

pub(crate) async fn source_handoff(
    source_app: &App,
    amount: u128,
) -> (serde_json::Value, [u8; 32]) {
    let (cfg, _) = dev_net();
    let source = cfg.accounts[0].acct;
    let source_hi = domain_of_account_id(&source).to_be_bytes()[0];
    source_handoff_for_hi(source_app, amount, source_hi.wrapping_add(1)).await
}

pub(crate) async fn source_handoff_for_hi(
    source_app: &App,
    amount: u128,
    target_hi: u8,
) -> (serde_json::Value, [u8; 32]) {
    let (cfg, sks) = dev_net();
    let sk = &sks[0];
    let source = cfg.accounts[0].acct;
    let source_dom = domain_of_account_id(&source);
    let recipient = if target_hi == source_dom.to_be_bytes()[0] {
        valid_cross_domain_recipient(source_dom.to_be_bytes()[0])
    } else {
        let (_, _, aid) = user_sk_matching_domain_hi([0x5B; 32], target_hi);
        aid
    };
    let target_domain = domain_of_account_id(&recipient);
    let tx = SignedTx::sign_body(
        sk,
        source_dom,
        0,
        0,
        TxBody::Export {
            to: recipient,
            target_domain,
            amount,
            fee: 1,
        },
    );
    let export_id = tx.export_id().expect("export id");
    let svc = router_dev(source_app.clone()).into_service();
    let ready = serde_json::json!({ "tx": tx, "ttl_sec": 30 });
    let ready_res = svc
        .clone()
        .oneshot(
            Request::post("/v1/export-readiness")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&ready).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ready_res.status(), StatusCode::OK);
    let create = serde_json::json!({ "tx": tx, "ttl_blocks": 3 });
    let create_res = svc
        .clone()
        .oneshot(
            Request::post("/v1/roaming-intents")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&create).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_res.status(), StatusCode::OK);
    let create_body = to_bytes(create_res.into_body(), 64 * 1024).await.unwrap();
    let create_json: serde_json::Value = serde_json::from_slice(&create_body).unwrap();
    let intent_id = create_json["intent_id"].as_str().expect("intent id");
    let finalize = svc
        .oneshot(
            Request::post(format!("/v1/roaming-intents/{intent_id}/finalize"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(finalize.status(), StatusCode::OK);
    let finalize_body = to_bytes(finalize.into_body(), 64 * 1024).await.unwrap();
    let finalize_json: serde_json::Value = serde_json::from_slice(&finalize_body).unwrap();
    (finalize_json["handoff"].clone(), export_id)
}
