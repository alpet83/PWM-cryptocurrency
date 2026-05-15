//! `/v1/peer/hello` handler tests and handshake-visible stats.

use super::helpers::*;
use super::prelude::*;

/// POST /v1/peer/hello accepts a matching native-domain hello and classifies counters on /v1/dev/peers.
#[tokio::test]
async fn v1_hi_accepts_native_cls() {
    let app = app_from_devnet(DevLane::Lane0);
    let hello = sample_hello(&app, "peer-native", 0x10, vec![1, 2, 3, 4]);
    let svc = router_dev(app.clone()).into_service();
    let res = svc
        .clone()
        .oneshot(
            Request::post("/v1/peer/hello")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&hello).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["accepted"], true);
    assert_eq!(v["class"], "native");

    let stats = svc
        .oneshot(Request::get("/v1/dev/peers").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let stats_body = to_bytes(stats.into_body(), 64 * 1024).await.unwrap();
    let sv: serde_json::Value = serde_json::from_slice(&stats_body).unwrap();
    assert_eq!(sv["accepted_total"], 1);
    assert_eq!(sv["class_accept_total"]["native"], 1);
    assert_eq!(sv["connected_by_class"]["native"], 1);
}

/// Peer hello rejects bad signatures, nonce replay branching, wrong network/genesis hashes, and malformed capabilities.
#[tokio::test]
async fn v1_hi_mx_sig() {
    let app = app_from_devnet(DevLane::Lane0);
    let mut bad_sig = sample_hello(&app, "peer-bad-sig", 0x10, vec![10, 11, 12]);
    bad_sig.signature[0] ^= 0x55;

    let replay = sample_hello(&app, "peer-replay", 0x10, vec![21, 22, 23]);
    let mut network_bad = sample_hello(&app, "peer-net", 0x10, vec![31, 32, 33]);
    network_bad.network_id = "othernet".to_string();
    network_bad
        .sign(&SigningKey::from_bytes(&[9u8; 32]))
        .unwrap();
    let mut genesis_bad = sample_hello(&app, "peer-genesis", 0x10, vec![41, 42, 43]);
    genesis_bad.genesis_hash = Some("wrong".to_string());
    genesis_bad
        .sign(&SigningKey::from_bytes(&[9u8; 32]))
        .unwrap();
    let mut malformed = sample_hello(&app, "peer-malformed", 0x10, vec![51, 52, 53]);
    malformed.capabilities.services = vec!["".to_string()];

    let svc = router_dev(app.clone()).into_service();
    for (hello, reason) in [
        (bad_sig.clone(), "bad_signature"),
        (replay.clone(), ""),
        (replay.clone(), "replay_nonce"),
        (network_bad, "network_mismatch"),
        (genesis_bad, "genesis_mismatch"),
        (malformed, "malformed"),
    ] {
        let res = svc
            .clone()
            .oneshot(
                Request::post("/v1/peer/hello")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&hello).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let body = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
        if reason.is_empty() {
            assert_eq!(v["accepted"], true);
        } else {
            assert_eq!(v["accepted"], false);
            assert_eq!(v["reason"], reason);
        }
    }
}

/// Genesis mismatch peer hello blocks /v1/tx and roaming exports with user-tx-blocked errors.
#[tokio::test]
async fn v1_gen_mismatch_blocks_tx() {
    let app = app_from_devnet(DevLane::Lane0);
    let mut genesis_bad = sample_hello(&app, "peer-genesis-block", 0x10, vec![91, 92, 93]);
    genesis_bad.genesis_hash = Some("wrong".to_string());
    genesis_bad
        .sign(&SigningKey::from_bytes(&[9u8; 32]))
        .unwrap();

    let svc = router_dev(app.clone()).into_service();
    let reject = svc
        .clone()
        .oneshot(
            Request::post("/v1/peer/hello")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&genesis_bad).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reject.status(), StatusCode::OK);
    let reject_body = to_bytes(reject.into_body(), 64 * 1024).await.unwrap();
    let reject_json: serde_json::Value = serde_json::from_slice(&reject_body).unwrap();
    assert_eq!(reject_json["accepted"], false);
    assert_eq!(reject_json["reason"], "genesis_mismatch");

    let (cfg, sks) = dev_net();
    let sender = &sks[0];
    let sender_dom = domain_of_account_id(&cfg.accounts[0].acct);
    let local_tx = SignedTx::sign_body(
        sender,
        sender_dom,
        0,
        0,
        TxBody::Transfer {
            to: cfg.accounts[0].acct,
            amount: 1,
            fee: 0,
        },
    );
    let tx_res = svc
        .clone()
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&local_tx).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(tx_res.status(), StatusCode::SERVICE_UNAVAILABLE);
    let tx_body = to_bytes(tx_res.into_body(), 64 * 1024).await.unwrap();
    assert!(String::from_utf8_lossy(&tx_body).contains("user tx blocked"));

    let export_tx = SignedTx::sign_body(
        sender,
        sender_dom,
        0,
        0,
        TxBody::Export {
            to: valid_cross_domain_recipient(sender_dom.to_be_bytes()[0]),
            target_domain: 0x2001,
            amount: 1,
            fee: 0,
        },
    );
    let roam_body = serde_json::json!({ "tx": export_tx, "ttl_blocks": 5 });
    let roam_res = svc
        .oneshot(
            Request::post("/v1/roaming-intents")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&roam_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(roam_res.status(), StatusCode::SERVICE_UNAVAILABLE);
    let roam_text = to_bytes(roam_res.into_body(), 64 * 1024).await.unwrap();
    assert!(String::from_utf8_lossy(&roam_text).contains("user tx blocked"));
}

/// /v1/status surfaces genesis_guard fields and mismatch diagnostics after a rejected hello.
#[tokio::test]
async fn v1_status_gen_guard_diag() {
    let app = app_from_devnet(DevLane::Lane0);
    let mut genesis_bad = sample_hello(&app, "peer-genesis-status", 0x10, vec![101, 102, 103]);
    genesis_bad.genesis_hash = Some("wrong".to_string());
    genesis_bad
        .sign(&SigningKey::from_bytes(&[9u8; 32]))
        .unwrap();

    let svc = router_dev(app).into_service();
    let _ = svc
        .clone()
        .oneshot(
            Request::post("/v1/peer/hello")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&genesis_bad).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = svc
        .oneshot(Request::get("/v1/status").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(status.status(), StatusCode::OK);
    let body = to_bytes(status.into_body(), 64 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["genesis_guard"], "blocked");
    assert_eq!(json["genesis_mismatch_total"], 1);
    assert_eq!(json["genesis_mismatch_received_hash"], "wrong");
    assert_eq!(json["genesis_mismatch_peer_id"], "peer-genesis-status");
    assert_eq!(json["genesis_mismatch_peer_hint"], "http");
    assert!(json["effective_genesis_hash"]
        .as_str()
        .map(|x| !x.is_empty())
        .unwrap_or(false));
    assert_eq!(
        json["genesis_mismatch_expected_hash"],
        json["effective_genesis_hash"]
    );
    assert!(json["genesis_mismatch_unix_ms"].as_u64().unwrap_or(0) > 0);
    assert!(json["genesis_guard_recovery_hint"]
        .as_str()
        .unwrap_or_default()
        .contains("stop node"));
    assert!(json["last_peer_error"]
        .as_str()
        .unwrap_or_default()
        .contains("genesis_mismatch"));
    assert!(json["peer_error_at_ms"].as_u64().unwrap_or(0) > 0);
}

/// Inbound accepted foreign hellos increase live_peer_count without satisfying trusted relay health.
#[tokio::test]
async fn inbound_hi_no_relay_ok() {
    let app = app_from_devnet(DevLane::Lane0);
    {
        let mut cfg = app.transport_config.write().await;
        cfg.peer_seeds = vec![SocketAddr::from(([127, 0, 0, 1], 1))];
    }
    let hello = sample_hello(&app, "inbound-peer", 0x20, vec![111, 112, 113]);
    let svc = router_dev(app).into_service();
    let accepted = svc
        .clone()
        .oneshot(
            Request::post("/v1/peer/hello")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&hello).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(accepted.status(), StatusCode::OK);

    let status = svc
        .oneshot(Request::get("/v1/status").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let body = to_bytes(status.into_body(), 64 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["live_peer_count"], 1);
    assert_eq!(json["trusted_relay_peer_count"], 0);
    assert_eq!(json["peer_relay_health"], "no_trusted_seed");
    assert!(json["roaming_relay_hint"]
        .as_str()
        .unwrap_or_default()
        .contains("live trusted seed peer"));
}

#[tokio::test]
async fn network_mismatch_sets_status_diagnostic() {
    let app = app_from_devnet(DevLane::Lane0);
    let mut hello = sample_hello(&app, "peer-wrong-net", 0x10, vec![121, 122, 123]);
    hello.network_id = "wrongnet".to_string();
    hello.sign(&SigningKey::from_bytes(&[9u8; 32])).unwrap();

    let svc = router_dev(app).into_service();
    let rejected = svc
        .clone()
        .oneshot(
            Request::post("/v1/peer/hello")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&hello).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::OK);
    let rejected_body = to_bytes(rejected.into_body(), 64 * 1024).await.unwrap();
    let rejected_json: serde_json::Value = serde_json::from_slice(&rejected_body).unwrap();
    assert_eq!(rejected_json["accepted"], false);
    assert_eq!(rejected_json["reason"], "network_mismatch");

    let status = svc
        .oneshot(Request::get("/v1/status").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let body = to_bytes(status.into_body(), 64 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let err = json["last_peer_error"].as_str().unwrap_or_default();
    assert!(err.contains("network_mismatch"));
    assert!(err.contains("expected_network_id=devnet"));
    assert!(err.contains("received_network_id=wrongnet"));
}

/// Foreign hellos classify accept/reject paths and update dev peer stats and reject_reason counters.
#[tokio::test]
async fn v1_hi_foreign_reject_ctr() {
    let app = app_from_devnet(DevLane::Lane0);
    let foreign_ok = sample_hello(&app, "peer-foreign", 0x20, vec![61, 62, 63]);
    let mut bad_sig = sample_hello(&app, "peer-foreign-bad", 0x20, vec![71, 72, 73]);
    bad_sig.signature[0] ^= 0xAA;

    let svc = router_dev(app).into_service();
    let accepted = svc
        .clone()
        .oneshot(
            Request::post("/v1/peer/hello")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&foreign_ok).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(accepted.status(), StatusCode::OK);
    let accepted_body = to_bytes(accepted.into_body(), 64 * 1024).await.unwrap();
    let accepted_json: serde_json::Value = serde_json::from_slice(&accepted_body).unwrap();
    assert_eq!(accepted_json["accepted"], true);
    assert_eq!(accepted_json["class"], "foreign");

    let rejected = svc
        .clone()
        .oneshot(
            Request::post("/v1/peer/hello")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&bad_sig).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::OK);
    let rejected_body = to_bytes(rejected.into_body(), 64 * 1024).await.unwrap();
    let rejected_json: serde_json::Value = serde_json::from_slice(&rejected_body).unwrap();
    assert_eq!(rejected_json["accepted"], false);
    assert_eq!(rejected_json["reason"], "bad_signature");

    let stats = svc
        .oneshot(Request::get("/v1/dev/peers").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(stats.status(), StatusCode::OK);
    let stats_body = to_bytes(stats.into_body(), 64 * 1024).await.unwrap();
    let stats_json: serde_json::Value = serde_json::from_slice(&stats_body).unwrap();
    assert_eq!(stats_json["accepted_total"], 1);
    assert_eq!(stats_json["rejected_total"], 1);
    assert_eq!(stats_json["class_accept_total"]["foreign"], 1);
    assert_eq!(stats_json["connected_by_class"]["foreign"], 1);
    assert_eq!(stats_json["reject_reason_total"]["bad_signature"], 1);
    assert_eq!(stats_json["peers"].as_array().map(|v| v.len()), Some(1));
    assert_eq!(stats_json["peers"][0]["node_id"], "peer-foreign");
    assert_eq!(stats_json["peers"][0]["class"], "foreign");
    assert_eq!(stats_json["peers"][0]["domain_hi"], 0x20);
}

/// prioritize_peer_candidates orders native before foreign peers using only domain equality (no range heuristics).
#[test]
fn policy_native_first_plain() {
    let local_domain_hi = 0x10;
    let mut peers = HashMap::new();
    peers.insert(
        "foreign-low".to_string(),
        PeerRecord {
            node_id: "foreign-low".to_string(),
            domain_hi: 0x01,
            class: PeerClass::Foreign,
            last_seen_ms: 100,
            status: PeerStatus::Accepted,
        },
    );
    peers.insert(
        "native".to_string(),
        PeerRecord {
            node_id: "native".to_string(),
            domain_hi: 0x10,
            class: PeerClass::Native,
            last_seen_ms: 90,
            status: PeerStatus::Accepted,
        },
    );
    peers.insert(
        "foreign-high".to_string(),
        PeerRecord {
            node_id: "foreign-high".to_string(),
            domain_hi: 0xF0,
            class: PeerClass::Foreign,
            last_seen_ms: 110,
            status: PeerStatus::Accepted,
        },
    );
    let ordered = prioritize_peer_candidates(local_domain_hi, &peers);
    assert_eq!(ordered[0].node_id, "native");
    assert_eq!(ordered[1].node_id, "foreign-high");
    assert_eq!(ordered[2].node_id, "foreign-low");
}

/// Backoff selection returns distinct envelope parameters for native vs foreign classes.
#[test]
fn policy_backoff_env_cls() {
    let mut policy = PeerPolicySnapshot {
        config: PeerPolicyConfig::default(),
        counters: PeerPolicyCounters::default(),
        native_live: 0,
        native_degraded_state: true,
    };
    let native = select_backoff_for_class(&mut policy, &PeerClass::Native);
    let foreign = select_backoff_for_class(&mut policy, &PeerClass::Foreign);
    assert_ne!(native, foreign);
    assert_eq!(native.base_ms, 250);
    assert_eq!(foreign.base_ms, 1_000);
    assert_eq!(policy.counters.backoff_select_native, 1);
    assert_eq!(policy.counters.backoff_select_foreign, 1);
}

/// native_degraded_state flips with native_min_live vs observed native peer counts (with flip counter).
#[test]
fn policy_nat_deg_min_live() {
    let mut policy = PeerPolicySnapshot {
        config: PeerPolicyConfig::default(),
        counters: PeerPolicyCounters::default(),
        native_live: 0,
        native_degraded_state: true,
    };
    policy.config.native_min_live = 2;
    refresh_native_degraded_state(&mut policy, 2);
    assert!(!policy.native_degraded_state);
    refresh_native_degraded_state(&mut policy, 1);
    assert!(policy.native_degraded_state);
    assert_eq!(policy.counters.native_degraded_flips, 2);
}

/// classify_peer treats only exact domain_hi equality as native; adjacent values stay foreign.
#[test]
fn policy_cls_domain_eq() {
    let local_domain_hi = 0x80;
    assert_eq!(classify_peer(local_domain_hi, 0x80), PeerClass::Native);
    assert_eq!(classify_peer(local_domain_hi, 0x7F), PeerClass::Foreign);
    assert_eq!(classify_peer(local_domain_hi, 0x81), PeerClass::Foreign);
}

/// Transport tick dials native peers before foreign ones and honors per-class backoff skip counters.
#[test]
fn xfer_sched_native_backoff() {
    let validation_ctx = HandshakeValidationCtx {
        expected_network_id: "devnet".to_string(),
        expected_genesis_hash: None,
        skew_window_ms: 30_000,
    };
    let mut hs = HandshakeState::new(validation_ctx, 0x10);
    hs.peers.insert(
        "foreign".to_string(),
        PeerRecord {
            node_id: "foreign".to_string(),
            domain_hi: 0x20,
            class: PeerClass::Foreign,
            last_seen_ms: 200,
            status: PeerStatus::Accepted,
        },
    );
    hs.peers.insert(
        "native".to_string(),
        PeerRecord {
            node_id: "native".to_string(),
            domain_hi: 0x10,
            class: PeerClass::Native,
            last_seen_ms: 100,
            status: PeerStatus::Accepted,
        },
    );
    hs.policy.config.native_outbound_target = 1;
    hs.policy.config.foreign_outbound_target = 1;
    let mut seen_order = Vec::new();
    run_transport_tick_with(&mut hs, 1_000, |p| {
        seen_order.push(p.node_id.clone());
        DialAttemptResult::RetryableFail
    });
    assert_eq!(
        seen_order,
        vec!["native".to_string(), "foreign".to_string()]
    );
    run_transport_tick_with(&mut hs, 1_100, |_| DialAttemptResult::RetryableFail);
    assert_eq!(hs.transport.snapshot.counters.backoff_skip_total, 2);
}

/// Repeated retryable failures advance next_due_ms along the capped exponential backoff schedule.
#[test]
fn xfer_retry_backoff_env() {
    let validation_ctx = HandshakeValidationCtx {
        expected_network_id: "devnet".to_string(),
        expected_genesis_hash: None,
        skew_window_ms: 30_000,
    };
    let mut hs = HandshakeState::new(validation_ctx, 0x10);
    hs.peers.insert(
        "foreign".to_string(),
        PeerRecord {
            node_id: "foreign".to_string(),
            domain_hi: 0x20,
            class: PeerClass::Foreign,
            last_seen_ms: 1,
            status: PeerStatus::Accepted,
        },
    );
    hs.policy.config.native_outbound_target = 0;
    hs.policy.config.foreign_outbound_target = 1;
    run_transport_tick_with(&mut hs, 1_000, |_| DialAttemptResult::RetryableFail);
    let st = hs.transport.peers.get("foreign").expect("foreign state");
    assert_eq!(st.attempts, 1);
    assert_eq!(st.next_due_ms, 2_000);
    run_transport_tick_with(&mut hs, 2_000, |_| DialAttemptResult::RetryableFail);
    let st = hs.transport.peers.get("foreign").expect("foreign state");
    assert_eq!(st.attempts, 2);
    assert_eq!(st.next_due_ms, 4_000);
    run_transport_tick_with(&mut hs, 3_000, |_| DialAttemptResult::RetryableFail);
    assert_eq!(hs.transport.snapshot.counters.backoff_skip_total, 1);
    run_transport_tick_with(&mut hs, 4_000, |_| DialAttemptResult::RetryableFail);
    let st = hs.transport.peers.get("foreign").expect("foreign state");
    assert_eq!(st.attempts, 3);
    assert_eq!(st.next_due_ms, 8_000);
}

/// native_degraded_state activates only after sustained native-peer underflow across transport ticks.
#[test]
fn xfer_deg_underflow_ticks() {
    let validation_ctx = HandshakeValidationCtx {
        expected_network_id: "devnet".to_string(),
        expected_genesis_hash: None,
        skew_window_ms: 30_000,
    };
    let mut hs = HandshakeState::new(validation_ctx, 0x10);
    hs.policy.config.native_min_live = 1;
    hs.peers.insert(
        "foreign".to_string(),
        PeerRecord {
            node_id: "foreign".to_string(),
            domain_hi: 0x20,
            class: PeerClass::Foreign,
            last_seen_ms: 10,
            status: PeerStatus::Accepted,
        },
    );
    run_transport_tick(&mut hs, 1_000);
    assert!(!hs.transport.snapshot.native_degraded_state);
    run_transport_tick(&mut hs, 2_000);
    assert!(!hs.transport.snapshot.native_degraded_state);
    run_transport_tick(&mut hs, 3_000);
    assert!(hs.transport.snapshot.native_degraded_state);
    assert_eq!(hs.transport.snapshot.native_degraded_transitions, 1);
    hs.peers.insert(
        "native".to_string(),
        PeerRecord {
            node_id: "native".to_string(),
            domain_hi: 0x10,
            class: PeerClass::Native,
            last_seen_ms: 20,
            status: PeerStatus::Accepted,
        },
    );
    run_transport_tick(&mut hs, 4_000);
    assert_eq!(hs.transport.snapshot.native_underflow_ticks, 0);
    assert!(!hs.transport.snapshot.native_degraded_state);
    assert_eq!(hs.transport.snapshot.native_degraded_transitions, 2);
}

/// /v1/dev/peers echoes transport.tick counters plus dial_attempt_class_result rollups.
#[tokio::test]
async fn v1_dev_peers_xfer_snap() {
    let app = app_from_devnet(DevLane::Lane0);
    {
        let mut hs = app.handshake.write().await;
        hs.peers.insert(
            "native".to_string(),
            PeerRecord {
                node_id: "native".to_string(),
                domain_hi: 0x10,
                class: PeerClass::Native,
                last_seen_ms: 100,
                status: PeerStatus::Accepted,
            },
        );
        hs.peers.insert(
            "foreign".to_string(),
            PeerRecord {
                node_id: "foreign".to_string(),
                domain_hi: 0x20,
                class: PeerClass::Foreign,
                last_seen_ms: 90,
                status: PeerStatus::Accepted,
            },
        );
        run_transport_tick(&mut hs, 1_000);
    }
    let svc = router_dev(app).into_service();
    let res = svc
        .oneshot(Request::get("/v1/dev/peers").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["transport"]["ticks_total"], 1);
    assert_eq!(
        json["transport"]["counters"]["dial_attempt_class_result"]["native:success"],
        1
    );
    assert_eq!(
        json["transport"]["counters"]["dial_attempt_class_result"]["foreign:retryable_fail"],
        1
    );
    assert_eq!(json["soak"]["loop_ticks_capped"], 0);
    assert_eq!(json["soak"]["runaway_stop_total"], 0);
}

/// real transport tick completes seed dial, handshake acceptance, and trust bookkeeping.
#[tokio::test]
async fn real_xfer_seed_hs_ok() {
    let app = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-a");
    let seed_app = app_with_identity(DevLane::Lane1, "testnet-qa", 0x20, "cluster-b", "node-b");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let cfg = TransportConfig {
        enabled: true,
        peer_seeds: vec![addr],
        connect_timeout_ms: 1_000,
        handshake_timeout_ms: 1_000,
        retry_base_ms: 200,
        retry_max_ms: 2_000,
        ..TransportConfig::default()
    };
    {
        let mut tc = seed_app.transport_config.write().await;
        *tc = cfg.clone();
    }
    tokio::spawn(async move {
        axum::serve(listener, router_dev(seed_app)).await.unwrap();
    });
    run_real_transport_tick(&app, &cfg, current_time_ms().unwrap()).await;
    let hs = app.handshake.read().await;
    assert_eq!(hs.metrics.accepted_total, 1);
    assert!(hs.peers.contains_key("node-b"));
    assert!(hs.trusted_peers.contains_key("node-b"));
    assert!(hs.transport.snapshot.last_peer_error.is_none());
    assert_eq!(
        hs.transport
            .snapshot
            .counters
            .dial_attempt_class_result
            .get("foreign:success")
            .copied(),
        Some(1)
    );
}

fn reserve_loopback_addr() -> SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("reserve addr");
    listener.local_addr().expect("local addr")
}

async fn read_wire_frame(stream: &mut tokio::net::TcpStream) -> Vec<u8> {
    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .await
        .expect("read frame len");
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut payload = vec![0u8; len];
    stream
        .read_exact(&mut payload)
        .await
        .expect("read frame payload");
    payload
}

async fn try_read_wire_frame(stream: &mut tokio::net::TcpStream) -> std::io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload).await?;
    Ok(payload)
}

async fn write_wire_payload(stream: &mut tokio::net::TcpStream, payload: &[u8]) {
    let len = u32::try_from(payload.len()).expect("payload len");
    stream
        .write_all(&len.to_be_bytes())
        .await
        .expect("write frame len");
    stream
        .write_all(payload)
        .await
        .expect("write frame payload");
}

/// Dedicated peer_listen sockets establish a trusted stateful peer session via outbound seed dialing.
#[tokio::test]
async fn xfer_state_sock_connect() {
    let app_a = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-a");
    let app_b = app_with_identity(DevLane::Lane1, "testnet-qa", 0x20, "cluster-b", "node-b");
    let peer_a = reserve_loopback_addr();
    let peer_b = reserve_loopback_addr();
    let mut cfg_a = TransportConfig::default();
    cfg_a.enabled = true;
    cfg_a.peer_listen = peer_a;
    cfg_a.peer_seeds = vec![peer_b];
    cfg_a.retry_base_ms = 50;
    cfg_a.heartbeat_interval_ms = 80;
    cfg_a.heartbeat_timeout_ms = 250;
    let mut cfg_b = TransportConfig::default();
    cfg_b.enabled = true;
    cfg_b.peer_listen = peer_b;
    cfg_b.retry_base_ms = 50;
    cfg_b.heartbeat_interval_ms = 80;
    cfg_b.heartbeat_timeout_ms = 250;
    spawn_peer_listener_loop(app_a.clone(), cfg_a.clone());
    spawn_peer_listener_loop(app_b.clone(), cfg_b.clone());
    spawn_stateful_transport_loop(app_a.clone(), cfg_a);
    let ok = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let hs = app_a.handshake.read().await;
            if hs.trusted_peers.contains_key("node-b") {
                break;
            }
            drop(hs);
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(ok.is_ok(), "stateful session did not establish");
    let hs = app_a.handshake.read().await;
    assert!(hs.transport.snapshot.session_connected_total >= 1);
    assert!(hs.transport.snapshot.session_trusted_total >= 1);
    assert_eq!(hs.transport.snapshot.last_session_close_reason, None);
}

/// Symmetric peer_listen + dual stateful loops keep bidirectional trusted sessions without churn counters.
#[tokio::test]
async fn xfer_state_bidir_stable() {
    let app_a = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-a");
    let app_b = app_with_identity(DevLane::Lane1, "testnet-qa", 0x20, "cluster-b", "node-b");
    let peer_a = reserve_loopback_addr();
    let peer_b = reserve_loopback_addr();
    let mut cfg_a = TransportConfig::default();
    cfg_a.enabled = true;
    cfg_a.peer_listen = peer_a;
    cfg_a.peer_seeds = vec![peer_b];
    cfg_a.retry_base_ms = 50;
    cfg_a.heartbeat_interval_ms = 80;
    cfg_a.heartbeat_timeout_ms = 250;
    let mut cfg_b = TransportConfig::default();
    cfg_b.enabled = true;
    cfg_b.peer_listen = peer_b;
    cfg_b.peer_seeds = vec![peer_a];
    cfg_b.retry_base_ms = 50;
    cfg_b.heartbeat_interval_ms = 80;
    cfg_b.heartbeat_timeout_ms = 250;
    spawn_peer_listener_loop(app_a.clone(), cfg_a.clone());
    spawn_peer_listener_loop(app_b.clone(), cfg_b.clone());
    spawn_stateful_transport_loop(app_a.clone(), cfg_a);
    spawn_stateful_transport_loop(app_b.clone(), cfg_b);
    let connected = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let hs_a = app_a.handshake.read().await;
            let hs_b = app_b.handshake.read().await;
            let ok = hs_a.trusted_peers.contains_key("node-b")
                && hs_b.trusted_peers.contains_key("node-a");
            drop(hs_a);
            drop(hs_b);
            if ok {
                break;
            }
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
    })
    .await;
    assert!(
        connected.is_ok(),
        "bidirectional trusted session not established"
    );
    tokio::time::sleep(Duration::from_millis(900)).await;
    let hs_a = app_a.handshake.read().await;
    assert_eq!(
        hs_a.transport.snapshot.session_disconnected_total, 0,
        "unexpected disconnect indicates session churn"
    );
    assert!(
        hs_a.transport.snapshot.session_connected_total <= 2,
        "unexpected reconnect churn detected"
    );
}

/// Same-shard follower (cluster disabled) converges to a cluster-enabled source tip over steady bidirectional TCP.
#[tokio::test]
async fn same_shard_follower_tcp_tip() {
    let mut app_src = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-a");
    app_src.cluster_cfg.enabled = true;
    app_src.cluster_cfg.role = crate::handshake::ClusterRole::Proposer;
    app_src.cluster_cfg.members = vec!["node-a".to_string(), "node-b".to_string()];
    app_src.cluster_cfg.quorum_n = 2;
    app_src.cluster_cfg.quorum_k = 1;
    app_src.node_instance_id = "node-a".to_string();
    {
        let mut g = app_src.inner.write().await;
        g.chain
            .seal(Vec::new())
            .expect("source must seal one block");
    }
    let (src_tip_h, src_tip_hash) = {
        let g = app_src.inner.read().await;
        (g.chain.tip_h(), hex::encode(g.chain.tip_hash()))
    };
    assert!(src_tip_h >= 1, "source tip must be above genesis");
    let mut app_follow =
        app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-b");
    app_follow.cluster_cfg.enabled = false;
    app_follow.cluster_cfg.role = crate::handshake::ClusterRole::None;
    let peer_src = reserve_loopback_addr();
    let peer_follow = reserve_loopback_addr();
    let mut cfg_src = TransportConfig::default();
    cfg_src.enabled = true;
    cfg_src.peer_listen = peer_src;
    cfg_src.peer_seeds = vec![peer_follow];
    cfg_src.retry_base_ms = 50;
    cfg_src.heartbeat_interval_ms = 80;
    cfg_src.heartbeat_timeout_ms = 250;
    let mut cfg_follow = TransportConfig::default();
    cfg_follow.enabled = true;
    cfg_follow.peer_listen = peer_follow;
    cfg_follow.peer_seeds = vec![peer_src];
    cfg_follow.retry_base_ms = 50;
    cfg_follow.heartbeat_interval_ms = 80;
    cfg_follow.heartbeat_timeout_ms = 250;
    let follow_div_base = {
        let hs = app_follow.handshake.read().await;
        hs.transport.snapshot.sync_tip_disconnect_total
    };
    let src_div_base = {
        let hs = app_src.handshake.read().await;
        hs.transport.snapshot.sync_tip_disconnect_total
    };
    spawn_peer_listener_loop(app_src.clone(), cfg_src.clone());
    spawn_peer_listener_loop(app_follow.clone(), cfg_follow.clone());
    spawn_stateful_transport_loop(app_src.clone(), cfg_src);
    spawn_stateful_transport_loop(app_follow.clone(), cfg_follow);
    let converged = tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let (tip_h, tip_hash) = {
                let g = app_follow.inner.read().await;
                (g.chain.tip_h(), hex::encode(g.chain.tip_hash()))
            };
            if tip_h == src_tip_h && tip_hash == src_tip_hash {
                break;
            }
            tokio::time::sleep(Duration::from_millis(80)).await;
        }
    })
    .await;
    assert!(
        converged.is_ok(),
        "follower did not converge to source tip_h={src_tip_h} hash={src_tip_hash}"
    );
    let hs_follow = app_follow.handshake.read().await;
    assert_eq!(
        hs_follow.transport.snapshot.sync_tip_disconnect_total, follow_div_base,
        "follower divergence disconnect counter must not grow"
    );
    drop(hs_follow);
    let hs_src = app_src.handshake.read().await;
    assert_eq!(
        hs_src.transport.snapshot.sync_tip_disconnect_total, src_div_base,
        "source divergence disconnect counter must not grow"
    );
}

/// Synthetic account_views traffic satisfies heartbeat freshness for a trusted AccountViews stream.
#[tokio::test]
async fn xfer_st_data_hb_ok() {
    let app_a = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-a");
    let app_b = app_with_identity(DevLane::Lane1, "testnet-qa", 0x20, "cluster-b", "node-b");
    let peer_a = reserve_loopback_addr();
    let seed_addr = reserve_loopback_addr();
    let mut cfg_a = TransportConfig::default();
    cfg_a.enabled = true;
    cfg_a.peer_listen = peer_a;
    cfg_a.peer_seeds = vec![seed_addr];
    cfg_a.retry_base_ms = 50;
    cfg_a.handshake_timeout_ms = 300;
    cfg_a.heartbeat_interval_ms = 50;
    cfg_a.heartbeat_timeout_ms = 100;
    let listener = TcpListener::bind(seed_addr).await.expect("bind data seed");
    let seed_app = app_b.clone();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept data seed");
        let _hello = read_wire_frame(&mut stream).await;
        let now_ms = current_time_ms().unwrap_or(0);
        let genesis_hash = {
            let hs = seed_app.handshake.read().await;
            hs.validation_ctx.expected_genesis_hash.clone()
        };
        let chain_tip_height = {
            let g = seed_app.inner.read().await;
            Some(g.chain.tip_h())
        };
        let remote =
            build_local_node_hello(&seed_app, genesis_hash, None, now_ms, chain_tip_height);
        let ack = serde_json::json!({
            "type": "hello_ack",
            "accepted": true,
            "node_hello": remote
        });
        let account_views = serde_json::json!({
            "type": "account_views",
            "rows": []
        });
        write_wire_payload(&mut stream, &serde_json::to_vec(&ack).unwrap()).await;
        write_wire_payload(&mut stream, &serde_json::to_vec(&account_views).unwrap()).await;
        let stop_at = current_time_ms().unwrap_or(0).saturating_add(1_200);
        loop {
            if current_time_ms().unwrap_or(0) >= stop_at {
                break;
            }
            let payload = match tokio::time::timeout(
                Duration::from_millis(250),
                try_read_wire_frame(&mut stream),
            )
            .await
            {
                Ok(Ok(v)) => v,
                Ok(Err(_)) => break,
                Err(_) => continue,
            };
            let frame: serde_json::Value = serde_json::from_slice(&payload).unwrap();
            if frame["type"] == "heartbeat" {
                write_wire_payload(&mut stream, &serde_json::to_vec(&account_views).unwrap()).await;
            }
        }
    });
    spawn_stateful_transport_loop(app_a.clone(), cfg_a);
    tokio::time::sleep(Duration::from_millis(650)).await;
    let hs = app_a.handshake.read().await;
    assert!(hs.trusted_peers.contains_key("node-b"));
    assert_eq!(
        hs.transport.snapshot.last_session_close_reason, None,
        "data-plane frames should satisfy heartbeat liveness: {:?}",
        hs.transport.snapshot.last_peer_error
    );
    assert!(
        hs.trusted_account_streams.contains_key("node-b"),
        "trusted AccountViews stream should remain fresh"
    );
}

/// Healthy trusted sessions emit healthy_session_skip reconnect decisions instead of needless redials.
#[tokio::test]
async fn xfer_health_skip_redial() {
    let app = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-a");
    let seed = reserve_loopback_addr();
    let now_ms = current_time_ms().unwrap_or(0);
    {
        let mut hs = app.handshake.write().await;
        hs.peers.insert(
            "node-b".to_string(),
            PeerRecord {
                node_id: "node-b".to_string(),
                domain_hi: 0x20,
                class: PeerClass::Foreign,
                last_seen_ms: now_ms,
                status: PeerStatus::Connected,
            },
        );
        hs.trusted_peers.insert(
            "node-b".to_string(),
            crate::transport::TrustedPeer {
                node_id: "node-b".to_string(),
                cluster_id: "cluster-b".to_string(),
                pubkey: [7u8; 32],
                domain_hi: 0x20,
                instance_id: None,
                cluster_attest_enabled: false,
                cluster_role: crate::handshake::ClusterRole::None,
            },
        );
        hs.transport
            .seed_peers
            .entry(seed.to_string())
            .or_default()
            .last_node_id = Some("node-b".to_string());
    }
    let mut cfg = TransportConfig::default();
    cfg.enabled = true;
    cfg.peer_seeds = vec![seed];
    cfg.retry_base_ms = 50;
    cfg.heartbeat_interval_ms = 50;
    cfg.heartbeat_timeout_ms = 500;
    spawn_stateful_transport_loop(app.clone(), cfg);
    let skipped = tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            let hs = app.handshake.read().await;
            if hs.transport.snapshot.last_reconnect_reason.as_deref()
                == Some("healthy_session_skip")
            {
                break;
            }
            drop(hs);
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
    })
    .await;
    assert!(skipped.is_ok(), "healthy session skip not observed");
    let hs = app.handshake.read().await;
    assert_eq!(hs.transport.snapshot.session_retrying_total, 0);
    assert_eq!(hs.transport.snapshot.last_peer_error, None);
    assert_eq!(
        hs.transport
            .snapshot
            .counters
            .reconnect_decision_by_reason
            .get("healthy_session_skip")
            .copied(),
        Some(1)
    );
}

/// Trusted AccountViews merges surface authoritative_home_balance lookups for foreign-domain accounts.
#[tokio::test]
async fn trust_foreign_view_ok() {
    let app = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-a");
    let (_, _, foreign_id) = user_sk_matching_domain_hi([0x42; 32], 0x20);
    {
        let mut g = app.inner.write().await;
        g.chain.st.accounts.insert(
            foreign_id,
            Account {
                balance_pwm: 7,
                initialized: true,
                ..Account::default()
            },
        );
        let merged = g.merge_peer_acct_views(
            vec![crate::state::PeerAccountViewWire {
                id: foreign_id,
                domain_hi: 0x20,
                balance_pwm: 777,
                initialized: true,
                nonce: 3,
                observed_at_ms: 123_456,
            }],
            "node-b",
            0x20,
        );
        assert_eq!(merged, 1);
    }
    {
        let mut hs = app.handshake.write().await;
        hs.peers.insert(
            "node-b".to_string(),
            PeerRecord {
                node_id: "node-b".to_string(),
                domain_hi: 0x20,
                class: PeerClass::Foreign,
                last_seen_ms: 123_456,
                status: PeerStatus::Connected,
            },
        );
        hs.trusted_peers.insert(
            "node-b".to_string(),
            crate::transport::TrustedPeer {
                node_id: "node-b".to_string(),
                cluster_id: "cluster-b".to_string(),
                pubkey: [7u8; 32],
                domain_hi: 0x20,
                instance_id: None,
                cluster_attest_enabled: false,
                cluster_role: crate::handshake::ClusterRole::None,
            },
        );
        hs.trusted_account_streams.insert(
            "node-b".to_string(),
            crate::transport::TrustedAccountStreamState {
                node_id: "node-b".to_string(),
                domain_hi: 0x20,
                last_update_ms: crate::current_time_ms().unwrap_or(0),
            },
        );
    }
    let svc = router_dev(app).into_service();
    let res = svc
        .oneshot(
            Request::get(format!("/v1/account/{}", hex::encode(foreign_id)))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let out: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(out["home_lookup_status"], "ok");
    assert_eq!(out["authoritative_home_balance"], "777");
    assert_eq!(out["authoritative_home_initialized"], true);
}

/// Trusted peers without wired AccountStreams still report unavailable home lookups (no spoofed balances).
#[tokio::test]
async fn trust_foreign_lookup_na() {
    let app = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-a");
    let (_, _, foreign_id) = user_sk_matching_domain_hi([0x52; 32], 0x20);
    {
        let mut g = app.inner.write().await;
        g.chain.st.accounts.insert(
            foreign_id,
            Account {
                balance_pwm: 13,
                initialized: true,
                ..Account::default()
            },
        );
    }
    {
        let mut hs = app.handshake.write().await;
        hs.peers.insert(
            "node-b".to_string(),
            PeerRecord {
                node_id: "node-b".to_string(),
                domain_hi: 0x20,
                class: PeerClass::Foreign,
                last_seen_ms: crate::current_time_ms().unwrap_or(0),
                status: PeerStatus::Connected,
            },
        );
        hs.trusted_peers.insert(
            "node-b".to_string(),
            crate::transport::TrustedPeer {
                node_id: "node-b".to_string(),
                cluster_id: "cluster-b".to_string(),
                pubkey: [7u8; 32],
                domain_hi: 0x20,
                instance_id: None,
                cluster_attest_enabled: false,
                cluster_role: crate::handshake::ClusterRole::None,
            },
        );
    }
    let svc = router_dev(app).into_service();
    let res = svc
        .oneshot(
            Request::get(format!("/v1/account/{}", hex::encode(foreign_id)))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let out: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(out["home_lookup_status"], "unavailable");
    assert!(out["authoritative_home_balance"].is_null());
}

/// Stateful transport records hello_rejected / handshake close reasons when inbound network mismatches.
#[tokio::test]
async fn xfer_state_mismatch_diag() {
    let app_a = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-a");
    let app_b = app_with_identity(DevLane::Lane1, "wrongnet", 0x20, "cluster-b", "node-b");
    let peer_a = reserve_loopback_addr();
    let peer_b = reserve_loopback_addr();
    let mut cfg_a = TransportConfig::default();
    cfg_a.enabled = true;
    cfg_a.peer_listen = peer_a;
    cfg_a.peer_seeds = vec![peer_b];
    cfg_a.retry_base_ms = 60;
    cfg_a.heartbeat_interval_ms = 80;
    cfg_a.heartbeat_timeout_ms = 250;
    let mut cfg_b = TransportConfig::default();
    cfg_b.enabled = true;
    cfg_b.peer_listen = peer_b;
    cfg_b.retry_base_ms = 60;
    spawn_peer_listener_loop(app_a.clone(), cfg_a.clone());
    spawn_peer_listener_loop(app_b.clone(), cfg_b);
    spawn_stateful_transport_loop(app_a.clone(), cfg_a);
    let got_err = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let hs = app_a.handshake.read().await;
            if hs
                .transport
                .snapshot
                .last_peer_error
                .as_deref()
                .unwrap_or_default()
                .contains("hello_rejected")
            {
                break;
            }
            drop(hs);
            tokio::time::sleep(Duration::from_millis(60)).await;
        }
    })
    .await;
    assert!(got_err.is_ok(), "mismatch diagnostic not observed");
    let hs = app_a.handshake.read().await;
    assert_eq!(
        hs.transport.snapshot.last_session_close_reason.as_deref(),
        Some("handshake_rejected")
    );
    assert_eq!(
        hs.transport
            .snapshot
            .counters
            .peer_close_by_reason
            .get("handshake_rejected")
            .copied(),
        Some(1)
    );
}

/// Remote hello_ack network skew yields remote_hello_rejected without trust or session counters.
#[tokio::test]
async fn xfer_remote_hi_mismatch() {
    let app_a = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-a");
    let app_b = app_with_identity(DevLane::Lane1, "wrongnet", 0x20, "cluster-b", "node-b");
    let peer_a = reserve_loopback_addr();
    let peer_b = reserve_loopback_addr();
    let mut cfg_a = TransportConfig::default();
    cfg_a.enabled = true;
    cfg_a.peer_listen = peer_a;
    cfg_a.peer_seeds = vec![peer_b];
    cfg_a.retry_base_ms = 60;
    cfg_a.heartbeat_interval_ms = 80;
    cfg_a.heartbeat_timeout_ms = 250;
    let mut cfg_b = TransportConfig::default();
    cfg_b.enabled = true;
    cfg_b.peer_listen = peer_b;
    cfg_b.retry_base_ms = 60;
    {
        let mut hs = app_b.handshake.write().await;
        hs.validation_ctx.expected_network_id = "testnet-qa".to_string();
    }
    spawn_peer_listener_loop(app_a.clone(), cfg_a.clone());
    spawn_peer_listener_loop(app_b.clone(), cfg_b);
    spawn_stateful_transport_loop(app_a.clone(), cfg_a);
    let got_err = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let hs = app_a.handshake.read().await;
            if hs
                .transport
                .snapshot
                .last_peer_error
                .as_deref()
                .unwrap_or_default()
                .contains("remote_hello_rejected")
            {
                break;
            }
            drop(hs);
            tokio::time::sleep(Duration::from_millis(60)).await;
        }
    })
    .await;
    assert!(
        got_err.is_ok(),
        "remote hello mismatch diagnostic not observed"
    );
    let hs = app_a.handshake.read().await;
    assert_eq!(hs.transport.snapshot.session_connected_total, 0);
    assert_eq!(hs.transport.snapshot.session_trusted_total, 0);
    assert!(!hs.trusted_peers.contains_key("node-b"));
}

/// Faulty seed acknowledgements drive wire/read diagnostics and prevent successful session trust.
#[tokio::test]
async fn xfer_wire_fail_diag() {
    let app_a = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-a");
    let peer_a = reserve_loopback_addr();
    let seed_addr = reserve_loopback_addr();
    let mut cfg_a = TransportConfig::default();
    cfg_a.enabled = true;
    cfg_a.peer_listen = peer_a;
    cfg_a.peer_seeds = vec![seed_addr];
    cfg_a.retry_base_ms = 50;
    cfg_a.handshake_timeout_ms = 300;
    let listener = TcpListener::bind(seed_addr)
        .await
        .expect("bind faulty seed");
    let app_for_seed = app_a.clone();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept outbound seed");
        let _hello = read_wire_frame(&mut stream).await;
        let now_ms = current_time_ms().unwrap_or(0);
        let genesis_hash = {
            let hs = app_for_seed.handshake.read().await;
            hs.validation_ctx.expected_genesis_hash.clone()
        };
        let chain_tip_height = {
            let g = app_for_seed.inner.read().await;
            Some(g.chain.tip_h())
        };
        let mut remote =
            build_local_node_hello(&app_for_seed, genesis_hash, None, now_ms, chain_tip_height);
        remote.network_id = "wrongnet".to_string();
        let ack = serde_json::json!({
            "type": "hello_ack",
            "accepted": true,
            "node_hello": remote
        });
        let payload = serde_json::to_vec(&ack).expect("encode hello_ack");
        write_wire_payload(&mut stream, &payload).await;
        drop(stream);
    });
    spawn_stateful_transport_loop(app_a.clone(), cfg_a);
    let got_err = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let hs = app_a.handshake.read().await;
            let err = hs
                .transport
                .snapshot
                .last_peer_error
                .as_deref()
                .unwrap_or_default()
                .to_string();
            if err.contains("remote_hello_rejected") || err.contains("wire_hello_read_failed") {
                break;
            }
            drop(hs);
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await;
    assert!(got_err.is_ok(), "wire failure diagnostic not observed");
    let hs = app_a.handshake.read().await;
    assert!(hs.transport.snapshot.session_retrying_total > 0);
    assert_eq!(hs.transport.snapshot.session_connected_total, 0);
    assert_eq!(hs.transport.snapshot.session_trusted_total, 0);
}

/// EOF after hello_ack records a peer-close reason bucket without losing connected telemetry.
#[tokio::test]
async fn xfer_eof_close_reason() {
    let app_a = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-a");
    let app_b = app_with_identity(DevLane::Lane1, "testnet-qa", 0x20, "cluster-b", "node-b");
    let peer_a = reserve_loopback_addr();
    let seed_addr = reserve_loopback_addr();
    let mut cfg_a = TransportConfig::default();
    cfg_a.enabled = true;
    cfg_a.peer_listen = peer_a;
    cfg_a.peer_seeds = vec![seed_addr];
    cfg_a.retry_base_ms = 50;
    cfg_a.handshake_timeout_ms = 300;
    cfg_a.heartbeat_interval_ms = 50;
    cfg_a.heartbeat_timeout_ms = 100;
    let listener = TcpListener::bind(seed_addr).await.expect("bind eof seed");
    let seed_app = app_b.clone();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept outbound seed");
        let _hello = read_wire_frame(&mut stream).await;
        let now_ms = current_time_ms().unwrap_or(0);
        let genesis_hash = {
            let hs = seed_app.handshake.read().await;
            hs.validation_ctx.expected_genesis_hash.clone()
        };
        let chain_tip_height = {
            let g = seed_app.inner.read().await;
            Some(g.chain.tip_h())
        };
        let remote =
            build_local_node_hello(&seed_app, genesis_hash, None, now_ms, chain_tip_height);
        let ack = serde_json::json!({
            "type": "hello_ack",
            "accepted": true,
            "node_hello": remote
        });
        let payload = serde_json::to_vec(&ack).expect("encode hello_ack");
        write_wire_payload(&mut stream, &payload).await;
        drop(stream);
    });
    spawn_stateful_transport_loop(app_a.clone(), cfg_a);
    let got_close = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let hs = app_a.handshake.read().await;
            if hs.transport.snapshot.last_session_close_reason.is_some() {
                break;
            }
            drop(hs);
            tokio::time::sleep(Duration::from_millis(40)).await;
        }
    })
    .await;
    assert!(got_close.is_ok(), "close reason not observed");
    let hs = app_a.handshake.read().await;
    let reason = hs
        .transport
        .snapshot
        .last_session_close_reason
        .as_deref()
        .expect("close reason");
    assert!(
        matches!(reason, "eof" | "wire_timeout" | "protocol_error"),
        "unexpected close reason: {reason}"
    );
    assert!(hs.transport.snapshot.session_connected_total >= 1);
    assert_eq!(
        hs.transport
            .snapshot
            .counters
            .peer_close_by_reason
            .get(reason)
            .copied(),
        Some(1)
    );
}

/// Dialing peers with mismatched genesis_hash increments genesis_guard totals and dial failure counters.
#[tokio::test]
async fn real_xfer_gen_mismatch() {
    let app = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-a");
    let seed_app = app_with_identity(DevLane::Lane1, "testnet-qa", 0x20, "cluster-b", "node-b");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let cfg = TransportConfig {
        enabled: true,
        peer_seeds: vec![addr],
        connect_timeout_ms: 1_000,
        handshake_timeout_ms: 1_000,
        retry_base_ms: 200,
        retry_max_ms: 2_000,
        ..TransportConfig::default()
    };
    {
        let mut tc = seed_app.transport_config.write().await;
        *tc = cfg.clone();
    }
    {
        let mut hs = seed_app.handshake.write().await;
        hs.validation_ctx.expected_genesis_hash = Some("other-genesis".to_string());
    }
    tokio::spawn(async move {
        axum::serve(listener, router_dev(seed_app)).await.unwrap();
    });
    run_real_transport_tick(&app, &cfg, current_time_ms().unwrap()).await;
    let hs = app.handshake.read().await;
    assert_eq!(hs.genesis_guard.mismatch_total, 1);
    assert_eq!(
        hs.genesis_guard
            .last_mismatch
            .as_ref()
            .and_then(|x| x.received_hash.as_deref()),
        Some("other-genesis")
    );
    assert!(hs
        .transport
        .snapshot
        .last_peer_error
        .as_deref()
        .unwrap_or("")
        .contains("genesis_mismatch"));
    assert_eq!(
        hs.transport
            .snapshot
            .counters
            .dial_attempt_class_result
            .get("unknown:retryable_fail")
            .copied(),
        Some(1)
    );
}

/// Connect-timeout failures enqueue seed backoff timestamps and increment backoff_skip_total appropriately.
#[tokio::test]
async fn real_xfer_backoff_conn() {
    let app = app_from_devnet(DevLane::Lane0);
    let cfg = TransportConfig {
        enabled: true,
        peer_seeds: vec![SocketAddr::from(([127, 0, 0, 1], 1))],
        connect_timeout_ms: 50,
        handshake_timeout_ms: 50,
        retry_base_ms: 400,
        retry_max_ms: 2_000,
        ..TransportConfig::default()
    };
    run_real_transport_tick(&app, &cfg, 1_000).await;
    run_real_transport_tick(&app, &cfg, 1_100).await;
    let hs = app.handshake.read().await;
    let key = cfg.peer_seeds[0].to_string();
    let st = hs.transport.seed_peers.get(&key).expect("seed state");
    assert_eq!(st.attempts, 1);
    assert!(st.next_due_ms >= 1_400);
    assert!(st.next_due_ms <= 1_500);
    assert_eq!(hs.transport.snapshot.counters.backoff_skip_total, 1);
    assert!(hs
        .transport
        .snapshot
        .last_peer_error
        .as_deref()
        .unwrap_or_default()
        .contains("connect"));
}

/// Non-JSON /v1/status responses surface status_decode_failed in transport.last_peer_error.
#[tokio::test]
async fn real_xfer_status_decode_bad() {
    let app = app_with_identity(DevLane::Lane0, "testnet-qa", 0x10, "cluster-a", "node-a");
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let router = Router::new().route("/v1/status", axum::routing::get(|| async { "not-json" }));
        axum::serve(listener, router).await.unwrap();
    });
    let cfg = TransportConfig {
        enabled: true,
        peer_seeds: vec![addr],
        connect_timeout_ms: 1_000,
        handshake_timeout_ms: 1_000,
        retry_base_ms: 200,
        retry_max_ms: 2_000,
        ..TransportConfig::default()
    };
    run_real_transport_tick(&app, &cfg, current_time_ms().unwrap()).await;
    let hs = app.handshake.read().await;
    assert!(hs
        .transport
        .snapshot
        .last_peer_error
        .as_deref()
        .unwrap_or_default()
        .contains("status_decode_failed"));
}

/// tick_attempt_budget rotates through deterministic seed peers while tracking churn counters.
#[tokio::test]
async fn real_xfer_seed_rot_budget() {
    let app = app_from_devnet(DevLane::Lane0);
    let seeds = vec![
        SocketAddr::from(([127, 0, 0, 1], 1)),
        SocketAddr::from(([127, 0, 0, 1], 2)),
        SocketAddr::from(([127, 0, 0, 1], 3)),
    ];
    {
        let mut hs = app.handshake.write().await;
        hs.transport.snapshot.tick_attempt_budget = 2;
    }
    let cfg = TransportConfig {
        enabled: true,
        peer_seeds: seeds.clone(),
        connect_timeout_ms: 20,
        handshake_timeout_ms: 20,
        retry_base_ms: 1,
        retry_max_ms: 1,
        ..TransportConfig::default()
    };
    run_real_transport_tick(&app, &cfg, 1_000).await;
    run_real_transport_tick(&app, &cfg, 2_000).await;
    let hs = app.handshake.read().await;
    let s0 = hs
        .transport
        .seed_peers
        .get(&seeds[0].to_string())
        .expect("s0 state");
    let s1 = hs
        .transport
        .seed_peers
        .get(&seeds[1].to_string())
        .expect("s1 state");
    let s2 = hs
        .transport
        .seed_peers
        .get(&seeds[2].to_string())
        .expect("s2 state");
    assert_eq!(s0.attempts, 1);
    assert_eq!(s1.attempts, 2);
    assert_eq!(s2.attempts, 1);
    assert_eq!(hs.transport.snapshot.last_tick_attempts, 2);
    assert_eq!(hs.churn.seed_rotation_total, 2);
}

/// Flaky reconnect paths hit Retrying/Disconnected states with bounded_retry_cooldowns_total accounting.
#[tokio::test]
async fn real_xfer_reconn_bounded() {
    let app = app_from_devnet(DevLane::Lane0);
    let addr = SocketAddr::from(([127, 0, 0, 1], 1));
    {
        let mut hs = app.handshake.write().await;
        hs.peers.insert(
            "seed-peer-flaky".to_string(),
            PeerRecord {
                node_id: "seed-peer-flaky".to_string(),
                domain_hi: 0x10,
                class: PeerClass::Native,
                last_seen_ms: 900,
                status: PeerStatus::Connected,
            },
        );
        let st = hs.transport.seed_peers.entry(addr.to_string()).or_default();
        st.last_node_id = Some("seed-peer-flaky".to_string());
    }
    let cfg = TransportConfig {
        enabled: true,
        peer_seeds: vec![addr],
        connect_timeout_ms: 50,
        handshake_timeout_ms: 50,
        retry_base_ms: 1,
        retry_max_ms: 1,
        ..TransportConfig::default()
    };
    for i in 0..=8u64 {
        run_real_transport_tick(&app, &cfg, 1_000 + i * 100).await;
    }
    let hs = app.handshake.read().await;
    let rec = hs.peers.get("seed-peer-flaky").expect("peer record");
    assert!(matches!(
        rec.status,
        PeerStatus::Retrying | PeerStatus::Disconnected
    ));
    assert!(hs.churn.retrying_total >= 1);
    assert!(hs.churn.disconnected_total >= 1);
    assert!(hs.churn.bounded_retry_cooldowns_total >= 1);
}

/// Soak rollup counters respect soak_counter_cap plus periodic soak_health_snapshot_total pacing.
#[tokio::test]
async fn real_xfer_soak_bounds() {
    let app = app_from_devnet(DevLane::Lane0);
    let cfg = TransportConfig {
        enabled: true,
        peer_seeds: vec![SocketAddr::from(([127, 0, 0, 1], 1))],
        connect_timeout_ms: 5,
        handshake_timeout_ms: 5,
        heartbeat_interval_ms: 500,
        heartbeat_timeout_ms: 1_500,
        retry_base_ms: 1,
        retry_max_ms: 1,
        soak_counter_cap: 20,
        soak_health_interval_ticks: 5,
        reconnect_runaway_streak_limit: 1_000,
        reconnect_runaway_cooldown_ms: 5,
        ..TransportConfig::default()
    };
    for i in 0..30u64 {
        run_real_transport_tick(&app, &cfg, 1_000 + i).await;
    }
    let hs = app.handshake.read().await;
    assert_eq!(hs.transport.snapshot.soak_ticks_capped, 20);
    assert_eq!(hs.transport.snapshot.soak_health_snapshot_total, 6);
    assert_eq!(hs.transport.snapshot.soak_health_last_tick, 30);
    assert!(hs.churn.unstable_tick_total > 0);
    assert!(hs.churn.unstable_tick_total <= 20);
    assert!(hs.churn.reconnect_streak_max >= 1);
    assert!(hs.churn.reconnect_streak_max <= 20);
}

/// Runaway reconnect guard pauses dial attempts during cooldown and resumes afterwards.
#[tokio::test]
async fn real_xfer_runaway_guard() {
    let app = app_from_devnet(DevLane::Lane0);
    let cfg = TransportConfig {
        enabled: true,
        peer_seeds: vec![SocketAddr::from(([127, 0, 0, 1], 1))],
        connect_timeout_ms: 5,
        handshake_timeout_ms: 5,
        retry_base_ms: 1,
        retry_max_ms: 1,
        reconnect_runaway_streak_limit: 3,
        reconnect_runaway_cooldown_ms: 200,
        ..TransportConfig::default()
    };
    run_real_transport_tick(&app, &cfg, 1_000).await;
    run_real_transport_tick(&app, &cfg, 1_050).await;
    run_real_transport_tick(&app, &cfg, 1_100).await;
    {
        let hs = app.handshake.read().await;
        assert!(hs.transport.snapshot.reconnect_runaway_guard_active);
        assert_eq!(hs.transport.snapshot.reconnect_runaway_stop_total, 1);
    }
    let before = {
        let hs = app.handshake.read().await;
        hs.transport
            .snapshot
            .counters
            .dial_attempt_class_result
            .get("unknown:retryable_fail")
            .copied()
            .unwrap_or(0)
    };
    run_real_transport_tick(&app, &cfg, 1_150).await;
    let after_skip = {
        let hs = app.handshake.read().await;
        assert!(hs.transport.snapshot.reconnect_runaway_guard_active);
        assert_eq!(hs.transport.snapshot.last_tick_attempts, 0);
        hs.transport
            .snapshot
            .counters
            .dial_attempt_class_result
            .get("unknown:retryable_fail")
            .copied()
            .unwrap_or(0)
    };
    assert_eq!(before, after_skip);
    run_real_transport_tick(&app, &cfg, 1_320).await;
    let hs = app.handshake.read().await;
    assert!(!hs.transport.snapshot.reconnect_runaway_guard_active);
    let after_resume = hs
        .transport
        .snapshot
        .counters
        .dial_attempt_class_result
        .get("unknown:retryable_fail")
        .copied()
        .unwrap_or(0);
    assert!(after_resume > after_skip);
}
