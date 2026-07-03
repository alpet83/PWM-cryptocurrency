//! Small HTTP helpers tests: CORS policy and status shard parsing.

use super::helpers::*;
use super::prelude::*;
use axum::extract::ConnectInfo;

fn req_conn(req: Request<Body>, addr: SocketAddr) -> Request<Body> {
    let (mut parts, body) = req.into_parts();
    parts.extensions.insert(ConnectInfo(addr));
    Request::from_parts(parts, body)
}

fn off_entry(nonce: u64) -> serde_json::Value {
    serde_json::json!({
        "account_id": hex::encode([7u8; 32]),
        "amount": "1",
        "nonce": nonce
    })
}

/// Static RPC allowlists reject non-matching source IPs.
#[tokio::test]
async fn rpc_ip_static_rejects() {
    let mut app = app_from_dev_net();
    app.rpc_allow =
        crate::rpc_allow::RpcAllowState::from_cfg(&["192.168.1.0/24".to_string()], 0).unwrap();
    let svc = router_dev(app).into_service();
    let req = Request::get("/v1/version").body(Body::empty()).unwrap();
    let res = svc
        .oneshot(req_conn(req, SocketAddr::from(([10, 0, 0, 9], 35000))))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

/// Static RPC allowlists accept matching CIDR source IPs.
#[tokio::test]
async fn rpc_ip_static_allows() {
    let mut app = app_from_dev_net();
    app.rpc_allow =
        crate::rpc_allow::RpcAllowState::from_cfg(&["192.168.1.0/24".to_string()], 0).unwrap();
    let svc = router_dev(app).into_service();
    let req = Request::get("/v1/version").body(Body::empty()).unwrap();
    let res = svc
        .oneshot(req_conn(req, SocketAddr::from(([192, 168, 1, 44], 35000))))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

/// Auto-enrolled RPC source IPs remain allowed after the enrollment window closes.
#[tokio::test]
async fn rpc_ip_auto_sticks() {
    let mut app = app_from_dev_net();
    app.rpc_allow = crate::rpc_allow::RpcAllowState::from_cfg(&[], 60).unwrap();
    let svc = router_dev(app.clone()).into_service();
    let req = Request::get("/v1/version").body(Body::empty()).unwrap();
    let enrolled = SocketAddr::from(([10, 0, 0, 9], 35000));
    let res = svc.oneshot(req_conn(req, enrolled)).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    app.rpc_allow = app.rpc_allow.with_closed_auto();
    let svc = router_dev(app).into_service();
    let req = Request::get("/v1/version").body(Body::empty()).unwrap();
    let res = svc.oneshot(req_conn(req, enrolled)).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let mut app = app_from_dev_net();
    app.rpc_allow = crate::rpc_allow::RpcAllowState::from_cfg(&[], 60)
        .unwrap()
        .with_closed_auto();
    let req = Request::get("/v1/version").body(Body::empty()).unwrap();
    let unknown = SocketAddr::from(([10, 0, 0, 10], 35000));
    let res = router_dev(app)
        .into_service()
        .oneshot(req_conn(req, unknown))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

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

/// Oversized offchain batches are rejected before parsing or storage.
#[tokio::test]
async fn v1_off_batch_413() {
    let app = app_from_dev_net();
    let body = (0..4097u64).map(off_entry).collect::<Vec<_>>();
    let svc = router_dev(app).into_service();
    let res = svc
        .oneshot(
            Request::post("/v1/offchain/batch")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

/// Offchain batch submission obeys the node readiness gate.
#[tokio::test]
async fn v1_off_batch_ready_gate() {
    let app = app_from_dev_net();
    {
        let mut st = app.init.write().await;
        *st = InitState::loading(Some(PathBuf::from("pwm-data.json")));
    }
    let svc = router_dev(app).into_service();
    let res = svc
        .oneshot(
            Request::post("/v1/offchain/batch")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&vec![off_entry(1)]).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
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
    let err = resolve_runtime_identity(DevLane::Lane0, input).unwrap_err();
    assert!(err.contains("partial identity configuration is not allowed"));
    assert!(err.contains("cluster_domain_hi"));
}

/// Default RuntimeIdentity resolves to explicit domain-first dev-lane presets.
#[test]
fn resolve_dev_lane_defaults_ok() {
    let a = resolve_runtime_identity(DevLane::Lane0, RuntimeIdentityInput::default()).unwrap();
    assert_eq!(a.network_id, "devnet");
    assert_eq!(a.cluster_domain_hi, 0x10);
    assert_eq!(a.cluster_id, "dev-cluster-0x10");
    assert_eq!(a.node_id, "dev-node-0x10");
    assert_eq!(a.mode, RuntimeIdentityMode::Explicit);
    assert!(a.mode.is_shard_enforced());

    let b = resolve_runtime_identity(DevLane::Lane1, RuntimeIdentityInput::default()).unwrap();
    assert_eq!(b.cluster_domain_hi, 0x20);
    assert_eq!(b.cluster_id, "dev-cluster-0x20");
    assert_eq!(b.node_id, "dev-node-0x20");
    assert_eq!(b.mode, RuntimeIdentityMode::Explicit);
    assert!(b.mode.is_shard_enforced());
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
        DevLane::Lane0,
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

/// storage_namespace maps both explicit presets and explicit inputs to domain-hi-* keys.
#[test]
fn storage_ns_domain_ok() {
    let lane0 = resolve_runtime_identity(DevLane::Lane0, RuntimeIdentityInput::default()).unwrap();
    assert_eq!(storage_namespace(&lane0), "domain-hi-0x10");

    let explicit = resolve_runtime_identity(
        DevLane::Lane1,
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

/// /v1/status reports explicit dev-lane namespace identifiers for default bootstraps.
#[tokio::test]
async fn v1_stat_default_lane_ns() {
    let svc = router_dev(app_from_devnet(DevLane::Lane0)).into_service();
    let res = svc
        .oneshot(Request::get("/v1/status").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["state_namespace"], "domain-hi-0x10");
    let expected = pwm_core::domain_index::lookup_regulatory_by_hi(0x10)
        .map(|entry| entry.label)
        .unwrap_or("0x10");
    assert_eq!(v["shard"], expected);
    assert_eq!(v["cluster_domain_hi"], 0x10);
    assert_eq!(v["bridge_exported_registry_size"], 0);
    assert_eq!(v["bridge_imported_set_size"], 0);
    assert_eq!(v["pipeline_metrics"]["enqueued"], 0);
    assert_eq!(v["pipeline_metrics"]["dequeued"], 0);
    assert_eq!(v["pipeline_metrics"]["rejected"], 0);
    assert_eq!(v["pipeline_metrics"]["validated"], 0);
    assert_eq!(v["pipeline_metrics"]["stale_validated"], 0);
    assert_eq!(v["pipeline_metrics"]["queue_depth_max"], 0);
    assert_eq!(v["pipeline_metrics"]["worker_wait_p50_ms"], 0);
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
    let app = app_from_devnet(DevLane::Lane0);
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

/// Neutral-runtime apps publish neutral shards/namespaces even when DevLane defaults to dev A.
#[tokio::test]
async fn v1_stat_neutral_relay() {
    let (cfg, sks) = dev_net();
    let identity = default_runtime_identity_neutral();
    let app = crate::bootstrap::app_from_chain_boot(cfg, sks, None, DevLane::Lane0, Some(identity));
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
    let svc = router_dev(mk_app_explicit_shard(DevLane::Lane1)).into_service();
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
    let svc = router_dev(app_from_devnet(DevLane::Lane0)).into_service();
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
    let app = app_from_devnet(DevLane::Lane0);
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

#[tokio::test]
async fn pending_cons_api_shape() {
    let app = app_from_devnet(DevLane::Lane0);
    let sender = fake_account_id_with_domain(0x1001);
    let other = fake_account_id_with_domain(0x1002);
    let recipient = fake_account_id_with_domain(0x1003);
    {
        let mut g = app.inner.write().await;
        g.chain.st.accounts.insert(
            sender,
            Account {
                balance_pwm: 88,
                initialized: true,
                ..Account::default()
            },
        );
        g.chain.st.accounts.insert(
            other,
            Account {
                balance_pwm: 77,
                initialized: true,
                ..Account::default()
            },
        );
        g.chain
            .st
            .pending_conservation
            .push(pwm_core::state::PendingConservationTransfer {
                sender,
                recipient,
                amount_pwm: 123,
                fee_pwm: 4,
                nonce: 5,
                enqueue_height: 6,
                execute_at_height: 9,
                tx_hash: [0xAB; 32],
            });
    }
    let svc = router_dev(app).into_service();
    let sender_res = svc
        .clone()
        .oneshot(
            Request::get(format!("/v1/account/{}", hex::encode(sender)))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(sender_res.status(), StatusCode::OK);
    let sender_bytes = to_bytes(sender_res.into_body(), 64 * 1024).await.unwrap();
    let sender_json: serde_json::Value = serde_json::from_slice(&sender_bytes).unwrap();
    let pending = sender_json["pending_conservation"]
        .as_array()
        .expect("pending_conservation array");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0]["recipient"], hex::encode(recipient));
    assert_eq!(pending[0]["amount_pwm"], "123");
    assert_eq!(pending[0]["fee_pwm"], "4");
    assert_eq!(pending[0]["nonce"], 5);
    assert_eq!(pending[0]["enqueue_height"], 6);
    assert_eq!(pending[0]["execute_at_height"], 9);

    let other_res = svc
        .oneshot(
            Request::get(format!("/v1/account/{}", hex::encode(other)))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(other_res.status(), StatusCode::OK);
    let other_bytes = to_bytes(other_res.into_body(), 64 * 1024).await.unwrap();
    let other_json: serde_json::Value = serde_json::from_slice(&other_bytes).unwrap();
    assert!(
        other_json.get("pending_conservation").is_none()
            || other_json["pending_conservation"]
                .as_array()
                .is_some_and(Vec::is_empty)
    );
}

/// /v1/accounts preserves the same split semantics for inserted local vs foreign-domain accounts.
#[tokio::test]
async fn v1_accts_foreign_split_ls() {
    let app = app_from_devnet(DevLane::Lane0);
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

/// /v1/perfmon exposes the four registered performance entities.
#[tokio::test]
async fn v1_perfmon_rows_ok() {
    let app = app_from_devnet(DevLane::Lane0);
    let svc = router_dev(app).into_service();
    let res = svc
        .oneshot(Request::get("/v1/perfmon").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let rows: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let rows = rows.as_array().expect("perfmon array");
    assert_eq!(rows.len(), 4);
    let names: Vec<&str> = rows.iter().filter_map(|row| row["name"].as_str()).collect();
    for name in ["ed25519_verify", "chain_seal", "pool_drain", "state_apply"] {
        assert!(names.contains(&name), "missing perfmon row {name}");
    }
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
    let mut app = app_for_devnet_sender(DevLane::Lane0);
    let pairs: Vec<_> = (0..16u8)
        .map(|i| {
            let sender_seed = 99u8.wrapping_add(i.wrapping_mul(2));
            let peer_seed = sender_seed.wrapping_add(1);
            let (sender_sk, sender_i, sender_aid) = plain_user_sk(sender_seed, &app);
            let (peer_sk, peer_i, peer_aid) = plain_user_sk(peer_seed, &app);
            (sender_sk, sender_i, sender_aid, peer_sk, peer_i, peer_aid)
        })
        .collect();
    let snapshot = {
        let mut g = app.inner.write().await;
        for (sender_sk, sender_i, sender_aid, peer_sk, peer_i, peer_aid) in &pairs {
            let sender_dom = domain_of_account_id(sender_aid);
            let peer_dom = domain_of_account_id(peer_aid);
            let init_sender = SignedTx::sign_body(
                sender_sk,
                sender_dom,
                *sender_i,
                0,
                TxBody::Init {
                    index: *sender_i,
                    flags: 0,
                },
            );
            g.chain
                .st
                .apply_tx(&init_sender)
                .expect("init sender account");
            g.chain
                .st
                .accounts
                .get_mut(sender_aid)
                .expect("sender account in snapshot")
                .balance_pwm = 50;
            let init_peer = SignedTx::sign_body(
                peer_sk,
                peer_dom,
                *peer_i,
                0,
                TxBody::Init {
                    index: *peer_i,
                    flags: 0,
                },
            );
            g.chain.st.apply_tx(&init_peer).expect("init peer account");
        }
        std::sync::Arc::new(g.chain.st.clone())
    };
    app.state_snapshot.store(snapshot);

    let snapshot_path = temp_path("lock_order_smoke");
    app.data_file = Some(snapshot_path.clone());
    let svc = router_dev(app.clone()).into_service();

    for (sender_sk, sender_i, sender_aid, _, _, peer_aid) in &pairs {
        let sender_dom = domain_of_account_id(sender_aid);
        let tx = SignedTx::sign_body(
            sender_sk,
            sender_dom,
            *sender_i,
            1,
            TxBody::Transfer {
                to: *peer_aid,
                amount: 1,
                fee: 0,
            },
        );
        let tx_body = serde_json::to_vec(&tx).unwrap();
        let status_fut = svc
            .clone()
            .oneshot(Request::get("/v1/status").body(Body::empty()).unwrap());
        let tx_fut = svc.clone().oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(tx_body))
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
    let svc = router_dev(app_for_sender(&aid)).into_service();
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

/// Underfunded non-roaming transfer is rejected by worker dry-run before ingress.
#[tokio::test]
async fn v1_tx_underfunded_xfer_mempool() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    let (sk0, i0, aid0) = routable_user_sk_for_app([99u8; 32], &app);
    let (sk1, i1, aid1) = routable_user_sk_for_app([100u8; 32], &app);
    let dom0 = domain_of_account_id(&aid0);
    let dom1 = domain_of_account_id(&aid1);
    let snapshot = {
        let mut g = app.inner.write().await;
        if !g.chain.st.accounts.contains_key(&aid0) {
            let init_sender = SignedTx::sign_body(
                &sk0,
                dom0,
                i0,
                0,
                TxBody::Init {
                    index: i0,
                    flags: 0,
                },
            );
            g.chain
                .st
                .apply_tx(&init_sender)
                .expect("init sender account");
        }
        let init_peer = SignedTx::sign_body(
            &sk1,
            dom1,
            i1,
            0,
            TxBody::Init {
                index: i1,
                flags: 0,
            },
        );
        g.chain.st.apply_tx(&init_peer).expect("init peer account");
        g.chain
            .st
            .accounts
            .get_mut(&aid0)
            .expect("sender")
            .balance_pwm = 5;
        std::sync::Arc::new(g.chain.st.clone())
    };
    app.state_snapshot.store(snapshot);
    let tx = SignedTx::sign_body(
        &sk0,
        dom0,
        i0,
        1,
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
    assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
    {
        let g = app.inner.read().await;
        assert_eq!(g.pool.len(), 0, "mempool must stay empty after reject");
    }
    let mut rx = app.tx_ingress.receiver.try_lock().expect("ingress rx");
    assert!(rx.try_recv().is_err(), "rejected tx must not enter ingress");
}

fn plain_user_sk(seed_start: u8, app: &App) -> (SigningKey, u32, pwm_core::AccountId) {
    for attempt in 0..4096u16 {
        let mut seed = [seed_start; 32];
        seed[0] = seed_start.wrapping_add(attempt as u8);
        seed[1] = (attempt >> 8) as u8;
        let (sk, idx, aid) = user_sk(&seed);
        if domain_of_account_id(&aid).to_be_bytes()[0] != app.identity.cluster_domain_hi {
            continue;
        }
        if pwm_core::types::cosign_non_dis(&aid) || pwm_core::types::conservation_flag(&aid) {
            continue;
        }
        let dom = domain_of_account_id(&aid);
        let probe = SignedTx::sign_body(
            &sk,
            dom,
            idx,
            0,
            TxBody::Init {
                index: idx,
                flags: 0,
            },
        );
        if validate_tx_shape(&probe).is_ok() {
            return (sk, idx, aid);
        }
    }
    panic!("failed to find plain account for app domain");
}

#[tokio::test]
async fn v1_tx_xfer_worker_once() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    let (sk0, i0, aid0) = plain_user_sk(101, &app);
    let (sk1, i1, aid1) = plain_user_sk(102, &app);
    let dom0 = domain_of_account_id(&aid0);
    let dom1 = domain_of_account_id(&aid1);
    let snapshot = {
        let mut g = app.inner.write().await;
        if !g.chain.st.accounts.contains_key(&aid0) {
            let init_sender = SignedTx::sign_body(
                &sk0,
                dom0,
                i0,
                0,
                TxBody::Init {
                    index: i0,
                    flags: 0,
                },
            );
            g.chain
                .st
                .apply_tx(&init_sender)
                .expect("init sender account");
        }
        let init_peer = SignedTx::sign_body(
            &sk1,
            dom1,
            i1,
            0,
            TxBody::Init {
                index: i1,
                flags: 0,
            },
        );
        g.chain.st.apply_tx(&init_peer).expect("init peer account");
        g.chain
            .st
            .accounts
            .get_mut(&aid0)
            .expect("sender")
            .balance_pwm = 50;
        std::sync::Arc::new(g.chain.st.clone())
    };
    app.state_snapshot.store(snapshot);
    let tx = SignedTx::sign_body(
        &sk0,
        dom0,
        i0,
        1,
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
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
    let mut rx = app.tx_ingress.receiver.try_lock().expect("ingress rx");
    assert!(rx.try_recv().is_err(), "valid tx must not enter ingress");

    let mut validated_rx = app._validated_rx.try_lock().expect("validated rx");
    let validated = validated_rx.try_recv().expect("valid tx queued by worker");
    assert_eq!(validated.tx.nonce, tx.nonce);
    assert_eq!(validated.tx.body, tx.body);
    assert!(
        validated_rx.try_recv().is_err(),
        "valid tx must be queued by worker exactly once"
    );

    let metrics = app.pipeline_metrics.snapshot();
    assert_eq!(metrics.enqueued, 1);
    assert_eq!(metrics.validated, 1);
    assert_eq!(metrics.rejected, 0);
}

#[tokio::test]
async fn v1_tx_event_sealed() {
    let (sk, i, aid) = routable_user_sk([33u8; 32]);
    let dom = domain_of_account_id(&aid);
    let tx = SignedTx::sign_body(&sk, dom, i, 0, TxBody::Init { index: 1, flags: 0 });
    let app = app_for_sender(&aid);
    let mut events = app.tx_events.subscribe();
    let svc = router_dev(app.clone()).into_service();
    let res = svc
        .clone()
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&tx).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    spawn_seal_loop(app.clone());
    let event = tokio::time::timeout(Duration::from_secs(10), events.recv())
        .await
        .expect("sealed event timeout")
        .expect("sealed event");
    match event {
        crate::pipeline::TxEvent::Sealed { txid, block_height } => {
            assert_eq!(txid, tx.tx_hash());
            assert_eq!(block_height, 1);
        }
        crate::pipeline::TxEvent::Rejected { .. } => panic!("unexpected reject event"),
    }

    let res = svc
        .oneshot(Request::get("/v1/perfmon").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let snapshot: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    eprintln!("perfmon: {:#}", snapshot);
    let rows = snapshot.as_array().expect("perfmon array");
    for name in ["chain_seal", "ed25519_verify"] {
        let calls = rows
            .iter()
            .find(|row| row["name"] == name)
            .and_then(|row| row["calls"].as_u64())
            .unwrap_or(0);
        assert!(calls > 0, "perfmon {name} calls must be > 0");
    }
}

async fn assert_preflight_apply_parity(
    app: &App,
    tx: SignedTx,
    expected_status: StatusCode,
    expected_code: &str,
    expected_class: &str,
    expected_tx_kind: &str,
) {
    let svc = router_dev(app.clone()).into_service();
    let res = svc
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&tx).expect("serialize tx")))
                .unwrap(),
        )
        .await
        .expect("tx request");
    assert_eq!(res.status(), expected_status);
    let body = to_bytes(res.into_body(), 64 * 1024)
        .await
        .expect("reject body");
    let preflight: serde_json::Value =
        serde_json::from_slice(&body).expect("preflight reject json");
    assert_eq!(preflight["ok"], false);
    assert_eq!(preflight["phase"], "preflight");
    assert_eq!(preflight["tx_kind"], expected_tx_kind);
    assert_eq!(preflight["response_class"], expected_class);
    assert_eq!(preflight["error"]["code"], expected_code);
    let preflight_trace_id = preflight["error"]["trace_id"]
        .as_str()
        .expect("trace_id")
        .to_string();
    assert!(
        !preflight_trace_id.is_empty(),
        "trace_id must be present in preflight reject"
    );

    let apply_err = {
        let g = app.inner.read().await;
        let (next_h, next_ts) = g.chain.next_apply_ctx().expect("apply ctx");
        g.chain
            .st
            .clone()
            .apply_tx_with_ctx(&tx, next_h, next_ts, &g.chain.cfg)
            .expect_err("tx must reject on apply")
    };
    let apply_json = crate::api::common::tx_reject_json(
        &tx,
        "apply",
        &apply_err,
        format!("tx apply rejected: {apply_err}"),
    );
    let apply: serde_json::Value = serde_json::from_str(&apply_json).expect("apply reject json");
    assert_eq!(apply["phase"], "apply");
    assert_eq!(apply["tx_kind"], expected_tx_kind);
    assert_eq!(apply["error"]["code"], expected_code);
    assert_eq!(apply["response_class"], expected_class);
    let apply_trace_id = apply["error"]["trace_id"].as_str().expect("apply trace_id");
    assert_eq!(preflight_trace_id, apply_trace_id);
}

/// Burn purpose invalid keeps stable wire fields and preflight/apply parity.
#[tokio::test]
async fn v1_tx_burn_purpose_bad() {
    let (cfg, sks) = dev_net();
    let sk = &sks[0];
    let sender = cfg.accounts[0].acct;
    let sender_dom = domain_of_account_id(&sender);
    let mut tx = SignedTx::sign_body(
        sk,
        sender_dom,
        0,
        0,
        TxBody::BurnMark {
            mark_amount: 1,
            beneficiary: None,
        },
    );
    tx.set_burn_purpose_signed(sk, "   ".to_string());
    let app = app_for_devnet_sender(DevLane::Lane0);
    assert_preflight_apply_parity(
        &app,
        tx,
        StatusCode::BAD_REQUEST,
        "E_BURN_SCHEMA_INVALID",
        "VALIDATION_ERROR",
        "burn",
    )
    .await;
}

/// Burn toward a foreign-domain beneficiary is accepted (V2-7 cross-domain burn policy).
#[tokio::test]
async fn v1_burn_cross_domain_ok() {
    let (cfg, sks) = dev_net();
    let sk = &sks[0];
    let sender = cfg.accounts[0].acct;
    let sender_dom = domain_of_account_id(&sender);
    let ben = fake_account_id_with_domain(0x2001);
    assert_ne!(
        sender_dom.to_be_bytes()[0],
        domain_of_account_id(&ben).to_be_bytes()[0],
        "fixture must cross domain-hi boundary"
    );
    let tx = SignedTx::sign_body(
        sk,
        sender_dom,
        0,
        0,
        TxBody::BurnMark {
            mark_amount: 1,
            beneficiary: Some(ben),
        },
    );
    let app = app_for_devnet_sender(DevLane::Lane0);
    let svc = router_dev(app).into_service();
    let res = svc
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&tx).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);
}

/// Import fee too low keeps policy reject parity and stable wire fields on submit.
#[tokio::test]
async fn v1_tx_import_fee_low() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    let (import_sk, import_i, import_aid, export_id, import_provenance) =
        seed_handoff_provenance_for_import(&app, 11).await;
    let import_dom = domain_of_account_id(&import_aid);
    let nonce = {
        let g = app.inner.read().await;
        g.chain.st.get(&import_aid).expect("import account").nonce
    };
    let mut tx = SignedTx::sign_body(
        &import_sk,
        import_dom,
        import_i,
        nonce,
        TxBody::Import {
            to: import_aid,
            amount: 11,
            export_id,
        },
    );
    tx.set_import_provenance_signed(&import_sk, Some(import_provenance));
    tx.set_import_fee_signed(&import_sk, pwm_core::tx::MIN_IMPORT_FEE_UNITS - 1);
    assert_preflight_apply_parity(
        &app,
        tx,
        StatusCode::BAD_REQUEST,
        "E_IMPORT_FEE_TOO_LOW",
        "POLICY_REJECT",
        "import",
    )
    .await;
}
