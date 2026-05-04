//! Small HTTP helpers tests: CORS policy and status shard parsing.

use super::helpers::*;
use super::prelude::*;

#[test]
fn cors_loopback_is_permissive() {
    let a = SocketAddr::from(([127, 0, 0, 1], 3030));
    assert!(cors_for_listen(a).is_ok());
}

/// PWM_CORS_ORIGINS must be set when binding to wildcard listeners (non-loopback).
#[test]
fn cors_wild_listen_needs_origins() {
    let key = "PWM_CORS_ORIGINS";
    let old = std::env::var(key).ok();
    std::env::remove_var(key);
    let a = SocketAddr::from(([0, 0, 0, 0], 3030));
    let r = cors_for_listen(a);
    assert!(r.is_err());
    if let Some(v) = old {
        std::env::set_var(key, v);
    }
}

/// parse_cluster_domain_hi accepts decimal and hex (0x…) cluster domain_hi inputs.
#[test]
fn parse_cls_dom_hi_ok() {
    assert_eq!(parse_cluster_domain_hi("16").unwrap(), 16);
    assert_eq!(parse_cluster_domain_hi("0x10").unwrap(), 16);
    assert_eq!(parse_cluster_domain_hi("0XFF").unwrap(), 255);
}

/// parse_cluster_domain_hi rejects out-of-domain values with the CLI flag diagnostics string.
#[test]
fn parse_cls_dom_hi_rejects() {
    let err = parse_cluster_domain_hi("0x100").unwrap_err();
    assert!(err.contains("invalid --cluster-domain-hi value"));
}

/// Explicit RuntimeIdentity parsing rejects partially filled flags (forces all-or-none fields).
#[test]
fn resolve_id_partial_explicit() {
    let input = RuntimeIdentityInput {
        network_id: Some("devnet".to_string()),
        cluster_domain_hi: None,
        cluster_id: Some("cluster-a".to_string()),
        node_id: Some("node-a".to_string()),
    };
    let err = resolve_runtime_identity(ShardId::A, input).unwrap_err();
    assert!(err.contains("partial identity configuration is not allowed"));
    assert!(err.contains("cluster_domain_hi"));
}

/// Default RuntimeIdentity resolves compat shard aliases plus Alias { shard } mode tagging.
#[test]
fn resolve_id_shard_alias_ok() {
    let a = resolve_runtime_identity(ShardId::A, RuntimeIdentityInput::default()).unwrap();
    assert_eq!(a.network_id, "devnet");
    assert_eq!(a.cluster_domain_hi, 0x10);
    assert_eq!(a.cluster_id, "compat-shard-a");
    assert_eq!(a.node_id, "compat-node-a");
    assert_eq!(a.mode, RuntimeIdentityMode::Alias { shard: ShardId::A });
    assert!(!a.mode.is_shard_enforced());

    let b = resolve_runtime_identity(ShardId::B, RuntimeIdentityInput::default()).unwrap();
    assert_eq!(b.cluster_domain_hi, 0x20);
    assert_eq!(b.cluster_id, "compat-shard-b");
    assert_eq!(b.node_id, "compat-node-b");
    assert_eq!(b.mode, RuntimeIdentityMode::Alias { shard: ShardId::B });
    assert!(!b.mode.is_shard_enforced());
}

/// Neutral runtime identities map to relay-neutral node metadata and neutral storage namespaces.
#[test]
fn def_rid_neutral_ok() {
    let identity = default_runtime_identity_neutral();
    assert_eq!(identity.network_id, "devnet");
    assert_eq!(identity.cluster_domain_hi, 0x00);
    assert_eq!(identity.cluster_id, "relay-neutral");
    assert_eq!(identity.node_id, "relay-neutral");
    assert_eq!(identity.mode, RuntimeIdentityMode::Neutral);
    assert!(!identity.mode.is_shard_enforced());
    assert_eq!(storage_namespace(&identity), "neutral");
}

/// Neutral default snapshot paths isolate on RPC listen (colon unsafe in file paths).
#[test]
fn neutral_listen_tag_ok() {
    let a = SocketAddr::from(([127, 0, 0, 1], 3030));
    assert_eq!(neutral_listen_dir_tag(a), "127.0.0.1+3030");
}

/// Fully specified explicit inputs pin RuntimeIdentityMode::Explicit with shard-enforced semantics.
#[test]
fn resolve_id_explicit_enforced() {
    let identity = resolve_runtime_identity(
        ShardId::A,
        RuntimeIdentityInput {
            network_id: Some("devnet".to_string()),
            cluster_domain_hi: Some(0x10),
            cluster_id: Some("cluster-a".to_string()),
            node_id: Some("node-a".to_string()),
        },
    )
    .unwrap();
    assert_eq!(identity.mode, RuntimeIdentityMode::Explicit);
    assert!(identity.mode.is_shard_enforced());
}

/// storage_namespace maps alias shards to shard-* namespaces and explicit domains to domain-hi-* keys.
#[test]
fn storage_ns_shard_domain_ok() {
    let alias = resolve_runtime_identity(ShardId::A, RuntimeIdentityInput::default()).unwrap();
    assert_eq!(storage_namespace(&alias), "shard-a");

    let explicit = resolve_runtime_identity(
        ShardId::B,
        RuntimeIdentityInput {
            network_id: Some("devnet".to_string()),
            cluster_domain_hi: Some(0x20),
            cluster_id: Some("cluster-b".to_string()),
            node_id: Some("node-b".to_string()),
        },
    )
    .unwrap();
    assert_eq!(storage_namespace(&explicit), "domain-hi-0x20");
}

/// /v1/status reports compat shard-a namespace identifiers for alias-mode bootstraps.
#[tokio::test]
async fn v1_stat_alias_shard_ns() {
    let svc = router_dev(app_from_dev_net_shard(ShardId::A)).into_service();
    let res = svc
        .oneshot(Request::get("/v1/status").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["state_namespace"], "shard-a");
    assert_eq!(v["shard"], "A");
    assert_eq!(v["cluster_domain_hi"], 0x10);
    assert_eq!(v["bridge_exported_registry_size"], 0);
    assert_eq!(v["bridge_imported_set_size"], 0);
    assert_eq!(v["roaming_relay_mode"], "peer_relay_one_window");
    assert_eq!(v["peer_relay_health"], "not_configured");
    assert_eq!(
        v["genesis_fetch_status"],
        "stub_parent_peer_fetch_not_enabled"
    );
}

/// /v1/status cross_shard_summary reflects CrossShardLedger handoff bookkeeping fields.
#[tokio::test]
async fn v1_stat_cross_shard_sum() {
    let app = app_from_dev_net_shard(ShardId::A);
    {
        let mut g = app.inner.write().await;
        let source = fake_account_id_with_domain(0x1001);
        let to = fake_account_id_with_domain(0x2001);
        g.cross_shard
            .record_handoff([0xAB; 32], 0x10, source, to, 0x2001, 55, 3, None);
    }
    let svc = router_dev(app).into_service();
    let res = svc
        .oneshot(Request::get("/v1/status").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        v["cross_shard_summary"]["scope"],
        "local_plus_trusted_peer_observations"
    );
    assert_eq!(v["cross_shard_summary"]["total_exported_count"], 1);
    assert_eq!(v["cross_shard_summary"]["total_exported_amount"], "55");
    assert_eq!(v["cross_shard_summary"]["pending_count"], 1);
}

/// Neutral-runtime apps publish neutral shards/namespaces even when ShardId defaults to dev A.
#[tokio::test]
async fn v1_stat_neutral_relay() {
    let (cfg, sks) = dev_net();
    let identity = default_runtime_identity_neutral();
    let app = crate::bootstrap::app_from_chain_boot(cfg, sks, None, ShardId::A, Some(identity));
    let svc = router_dev(app.clone()).into_service();
    let res = svc
        .oneshot(Request::get("/v1/status").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["state_namespace"], "neutral");
    assert_eq!(v["shard"], "neutral");
}

/// Explicit cluster_domain_hi shards surface domain-hi storage namespaces alongside /v1/head.
#[tokio::test]
async fn v1_stat_expl_domain_ns() {
    let svc = router_dev(mk_app_explicit_shard(ShardId::B)).into_service();
    let status = svc
        .clone()
        .oneshot(Request::get("/v1/status").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(status.status(), StatusCode::OK);
    let bytes = to_bytes(status.into_body(), 64 * 1024).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["state_namespace"], "domain-hi-0x20");
    let expected = pwm_core::domain_index::lookup_regulatory_by_hi(0x20)
        .map(|entry| entry.label)
        .unwrap_or("0x20");
    assert_eq!(v["shard"], expected);

    let head = svc
        .oneshot(Request::get("/v1/head").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(head.status(), StatusCode::OK);
}

/// /v1/status advertises split balance semantics for local vs authoritative home balances.
#[tokio::test]
async fn v1_stat_bal_sem_contract() {
    let svc = router_dev(app_from_dev_net_shard(ShardId::A)).into_service();
    let res = svc
        .oneshot(Request::get("/v1/status").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        v["balance_semantics"],
        "split:v1(local_state_balance,authoritative_home_balance,spendable_on_this_shard)"
    );
}

/// /v1/account labels foreign-domain accounts as local_view_only with zero spendable_pwm.
#[tokio::test]
async fn v1_acct_foreign_view_only() {
    let app = app_from_dev_net_shard(ShardId::A);
    let foreign_id = fake_account_id_with_domain(0x2001);
    let local_id = fake_account_id_with_domain(0x1001);
    {
        let mut g = app.inner.write().await;
        g.chain.st.accounts.insert(
            local_id,
            Account {
                balance_pwm: 88,
                initialized: true,
                ..Account::default()
            },
        );
        g.chain.st.accounts.insert(
            foreign_id,
            Account {
                balance_pwm: 77,
                initialized: true,
                ..Account::default()
            },
        );
    }
    let svc = router_dev(app).into_service();

    let local_res = svc
        .clone()
        .oneshot(
            Request::get(format!("/v1/account/{}", hex::encode(local_id)))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(local_res.status(), StatusCode::OK);
    let local_bytes = to_bytes(local_res.into_body(), 64 * 1024).await.unwrap();
    let local: serde_json::Value = serde_json::from_slice(&local_bytes).unwrap();
    assert_eq!(local["local_view_only"], false);
    assert_eq!(local["local_state_balance"], "88");
    assert_eq!(local["local_state_balance"], local["balance_pwm"]);
    assert_eq!(
        local["spendable_on_this_shard"],
        local["local_state_balance"]
    );
    assert!(local["authoritative_home_balance"].is_null());

    let foreign_res = svc
        .oneshot(
            Request::get(format!("/v1/account/{}", hex::encode(foreign_id)))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(foreign_res.status(), StatusCode::OK);
    let foreign_bytes = to_bytes(foreign_res.into_body(), 64 * 1024).await.unwrap();
    let foreign: serde_json::Value = serde_json::from_slice(&foreign_bytes).unwrap();
    assert_eq!(foreign["local_view_only"], true);
    assert_eq!(foreign["local_state_balance"], "77");
    assert_eq!(foreign["balance_pwm"], "0");
    assert!(foreign["authoritative_home_balance"].is_null());
    assert!(foreign["spendable_on_this_shard"].is_null());
}

/// /v1/accounts preserves the same split semantics for inserted local vs foreign-domain accounts.
#[tokio::test]
async fn v1_accts_foreign_split_ls() {
    let app = app_from_dev_net_shard(ShardId::A);
    let foreign_id = fake_account_id_with_domain(0x2001);
    let local_id = fake_account_id_with_domain(0x1001);
    {
        let mut g = app.inner.write().await;
        g.chain.st.accounts.insert(
            local_id,
            Account {
                balance_pwm: 88,
                initialized: true,
                ..Account::default()
            },
        );
        g.chain.st.accounts.insert(
            foreign_id,
            Account {
                balance_pwm: 77,
                initialized: true,
                ..Account::default()
            },
        );
    }
    let svc = router_dev(app).into_service();
    let res = svc
        .oneshot(Request::get("/v1/accounts").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let accounts = v["accounts"].as_array().expect("accounts array");
    let local = accounts
        .iter()
        .find(|row| row["id"] == hex::encode(local_id))
        .expect("local account in list");
    let foreign = accounts
        .iter()
        .find(|row| row["id"] == hex::encode(foreign_id))
        .expect("foreign account in list");

    assert_eq!(local["local_view_only"], false);
    assert_eq!(local["local_state_balance"], "88");
    assert_eq!(local["balance_pwm"], "88");
    assert_eq!(local["spendable_on_this_shard"], "88");

    assert_eq!(foreign["local_view_only"], true);
    assert_eq!(foreign["local_state_balance"], "77");
    assert_eq!(foreign["balance_pwm"], "0");
    assert!(foreign["spendable_on_this_shard"].is_null());
}

/// Genesis JSON v4 roundtrips decrypt validator enc_seed blobs into loaded signing keys.
#[test]
fn genes_v4_enc_key_rt() {
    let passphrase = "slice10-pass";
    let bundle = sample_genesis_v4_bundle(passphrase);
    let p = std::env::temp_dir().join("pwmd_genesis_test_v3_roundtrip.json");
    std::fs::write(&p, serde_json::to_string_pretty(&bundle).unwrap()).unwrap();
    let (cfg, sks) = load_genesis_bundle(&p, Some(passphrase)).expect("load v4");
    assert_eq!(cfg.funding.accounts.len(), 1);
    assert_eq!(cfg.vals.set[0].der_idx, 1);
    assert_eq!(sks.len(), 1);
    let _ = std::fs::remove_file(&p);
}

/// Genesis v4 decryption fails closed when the passphrase does not decrypt enc_seed payloads.
#[test]
fn genes_v4_bad_pass_fail() {
    let bundle = sample_genesis_v4_bundle("good-pass");
    let p = std::env::temp_dir().join("pwmd_genesis_test_v3_wrong_pass.json");
    std::fs::write(&p, serde_json::to_string_pretty(&bundle).unwrap()).unwrap();
    let err = load_genesis_bundle(&p, Some("bad-pass")).expect_err("must fail");
    assert!(
        err.contains("failed to decrypt wallet payload"),
        "unexpected error: {err}"
    );
    let _ = std::fs::remove_file(&p);
}

/// Malformed validator enc_seed payloads fail genesis load before touching chain boot state.
#[test]
fn genes_v4_bad_enc_fail() {
    let mut bundle = sample_genesis_v4_bundle("good-pass");
    bundle["validator_keys"][0]["enc_seed"]["aead"]["ciphertext_b64"] =
        serde_json::Value::String("%%%not-base64%%%".to_string());
    let p = std::env::temp_dir().join("pwmd_genesis_test_v3_bad_payload.json");
    std::fs::write(&p, serde_json::to_string_pretty(&bundle).unwrap()).unwrap();
    let err = load_genesis_bundle(&p, Some("good-pass")).expect_err("must fail");
    assert!(
        err.contains("encrypted_payload_b64"),
        "unexpected error: {err}"
    );
    let _ = std::fs::remove_file(&p);
}

/// Genesis rejects unsafe kdf.iter magnitudes exceeding the hardened safety caps.
#[test]
fn genes_v4_kdf_hi_fail() {
    let mut bundle = sample_genesis_v4_bundle("good-pass");
    bundle["validator_keys"][0]["enc_seed"]["kdf"]["iters"] =
        serde_json::Value::Number(serde_json::Number::from(50_000_000_u64));
    let p = std::env::temp_dir().join("pwmd_genesis_test_v3_extreme_kdf_iters.json");
    std::fs::write(&p, serde_json::to_string_pretty(&bundle).unwrap()).unwrap();
    let err = load_genesis_bundle(&p, Some("good-pass")).expect_err("must fail");
    assert!(
        err.contains("kdf.iters exceeds safety cap"),
        "unexpected error: {err}"
    );
    let _ = std::fs::remove_file(&p);
}

/// Unsupported schema_version shards error before attempting validator key ingestion.
#[test]
fn genes_schema_unsup_fail() {
    let bundle = serde_json::json!({
        "schema_version": 2,
        "gen_cfg": { "rows": [], "block_reward": "9", "marks_coeff": "10" },
        "validator_keys": []
    });
    let p = std::env::temp_dir().join("pwmd_genesis_test_schema_unsupported.json");
    std::fs::write(&p, serde_json::to_string_pretty(&bundle).unwrap()).unwrap();
    let err = load_genesis_bundle(&p, Some("x")).expect_err("unsupported schema must fail");
    assert!(
        err.contains("unsupported schema_version 2"),
        "unexpected error: {err}"
    );
    let _ = std::fs::remove_file(&p);
}

/// Validator derivation_path mismatches versus enc_seed expectations fail deterministic validation.
#[test]
fn genes_v4_path_bad_fail() {
    let mut bundle = sample_genesis_v4_bundle("good-pass");
    bundle["validator_keys"][0]["derivation_path"] =
        serde_json::Value::String("m/0'/0'".to_string());
    let p = std::env::temp_dir().join("pwmd_genesis_test_v3_path_mismatch.json");
    std::fs::write(&p, serde_json::to_string_pretty(&bundle).unwrap()).unwrap();
    let err = load_genesis_bundle(&p, Some("good-pass")).expect_err("must fail");
    assert!(err.contains("derivation_path"), "unexpected error: {err}");
    let _ = std::fs::remove_file(&p);
}

#[tokio::test]
async fn v1_head_returns_tip_json() {
    let svc = router_dev(app_from_dev_net()).into_service();
    let res = svc
        .oneshot(Request::get("/v1/head").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["height"].as_u64(), Some(0));
    assert!(v.get("tip").and_then(|t| t.as_str()).map(|s| !s.is_empty()) == Some(true));
}

/// /v1/status reports loading snapshots while concurrent /v1/head rejects with SERVICE_UNAVAILABLE.
#[tokio::test]
async fn v1_stat_load_head_503() {
    let app = app_from_dev_net();
    {
        let mut st = app.init.write().await;
        *st = InitState::loading(Some(PathBuf::from("pwm-data.json")));
    }
    let svc = router_dev(app.clone()).into_service();
    let status = svc
        .clone()
        .oneshot(Request::get("/v1/status").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(status.status(), StatusCode::OK);
    let bytes = to_bytes(status.into_body(), 64 * 1024).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["phase"], "loading_snapshot");
    assert_eq!(v["ready"], false);

    let head = svc
        .oneshot(Request::get("/v1/head").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(head.status(), StatusCode::SERVICE_UNAVAILABLE);
}

/// ready_degraded init states expose snapshot_error text yet still satisfy /v1/head callers.
#[tokio::test]
async fn v1_stat_deg_snap_err() {
    let app = app_from_dev_net();
    {
        let mut st = app.init.write().await;
        *st = InitState::ready_degraded(
            Some(PathBuf::from("pwm-data.json")),
            "snapshot parse failed".to_string(),
        );
    }
    let svc = router_dev(app.clone()).into_service();
    let status = svc
        .clone()
        .oneshot(Request::get("/v1/status").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(status.status(), StatusCode::OK);
    let bytes = to_bytes(status.into_body(), 64 * 1024).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["phase"], "ready_degraded");
    assert_eq!(v["ready"], true);
    assert_eq!(v["snapshot_error"], "snapshot parse failed");

    let head = svc
        .oneshot(Request::get("/v1/head").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(head.status(), StatusCode::OK);
}

/// Concurrent snapshot persistence alongside /v1/status and signed /v1/tx callers must not deadlock.
#[tokio::test]
async fn v1_stat_snap_tx_nl() {
    let seed = [99u8; 32];
    let sk0_bytes = derive_ed25519_private_key(&seed, &[0, 0]);
    let sk1_bytes = derive_ed25519_private_key(&seed, &[0, 1]);
    let sk0 = SigningKey::from_bytes(&sk0_bytes);
    let sk1 = SigningKey::from_bytes(&sk1_bytes);
    let aid0 = account_id_from_parts(&sk0.verifying_key().to_bytes(), 0);
    let aid1 = account_id_from_parts(&sk1.verifying_key().to_bytes(), 1);
    let dom0 = domain_of_account_id(&aid0);
    let dom1 = domain_of_account_id(&aid1);

    let mut app = app_from_dev_net();
    {
        let mut g = app.inner.write().await;
        let init_peer = SignedTx::sign_body(&sk1, dom1, 1, 0, TxBody::Init { index: 1, flags: 0 });
        g.chain.st.apply_tx(&init_peer).expect("init peer account");
    }
    let tx = SignedTx::sign_body(
        &sk0,
        dom0,
        0,
        0,
        TxBody::Transfer {
            to: aid1,
            amount: 1,
            fee: 0,
        },
    );
    let tx_body = serde_json::to_vec(&tx).unwrap();

    let snapshot_path = temp_path("lock_order_smoke");
    app.data_file = Some(snapshot_path.clone());
    let svc = router_dev(app.clone()).into_service();

    for _ in 0..16 {
        let status_fut = svc
            .clone()
            .oneshot(Request::get("/v1/status").body(Body::empty()).unwrap());
        let tx_fut = svc.clone().oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(tx_body.clone()))
                .unwrap(),
        );
        let (status, tx_res) = tokio::time::timeout(Duration::from_secs(2), async move {
            tokio::join!(status_fut, tx_fut)
        })
        .await
        .expect("concurrent status+tx call timed out");
        let status = status.unwrap();
        let tx_res = tx_res.unwrap();
        assert_eq!(status.status(), StatusCode::OK);
        assert_eq!(tx_res.status(), StatusCode::NO_CONTENT);
    }

    let _ = std::fs::remove_file(snapshot_path);
}

#[tokio::test]
async fn v1_tx_rejects_domain_mismatch() {
    let (sk, i, aid) = routable_user_sk([17u8; 32]);
    let d_ok = domain_of_account_id(&aid);
    let d_bad = if d_ok == u16::MAX { 0 } else { d_ok + 1 };
    let tx = SignedTx::sign_body(&sk, d_bad, i, 0, TxBody::Init { index: 0, flags: 0 });
    let svc = router_dev(app_from_dev_net_shard(
        shard_for_phase1_account(&aid).expect("routable"),
    ))
    .into_service();
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
}

/// Underfunded transfer must not enter the mempool (conflict), avoiding seal-loop spam on the node.
#[tokio::test]
async fn v1_tx_rejects_underfunded_transfer_mempool() {
    let seed = [99u8; 32];
    let sk0_bytes = derive_ed25519_private_key(&seed, &[0, 0]);
    let sk1_bytes = derive_ed25519_private_key(&seed, &[0, 1]);
    let sk0 = SigningKey::from_bytes(&sk0_bytes);
    let sk1 = SigningKey::from_bytes(&sk1_bytes);
    let aid0 = account_id_from_parts(&sk0.verifying_key().to_bytes(), 0);
    let aid1 = account_id_from_parts(&sk1.verifying_key().to_bytes(), 1);
    let dom0 = domain_of_account_id(&aid0);
    let dom1 = domain_of_account_id(&aid1);

    let app = app_from_dev_net();
    {
        let mut g = app.inner.write().await;
        let init_peer = SignedTx::sign_body(&sk1, dom1, 1, 0, TxBody::Init { index: 1, flags: 0 });
        g.chain.st.apply_tx(&init_peer).expect("init peer account");
        g.chain
            .st
            .accounts
            .get_mut(&aid0)
            .expect("sender")
            .balance_pwm = 5;
    }
    let tx = SignedTx::sign_body(
        &sk0,
        dom0,
        0,
        0,
        TxBody::Transfer {
            to: aid1,
            amount: 10,
            fee: 0,
        },
    );
    let svc = router_dev(app.clone()).into_service();
    let res = svc
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&tx).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    let body = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(
        text.to_ascii_lowercase().contains("insufficient")
            || text.contains("insufficient balance"),
        "expected insufficient-balance hint, got: {text}"
    );
    let g = app.inner.read().await;
    assert_eq!(g.pool.len(), 0, "mempool must stay empty");
}
