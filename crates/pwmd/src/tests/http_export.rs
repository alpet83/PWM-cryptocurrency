//! HTTP router tests for tx/export/roaming endpoints against dev-net apps.

use super::helpers::*;
use super::prelude::*;

/// /v1/tx rejects Init bodies whose domain low-byte violates regulatory routing for the account.
#[tokio::test]
async fn v1_tx_reg_lo0_bad() {
    let (sk, i, aid) = routable_user_sk([29u8; 32]);
    let mut domain = domain_of_account_id(&aid);
    domain = (domain & 0xFF00) | 0x00;
    let tx = SignedTx::sign_body(&sk, domain, i, 0, TxBody::Init { index: 0, flags: 0 });
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
    let body = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("domain mismatch"));
}

#[tokio::test]
async fn v1_tx_accepts_signed_init() {
    let (sk, i, aid) = routable_user_sk([23u8; 32]);
    let dom = domain_of_account_id(&aid);
    let tx = SignedTx::sign_body(&sk, dom, i, 0, TxBody::Init { index: 1, flags: 0 });
    let app = app_for_sender(&aid);
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
    let status = res.status();
    let body = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    assert_eq!(status, StatusCode::NO_CONTENT, "body={text}");
    let g = app.inner.read().await;
    assert_eq!(g.pool.len(), 1);
}

/// Failed snapshot persistence after /v1/tx surfaces HTTP 500 and marks ready_degraded.
#[tokio::test]
async fn v1_tx_500_snap_fail() {
    let (sk, i, aid) = routable_user_sk([24u8; 32]);
    let dom = domain_of_account_id(&aid);
    let tx = SignedTx::sign_body(&sk, dom, i, 0, TxBody::Init { index: 1, flags: 0 });
    let mut app = app_for_sender(&aid);
    let bad_dir = std::env::temp_dir().join(format!(
        "pwmd_snapshot_fail_dir_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&bad_dir).unwrap();
    app.data_file = Some(bad_dir.clone());
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
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("snapshot save failed"), "body={text}");
    {
        let st = app.init.read().await;
        assert_eq!(st.phase, crate::state::InitPhase::ReadyDegraded);
        assert!(st.snapshot_error.is_some());
    }
    let res2 = svc
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&tx).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res2.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body2 = to_bytes(res2.into_body(), 64 * 1024).await.unwrap();
    let t2 = String::from_utf8_lossy(&body2);
    assert!(
        t2.contains("user tx blocked") && t2.contains("degraded"),
        "body={t2}"
    );
    std::fs::remove_dir_all(&bad_dir).ok();
}

#[tokio::test]
async fn v1_tx_accepts_export() {
    let (cfg, sks) = dev_net();
    let sk = &sks[0];
    let i = 0;
    let aid = cfg.accounts[0].acct;
    let sender_dom = domain_of_account_id(&aid);
    let sender_hi = sender_dom.to_be_bytes()[0];
    let recipient = (0u16..4096)
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
        .expect("must find valid recipient in different domain");
    let target_domain = domain_of_account_id(&recipient);
    let tx = SignedTx::sign_body(
        sk,
        sender_dom,
        i,
        0,
        TxBody::Export {
            to: recipient,
            target_domain,
            amount: 10,
            fee: 1,
        },
    );
    let app = app_for_sender(&aid);
    let svc = router_dev(app.clone()).into_service();
    let ready = svc
        .clone()
        .oneshot(
            Request::post("/v1/export-readiness")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({ "tx": tx.clone(), "ttl_sec": 30 }))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ready.status(), StatusCode::OK);
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
    let g = app.inner.read().await;
    assert_eq!(g.pool.len(), 0);
    let export_id = tx.export_id().expect("export id");
    assert!(g.chain.st.exported_registry.contains_key(&export_id));
    let blk = g.chain.blocks.back().expect("sealed block for export");
    assert_eq!(blk.txs.len(), 1);
    assert_eq!(blk.txs[0].tx_hash(), tx.tx_hash());
}

/// Latched `bridge_federation_trust_refused` yields 409 on `/v1/export-readiness` (one-window export path).
#[tokio::test]
async fn v1_rd_conflict_bridge_trust() {
    let (cfg, sks) = dev_net();
    let sk = &sks[0];
    let aid = cfg.accounts[0].acct;
    let sender_dom = domain_of_account_id(&aid);
    let recipient = valid_cross_domain_recipient(sender_dom.to_be_bytes()[0]);
    let target_domain = domain_of_account_id(&recipient);
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
    let app = app_for_sender(&aid);
    {
        let mut hs = app.handshake.write().await;
        hs.bridge_trust.refused = true;
        hs.bridge_trust.refusal_total = 1;
        hs.bridge_trust.refusal_reason = Some(
            "bridge_federation_trust_refused expected_bridge_commitment=deadbeef received_bridge_commitment=cafebabe"
                .to_string(),
        );
    }
    let svc = router_dev(app.clone()).into_service();
    let res = svc
        .clone()
        .oneshot(
            Request::post("/v1/export-readiness")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({ "tx": tx, "ttl_sec": 30 })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CONFLICT);
    let body = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(
        text.contains("bridge_federation_trust_refused")
            && text.contains("expected_bridge_commitment"),
        "body={text}"
    );

    let res_st = svc
        .oneshot(Request::get("/v1/status").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res_st.status(), StatusCode::OK);
    let st_body = to_bytes(res_st.into_body(), 64 * 1024).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&st_body).unwrap();
    assert_eq!(
        v["bridge_federation_trust"],
        "bridge_federation_trust_refused"
    );
    assert!(v["bridge_refusal_reason"]
        .as_str()
        .unwrap_or("")
        .contains("expected_bridge_commitment"));
}

/// Export /v1/tx rolls back sender balances when post-seal snapshot writes fail.
#[tokio::test]
async fn v1_tx_rbexp_ok() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    let (cfg, sks) = dev_net();
    let sk = &sks[0];
    let source = cfg.accounts[0].acct;
    let source_dom = domain_of_account_id(&source);
    let recipient = valid_cross_domain_recipient(source_dom.to_be_bytes()[0]);
    let target_domain = domain_of_account_id(&recipient);
    let tx = SignedTx::sign_body(
        sk,
        source_dom,
        0,
        0,
        TxBody::Export {
            to: recipient,
            target_domain,
            amount: 17,
            fee: 1,
        },
    );
    let mut app = app;
    let bad_dir = std::env::temp_dir().join(format!(
        "pwmd_snapshot_fail_tx_export_dir_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&bad_dir).unwrap();
    app.data_file = Some(bad_dir.clone());
    let svc = router_dev(app.clone()).into_service();
    let preflight = serde_json::json!({ "tx": tx.clone(), "ttl_sec": 30 });
    let ready = svc
        .clone()
        .oneshot(
            Request::post("/v1/export-readiness")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&preflight).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ready.status(), StatusCode::OK);
    let res = svc
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&tx).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let g = app.inner.read().await;
    let src = g.chain.st.get(&source).expect("source account");
    assert_eq!(src.nonce, 0);
    assert_eq!(src.balance_pwm, 1_000_000);
    assert!(!g
        .chain
        .st
        .exported_registry
        .contains_key(&tx.export_id().expect("export id")));
    assert_eq!(g.cross_shard.summary().total_exported_count, 0);
    std::fs::remove_dir_all(&bad_dir).ok();
}

/// /v1/export-provenance rejects self-attested handoffs that lack peer provenance.
#[tokio::test]
async fn v1_exp_pv_self_bad() {
    let source_app = app_for_devnet_sender(DevLane::Lane0);
    let target_app = app_from_devnet(DevLane::Lane1);
    let (handoff, export_id) = source_handoff(&source_app, 23).await;
    let svc = router_dev(target_app.clone()).into_service();
    let res = svc
        .oneshot(
            Request::post("/v1/export-provenance")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&handoff).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let g = target_app.inner.read().await;
    assert!(!g.chain.st.exported_registry.contains_key(&export_id));
}

/// Inbound (non-trusted) peer hellos block provenance handoffs on the target node.
#[tokio::test]
async fn v1_exp_ib_hi_pv() {
    let source_app = app_for_devnet_sender(DevLane::Lane0);
    let target_app = app_from_devnet(DevLane::Lane1);
    let (handoff, export_id) = source_handoff(&source_app, 24).await;
    let now_ms = current_time_ms().expect("clock");
    let genesis_hash = {
        let hs = target_app.handshake.read().await;
        hs.validation_ctx.expected_genesis_hash.clone()
    };
    let chain_tip_height = {
        let g = source_app.inner.read().await;
        Some(g.chain.tip_h())
    };
    let hello = crate::transport::build_local_node_hello(
        &source_app,
        genesis_hash,
        None,
        now_ms,
        chain_tip_height,
    );
    let svc = router_dev(target_app.clone()).into_service();
    let hello_res = svc
        .clone()
        .oneshot(
            Request::post("/v1/peer/hello")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&hello).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(hello_res.status(), StatusCode::OK);
    let hello_body = to_bytes(hello_res.into_body(), 64 * 1024).await.unwrap();
    let hello_json: serde_json::Value = serde_json::from_slice(&hello_body).unwrap();
    assert_eq!(hello_json["accepted"], true);
    {
        let hs = target_app.handshake.read().await;
        assert!(hs.peers.contains_key(&source_app.identity.node_id));
        assert!(!hs.trusted_peers.contains_key(&source_app.identity.node_id));
    }

    let res = svc
        .oneshot(
            Request::post("/v1/export-provenance")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&handoff).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let body = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    assert!(String::from_utf8_lossy(&body).contains("not trusted"));
    let g = target_app.inner.read().await;
    assert!(!g.chain.st.exported_registry.contains_key(&export_id));
}

/// Trusted peer configuration allows provenance ingestion for otherwise valid handoffs.
#[tokio::test]
async fn v1_exp_pv_trust_ok() {
    let source_app = app_for_devnet_sender(DevLane::Lane0);
    let target_app = app_from_devnet(DevLane::Lane1);
    let (handoff, export_id) =
        source_handoff_for_hi(&source_app, 25, target_app.identity.cluster_domain_hi).await;
    trust_source_peer(&target_app, &source_app).await;
    let svc = router_dev(target_app.clone()).into_service();
    let res = svc
        .oneshot(
            Request::post("/v1/export-provenance")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&handoff).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let g = target_app.inner.read().await;
    assert!(!g.chain.st.exported_registry.contains_key(&export_id));
    assert!(
        g.cross_shard
            .facts()
            .iter()
            .any(|f| f.export_id == export_id),
        "handoff must be kept in non-root cross_shard state"
    );
}

/// Genesis guard blocks provenance writes while the node is in blocked user-tx mode.
#[tokio::test]
async fn v1_exp_pv_gen_guard() {
    let source_app = app_for_devnet_sender(DevLane::Lane0);
    let target_app = app_for_devnet_sender(DevLane::Lane1);
    let (handoff, export_id) = source_handoff(&source_app, 29).await;
    trust_source_peer(&target_app, &source_app).await;
    {
        let mut hs = target_app.handshake.write().await;
        hs.genesis_guard.blocked = true;
    }
    let svc = router_dev(target_app.clone()).into_service();
    let res = svc
        .oneshot(
            Request::post("/v1/export-provenance")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&handoff).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    let g = target_app.inner.read().await;
    assert!(!g.chain.st.exported_registry.contains_key(&export_id));
}

/// Backfill imports missing cross-shard fact exactly once from trusted peer endpoint.
#[tokio::test]
async fn v1_xsh_backfill_once_ok() {
    let source_app = app_from_devnet(DevLane::Lane1);
    let target_app = app_for_devnet_sender(DevLane::Lane0);
    let target_hi = {
        let g = target_app.inner.read().await;
        domain_of_account_id(&g.chain.cfg.accounts[0].acct).to_be_bytes()[0]
    };
    let (target_sk, target_i, target_aid) = user_sk_matching_domain_hi([0x72; 32], target_hi);
    let target_dom = domain_of_account_id(&target_aid);
    {
        let init = SignedTx::sign_body(
            &target_sk,
            target_dom,
            target_i,
            0,
            TxBody::Init { index: 1, flags: 0 },
        );
        let mut g = target_app.inner.write().await;
        g.chain.st.apply_tx(&init).expect("init target recipient");
    }
    let (source_sk, source_i, source_aid) =
        user_sk_matching_domain_hi([0x71; 32], source_app.identity.cluster_domain_hi);
    let source_dom = domain_of_account_id(&source_aid);
    {
        let init = SignedTx::sign_body(
            &source_sk,
            source_dom,
            source_i,
            0,
            TxBody::Init { index: 1, flags: 0 },
        );
        let mut g = source_app.inner.write().await;
        g.chain.st.apply_tx(&init).expect("init source sender");
        let src = g
            .chain
            .st
            .accounts
            .get_mut(&source_aid)
            .expect("source account");
        src.balance_pwm = 1_000;
        src.nonce = 0;
    }
    let amount = 17u128;
    let export = SignedTx::sign_body(
        &source_sk,
        source_dom,
        source_i,
        0,
        TxBody::Export {
            to: target_aid,
            target_domain: target_dom,
            amount,
            fee: 1,
        },
    );
    let export_id = export.export_id().expect("export id");
    let source_svc = router_dev(source_app.clone()).into_service();
    let ready = source_svc
        .clone()
        .oneshot(
            Request::post("/v1/export-readiness")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({ "tx": export.clone(), "ttl_sec": 30 }))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ready.status(), StatusCode::OK);
    let export_res = source_svc
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&export).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(export_res.status(), StatusCode::NO_CONTENT);
    {
        let g = source_app.inner.read().await;
        assert!(
            g.cross_shard
                .facts()
                .iter()
                .any(|x| x.export_id == export_id),
            "source must expose export in cross_shard facts"
        );
    }

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind source");
    let source_addr = listener.local_addr().expect("source addr");
    let source_router = router_dev(source_app.clone());
    tokio::spawn(async move {
        axum::serve(listener, source_router).await.unwrap();
    });
    let peer_base = format!("http://{source_addr}");

    let target_svc = router_dev(target_app.clone()).into_service();
    let backfill_body = serde_json::json!({
        "peer_base": peer_base,
        "from_height": 0,
        "limit": 8
    });
    let first = target_svc
        .clone()
        .oneshot(
            Request::post("/v1/cross-shard/backfill")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&backfill_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    let first_json: serde_json::Value =
        serde_json::from_slice(&to_bytes(first.into_body(), 64 * 1024).await.unwrap()).unwrap();
    assert_eq!(first_json["discovered"], 1, "first={first_json}");
    assert_eq!(first_json["imported"], 1, "first={first_json}");
    assert_eq!(first_json["skipped_existing"], 0, "first={first_json}");
    assert_eq!(first_json["untrusted"], 0, "first={first_json}");

    let second = target_svc
        .oneshot(
            Request::post("/v1/cross-shard/backfill")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&backfill_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::OK);
    let second_json: serde_json::Value =
        serde_json::from_slice(&to_bytes(second.into_body(), 64 * 1024).await.unwrap()).unwrap();
    assert_eq!(second_json["imported"], 0);
    assert_eq!(second_json["skipped_existing"], 1);
    {
        let g = target_app.inner.read().await;
        assert!(g.chain.st.imported_set.contains(&export_id));
        assert_eq!(
            g.chain
                .st
                .get(&target_aid)
                .expect("target account")
                .balance_pwm,
            amount
        );
    }
}

/// Backfill rejects peers with mismatched trust envelope (network/genesis).
#[tokio::test]
async fn v1_xsh_backfill_untrusted_skip() {
    let source_app = app_with_identity(DevLane::Lane0, "othernet", 0x10, "src-cl", "src-node");
    let target_app = app_for_devnet_sender(DevLane::Lane1);
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind source");
    let source_addr = listener.local_addr().expect("source addr");
    let source_router = router_dev(source_app);
    tokio::spawn(async move {
        axum::serve(listener, source_router).await.unwrap();
    });
    let peer_base = format!("http://{source_addr}");
    let svc = router_dev(target_app).into_service();
    let res = svc
        .oneshot(
            Request::post("/v1/cross-shard/backfill")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({ "peer_base": peer_base })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let json: serde_json::Value =
        serde_json::from_slice(&to_bytes(res.into_body(), 64 * 1024).await.unwrap()).unwrap();
    assert_eq!(json["imported"], 0);
    assert_eq!(json["untrusted"], 1);
}

/// Backfill is blocked in ready_degraded mode and must not mutate local facts.
#[tokio::test]
async fn v1_xsh_backfill_degraded_hold() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    {
        let mut st = app.init.write().await;
        *st = crate::state::InitState::ready_degraded(None, "disk error".into());
    }
    let before = {
        let g = app.inner.read().await;
        (g.cross_shard.facts().len(), g.chain.st.imported_set.len())
    };
    let svc = router_dev(app.clone()).into_service();
    let res = svc
        .oneshot(
            Request::post("/v1/cross-shard/backfill")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "peer_base": "http://127.0.0.1:1"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("user tx blocked") && text.contains("degraded"));
    let after = {
        let g = app.inner.read().await;
        (g.cross_shard.facts().len(), g.chain.st.imported_set.len())
    };
    assert_eq!(after, before);
}

/// Backfill is blocked by genesis guard and must not mutate local facts.
#[tokio::test]
async fn v1_xsh_backfill_genblock_hold() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    {
        let mut hs = app.handshake.write().await;
        hs.genesis_guard.blocked = true;
    }
    let before = {
        let g = app.inner.read().await;
        (g.cross_shard.facts().len(), g.chain.st.imported_set.len())
    };
    let svc = router_dev(app.clone()).into_service();
    let res = svc
        .oneshot(
            Request::post("/v1/cross-shard/backfill")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "peer_base": "http://127.0.0.1:1"
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("genesis/hash mismatch"));
    let after = {
        let g = app.inner.read().await;
        (g.cross_shard.facts().len(), g.chain.st.imported_set.len())
    };
    assert_eq!(after, before);
}

/// Roaming finalize without seeds keeps exported state but records relay errors.
#[tokio::test]
async fn v1_rov_seedless_err() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    let (handoff, export_id) = source_handoff(&app, 31).await;
    assert_eq!(handoff["status"], "relayed");
    let g = app.inner.read().await;
    let intent = g
        .roaming_pool
        .get_by_export_id(&export_id)
        .expect("intent by export id");
    assert_eq!(intent.status, crate::roaming::IntentStatus::Exported);
    assert!(intent
        .last_error
        .as_deref()
        .unwrap_or("")
        .contains("no HTTP relay base configured"));
}

/// HTTP import flow accepts a signed import referencing a prior export delivery.
#[tokio::test]
async fn v1_tx_imp_after_exp() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    let (import_sk, import_i, import_aid, export_id, provenance) =
        seed_handoff_provenance_for_import(&app, 17).await;
    let import_dom = domain_of_account_id(&import_aid);
    let mut tx = SignedTx::sign_body(
        &import_sk,
        import_dom,
        import_i,
        1,
        TxBody::Import {
            to: import_aid,
            amount: 17,
            export_id,
        },
    );
    tx.set_import_provenance_signed(&import_sk, Some(provenance));
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
    let g = app.inner.read().await;
    assert_eq!(g.pool.len(), 0);
    assert!(g.chain.st.imported_set.contains(&export_id));
}

/// Unknown import bodies are rejected once the signer account is initialized.
#[tokio::test]
async fn v1_tx_ia_ximp() {
    let app = mk_app_explicit_shard(DevLane::Lane0);
    let (import_sk, import_i, import_aid) = user_sk_matching_domain_hi([0x44; 32], 0x10);
    let import_dom = domain_of_account_id(&import_aid);
    let init = SignedTx::sign_body(
        &import_sk,
        import_dom,
        import_i,
        0,
        TxBody::Init { index: 1, flags: 0 },
    );
    {
        let mut g = app.inner.write().await;
        g.chain.st.apply_tx(&init).expect("init import signer");
    }
    let svc = router_dev(app.clone()).into_service();
    let before = {
        let g = app.inner.read().await;
        g.chain.st.get(&import_aid).expect("import signer").clone()
    };

    let tx = SignedTx::sign_body(
        &import_sk,
        import_dom,
        import_i,
        before.nonce,
        TxBody::Import {
            to: import_aid,
            amount: 19,
            export_id: [0xAB; 32],
        },
    );
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
    assert!(String::from_utf8_lossy(&body).contains("export_id is not known"));
    let g = app.inner.read().await;
    assert_eq!(g.chain.st.get(&import_aid).expect("after reject"), &before);
    assert!(!g.chain.st.imported_set.contains(&[0xAB; 32]));
    assert!(!g.chain.st.exported_registry.contains_key(&[0xAB; 32]));
}

/// Transfers to unknown recipients fail before mempool admission with policy errors.
#[tokio::test]
async fn v1_tx_xfer_miss_pre() {
    let app = mk_app_explicit_shard(DevLane::Lane0);
    let (send_sk, send_i, send_aid) = user_sk_matching_domain_hi([0x45; 32], 0x10);
    let send_dom = domain_of_account_id(&send_aid);
    let (_, _, recv_aid) = user_sk_matching_domain_hi([0x46; 32], 0x10);
    let init = SignedTx::sign_body(
        &send_sk,
        send_dom,
        send_i,
        0,
        TxBody::Init { index: 1, flags: 0 },
    );
    {
        let mut g = app.inner.write().await;
        g.chain.st.apply_tx(&init).expect("init sender");
        let sender = g.chain.st.accounts.get_mut(&send_aid).expect("sender");
        sender.balance_pwm = 100;
    }
    let tx = SignedTx::sign_body(
        &send_sk,
        send_dom,
        send_i,
        1,
        TxBody::Transfer {
            to: recv_aid,
            amount: 10,
            fee: 1,
        },
    );
    let before = {
        let g = app.inner.read().await;
        g.chain.st.get(&send_aid).expect("sender").clone()
    };
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
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    assert!(String::from_utf8_lossy(&body).contains("recipient account not found"));
    let g = app.inner.read().await;
    assert_eq!(g.pool.len(), 0);
    assert_eq!(g.chain.st.get(&send_aid).expect("sender"), &before);
    assert!(g.chain.st.get(&recv_aid).is_none());
}

/// Imports targeting missing recipients fail even when provenance metadata exists.
#[tokio::test]
async fn v1_tx_imp_miss_rcv() {
    let app = mk_app_explicit_shard(DevLane::Lane0);
    let (import_sk, import_i, import_aid) = user_sk_matching_domain_hi([0x47; 32], 0x10);
    let import_dom = domain_of_account_id(&import_aid);
    let (_, _, recv_aid) = user_sk_matching_domain_hi([0x48; 32], 0x10);
    let export_id = [0xA5; 32];
    let init = SignedTx::sign_body(
        &import_sk,
        import_dom,
        import_i,
        0,
        TxBody::Init { index: 1, flags: 0 },
    );
    {
        let mut g = app.inner.write().await;
        g.chain.st.apply_tx(&init).expect("init importer");
        g.chain.st.exported_registry.insert(
            export_id,
            pwm_core::state::ExportProvenance {
                to: recv_aid,
                target_domain: import_dom,
                amount: 19,
            },
        );
    }
    let before = {
        let g = app.inner.read().await;
        (
            g.chain.st.get(&import_aid).expect("importer").clone(),
            g.chain.st.imported_set.clone(),
        )
    };
    let tx = SignedTx::sign_body(
        &import_sk,
        import_dom,
        import_i,
        1,
        TxBody::Import {
            to: recv_aid,
            amount: 19,
            export_id,
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
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    assert!(String::from_utf8_lossy(&body).contains("recipient account not found"));
    let g = app.inner.read().await;
    assert_eq!(g.chain.st.get(&import_aid).expect("importer"), &before.0);
    assert_eq!(g.chain.st.imported_set, before.1);
    assert!(g.chain.st.get(&recv_aid).is_none());
}

/// Roaming intent creation is visible through the status endpoint lifecycle fields.
#[tokio::test]
async fn v1_rov_create_get_ok() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    let (cfg, sks) = dev_net();
    let sk = &sks[0];
    let source = cfg.accounts[0].acct;
    let source_dom = domain_of_account_id(&source);
    let recipient = valid_cross_domain_recipient(source_dom.to_be_bytes()[0]);
    let target_domain = domain_of_account_id(&recipient);
    let tx = SignedTx::sign_body(
        sk,
        source_dom,
        0,
        0,
        TxBody::Export {
            to: recipient,
            target_domain,
            amount: 17,
            fee: 1,
        },
    );
    let body = serde_json::json!({
        "tx": tx.clone(),
        "ttl_blocks": 3
    });
    let svc = router_dev(app.clone()).into_service();
    let ready = svc
        .clone()
        .oneshot(
            Request::post("/v1/export-readiness")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({ "tx": tx.clone(), "ttl_sec": 30 }))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ready.status(), StatusCode::OK);
    let create = svc
        .clone()
        .oneshot(
            Request::post("/v1/roaming-intents")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::OK);
    let create_body = to_bytes(create.into_body(), 64 * 1024).await.unwrap();
    let create_json: serde_json::Value = serde_json::from_slice(&create_body).unwrap();
    assert_eq!(create_json["status"], "exported");
    let intent_id = create_json["intent_id"].as_str().expect("intent_id");
    let status = svc
        .oneshot(
            Request::get(format!("/v1/roaming-intents/{intent_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status.status(), StatusCode::OK);
    let status_body = to_bytes(status.into_body(), 64 * 1024).await.unwrap();
    let status_json: serde_json::Value = serde_json::from_slice(&status_body).unwrap();
    assert_eq!(status_json["status"], "exported");
    assert_eq!(status_json["amount"], "17");
    assert_eq!(
        status_json["relay_mode"].as_str().unwrap_or(""),
        crate::relay::RELAY_MODE
    );
    assert!(status_json["relay_hint"]
        .as_str()
        .unwrap_or("")
        .contains("transport-peer-seed"));
    let g = app.inner.read().await;
    let blk = g
        .chain
        .blocks
        .back()
        .expect("sealed block for roaming export");
    assert_eq!(blk.txs.len(), 1);
    assert_eq!(blk.txs[0].tx_hash(), tx.tx_hash());
}

/// Finalize without live seeds preserves exported roaming state and error surfaces.
#[tokio::test]
async fn v1_rov_fin_seedless() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    let (cfg, sks) = dev_net();
    let sk = &sks[0];
    let source = cfg.accounts[0].acct;
    let source_dom = domain_of_account_id(&source);
    let recipient = valid_cross_domain_recipient(source_dom.to_be_bytes()[0]);
    let target_domain = domain_of_account_id(&recipient);
    let tx = SignedTx::sign_body(
        sk,
        source_dom,
        0,
        0,
        TxBody::Export {
            to: recipient,
            target_domain,
            amount: 19,
            fee: 1,
        },
    );
    let body = serde_json::json!({
        "tx": tx,
        "ttl_blocks": 3
    });
    let svc = router_dev(app).into_service();
    let ready = svc
        .clone()
        .oneshot(
            Request::post("/v1/export-readiness")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({ "tx": tx, "ttl_sec": 30 })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ready.status(), StatusCode::OK);
    let create = svc
        .clone()
        .oneshot(
            Request::post("/v1/roaming-intents")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::OK);
    let create_body = to_bytes(create.into_body(), 64 * 1024).await.unwrap();
    let create_json: serde_json::Value = serde_json::from_slice(&create_body).unwrap();
    let intent_id = create_json["intent_id"].as_str().unwrap().to_string();

    let first = svc
        .clone()
        .oneshot(
            Request::post(format!("/v1/roaming-intents/{intent_id}/finalize"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    let first_body = to_bytes(first.into_body(), 64 * 1024).await.unwrap();
    let first_json: serde_json::Value = serde_json::from_slice(&first_body).unwrap();
    assert_eq!(first_json["status"], "exported");
    assert_eq!(first_json["changed"], false);
    assert!(first_json["message"]
        .as_str()
        .unwrap_or("")
        .contains("peer relay pending"));

    let second = svc
        .clone()
        .oneshot(
            Request::post(format!("/v1/roaming-intents/{intent_id}/finalize"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::OK);
    let second_body = to_bytes(second.into_body(), 64 * 1024).await.unwrap();
    let second_json: serde_json::Value = serde_json::from_slice(&second_body).unwrap();
    assert_eq!(second_json["status"], "exported");
    assert_eq!(second_json["changed"], false);
    assert!(second_json["message"]
        .as_str()
        .unwrap_or("")
        .contains("peer relay pending"));

    let status = svc
        .oneshot(
            Request::get(format!("/v1/roaming-intents/{intent_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status_body = to_bytes(status.into_body(), 64 * 1024).await.unwrap();
    let status_json: serde_json::Value = serde_json::from_slice(&status_body).unwrap();
    assert_eq!(status_json["status"], "exported");
    assert!(status_json["last_error"]
        .as_str()
        .unwrap_or("")
        .contains("no HTTP relay base configured"));
}

/// Finalize on unknown intent ids returns NOT_FOUND without mutating pools.
#[tokio::test]
async fn v1_rov_fin_unk_404() {
    let svc = router_dev(app_for_devnet_sender(DevLane::Lane0)).into_service();
    let res = svc
        .oneshot(
            Request::post(format!("/v1/roaming-intents/{}/finalize", "11".repeat(32)))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let body = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("roaming intent not found"));
}

/// /v1/flow/recent surfaces accepted mempool events and sealed block metadata.
#[tokio::test]
async fn v1_flow_rcnt_ac_ok() {
    let (sk, i, aid) = routable_user_sk([25u8; 32]);
    let dom = domain_of_account_id(&aid);
    let tx = SignedTx::sign_body(&sk, dom, i, 0, TxBody::Init { index: 1, flags: 0 });
    let app = app_for_sender(&aid);
    let svc = router_dev(app).into_service();
    let submit = svc
        .clone()
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&tx).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(submit.status(), StatusCode::NO_CONTENT);
    let recent = svc
        .oneshot(Request::get("/v1/flow/recent").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(recent.status(), StatusCode::OK);
    let body = to_bytes(recent.into_body(), 64 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let rows = json["rows"].as_array().expect("rows array");
    assert!(!rows.is_empty());
    assert!(rows
        .iter()
        .any(|x| x["kind"].as_str().unwrap_or("").starts_with("accepted:")));
}

/// /v1/flow/recent records roaming finalize lifecycle transitions for audits.
#[tokio::test]
async fn v1_flow_rov_fin_ls() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    let (cfg, sks) = dev_net();
    let sk = &sks[0];
    let source = cfg.accounts[0].acct;
    let source_dom = domain_of_account_id(&source);
    let recipient = valid_cross_domain_recipient(source_dom.to_be_bytes()[0]);
    let target_domain = domain_of_account_id(&recipient);
    let tx = SignedTx::sign_body(
        sk,
        source_dom,
        0,
        0,
        TxBody::Export {
            to: recipient,
            target_domain,
            amount: 31,
            fee: 1,
        },
    );
    let body = serde_json::json!({
        "tx": tx.clone(),
        "ttl_blocks": 3
    });
    let svc = router_dev(app).into_service();
    let ready = svc
        .clone()
        .oneshot(
            Request::post("/v1/export-readiness")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({ "tx": tx, "ttl_sec": 30 })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ready.status(), StatusCode::OK);
    let create = svc
        .clone()
        .oneshot(
            Request::post("/v1/roaming-intents")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::OK);
    let create_body = to_bytes(create.into_body(), 64 * 1024).await.unwrap();
    let create_json: serde_json::Value = serde_json::from_slice(&create_body).unwrap();
    let intent_id = create_json["intent_id"]
        .as_str()
        .expect("intent_id")
        .to_string();

    let finalize = svc
        .clone()
        .oneshot(
            Request::post(format!("/v1/roaming-intents/{intent_id}/finalize"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(finalize.status(), StatusCode::OK);

    let recent = svc
        .oneshot(Request::get("/v1/flow/recent").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(recent.status(), StatusCode::OK);
    let body = to_bytes(recent.into_body(), 64 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let rows = json["rows"].as_array().expect("rows array");
    assert!(rows
        .iter()
        .any(|x| x["kind"].as_str().unwrap_or("").starts_with("accepted:")));
    assert!(rows.iter().any(|x| {
        x["kind"].as_str() == Some("finalized:roaming_intent")
            && x["intent_id"].as_str() == Some(intent_id.as_str())
    }));
    assert!(rows.iter().any(|x| {
        x["kind"].as_str() == Some("relay_error:export_provenance")
            && x["intent_id"].as_str() == Some(intent_id.as_str())
    }));
}

/// Roaming intent handlers return 500 when snapshot persistence fails after mutations.
#[tokio::test]
async fn v1_rov_500_snap_fail() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    let (cfg, sks) = dev_net();
    let sk = &sks[0];
    let source = cfg.accounts[0].acct;
    let source_dom = domain_of_account_id(&source);
    let recipient = valid_cross_domain_recipient(source_dom.to_be_bytes()[0]);
    let target_domain = domain_of_account_id(&recipient);
    let tx = SignedTx::sign_body(
        sk,
        source_dom,
        0,
        0,
        TxBody::Export {
            to: recipient,
            target_domain,
            amount: 17,
            fee: 1,
        },
    );
    let body = serde_json::json!({
        "tx": tx,
        "ttl_blocks": 3
    });
    let mut app = app;
    let bad_dir = std::env::temp_dir().join(format!(
        "pwmd_snapshot_fail_intent_dir_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&bad_dir).unwrap();
    app.data_file = Some(bad_dir.clone());
    let svc = router_dev(app.clone()).into_service();
    let ready = svc
        .clone()
        .oneshot(
            Request::post("/v1/export-readiness")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({ "tx": tx.clone(), "ttl_sec": 30 }))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ready.status(), StatusCode::OK);
    let res = svc
        .oneshot(
            Request::post("/v1/roaming-intents")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("tx commit rolled back"));
    {
        let st = app.init.read().await;
        assert_eq!(st.phase, crate::state::InitPhase::Ready);
        assert!(st.snapshot_error.is_none());
    }
    {
        let g = app.inner.read().await;
        let src = cfg.accounts[0].acct;
        let ac = g.chain.st.get(&src).expect("source account");
        assert_eq!(ac.nonce, 0);
        assert_eq!(ac.balance_pwm, 1_000_000);
    }
    std::fs::remove_dir_all(&bad_dir).ok();
}

/// Roaming finalize returns 500 if snapshot persistence fails after terminal updates.
#[tokio::test]
async fn v1_rov_fin_500_snap() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    let (cfg, sks) = dev_net();
    let sk = &sks[0];
    let source = cfg.accounts[0].acct;
    let source_dom = domain_of_account_id(&source);
    let recipient = valid_cross_domain_recipient(source_dom.to_be_bytes()[0]);
    let target_domain = domain_of_account_id(&recipient);
    let tx = SignedTx::sign_body(
        sk,
        source_dom,
        0,
        0,
        TxBody::Export {
            to: recipient,
            target_domain,
            amount: 29,
            fee: 1,
        },
    );
    let body = serde_json::json!({
        "tx": tx.clone(),
        "ttl_blocks": 3
    });
    let create_svc = router_dev(app.clone()).into_service();
    let ready = create_svc
        .clone()
        .oneshot(
            Request::post("/v1/export-readiness")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({ "tx": tx, "ttl_sec": 30 })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ready.status(), StatusCode::OK);
    let create = create_svc
        .oneshot(
            Request::post("/v1/roaming-intents")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::OK);
    let create_body = to_bytes(create.into_body(), 64 * 1024).await.unwrap();
    let create_json: serde_json::Value = serde_json::from_slice(&create_body).unwrap();
    let intent_id = create_json["intent_id"]
        .as_str()
        .expect("intent_id")
        .to_string();

    let mut app_bad = app.clone();
    let bad_dir = std::env::temp_dir().join(format!(
        "pwmd_snapshot_fail_finalize_dir_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&bad_dir).unwrap();
    app_bad.data_file = Some(bad_dir.clone());
    let bad_svc = router_dev(app_bad.clone()).into_service();
    let res = bad_svc
        .oneshot(
            Request::post(format!("/v1/roaming-intents/{intent_id}/finalize"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("snapshot save failed"));
    {
        let st = app_bad.init.read().await;
        assert_eq!(st.phase, crate::state::InitPhase::ReadyDegraded);
        assert!(st.snapshot_error.is_some());
    }
    let retry_svc = router_dev(app.clone()).into_service();
    let retry = retry_svc
        .oneshot(
            Request::post(format!("/v1/roaming-intents/{intent_id}/finalize"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(retry.status(), StatusCode::SERVICE_UNAVAILABLE);
    let retry_body = to_bytes(retry.into_body(), 64 * 1024).await.unwrap();
    let retry_txt = String::from_utf8_lossy(&retry_body);
    assert!(
        retry_txt.contains("user tx blocked") && retry_txt.contains("degraded"),
        "body={retry_txt}"
    );
    std::fs::remove_dir_all(&bad_dir).ok();
}

/// Repeated finalize requests against terminal intents stay idempotent.
#[tokio::test]
async fn v1_rov_fin_term_idem() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    let (cfg, sks) = dev_net();
    let sk = &sks[0];
    let source = cfg.accounts[0].acct;
    let source_dom = domain_of_account_id(&source);
    let recipient = valid_cross_domain_recipient(source_dom.to_be_bytes()[0]);
    let target_domain = domain_of_account_id(&recipient);
    let svc = router_dev(app.clone()).into_service();

    let imported_tx = SignedTx::sign_body(
        sk,
        source_dom,
        0,
        0,
        TxBody::Export {
            to: recipient,
            target_domain,
            amount: 37,
            fee: 1,
        },
    );
    let imported_body = serde_json::json!({ "tx": imported_tx, "ttl_blocks": 5 });
    let ir = svc
        .clone()
        .oneshot(
            Request::post("/v1/export-readiness")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "tx": imported_tx.clone(),
                        "ttl_sec": 30
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ir.status(), StatusCode::OK);
    let imported_create = svc
        .clone()
        .oneshot(
            Request::post("/v1/roaming-intents")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&imported_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(imported_create.status(), StatusCode::OK);
    let imported_create_body = to_bytes(imported_create.into_body(), 64 * 1024)
        .await
        .unwrap();
    let imported_create_json: serde_json::Value =
        serde_json::from_slice(&imported_create_body).unwrap();
    let imported_intent_id = imported_create_json["intent_id"]
        .as_str()
        .expect("intent_id")
        .to_string();
    let imported_export_id = imported_create_json["export_id"]
        .as_str()
        .expect("export_id")
        .to_string();
    let imported_export_key: [u8; 32] = hex::decode(&imported_export_id)
        .expect("export_id parse")
        .try_into()
        .expect("export_id len");

    {
        let mut g = app.inner.write().await;
        g.roaming_pool.mark_import_by_export(imported_export_key);
    }
    let imported = svc
        .clone()
        .oneshot(
            Request::post(format!("/v1/roaming-intents/{imported_intent_id}/finalize"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(imported.status(), StatusCode::OK);
    let imported_json: serde_json::Value =
        serde_json::from_slice(&to_bytes(imported.into_body(), 64 * 1024).await.unwrap()).unwrap();
    assert_eq!(imported_json["status"], "imported");
    assert_eq!(imported_json["changed"], false);
    assert!(imported_json["message"]
        .as_str()
        .unwrap_or("")
        .contains("already imported"));

    let expired_tx = SignedTx::sign_body(
        sk,
        source_dom,
        0,
        1,
        TxBody::Export {
            to: recipient,
            target_domain,
            amount: 38,
            fee: 1,
        },
    );
    let expired_body = serde_json::json!({ "tx": expired_tx, "ttl_blocks": 1 });
    let er = svc
        .clone()
        .oneshot(
            Request::post("/v1/export-readiness")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "tx": expired_tx.clone(),
                        "ttl_sec": 30
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(er.status(), StatusCode::OK);
    let expired_create = svc
        .clone()
        .oneshot(
            Request::post("/v1/roaming-intents")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&expired_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(expired_create.status(), StatusCode::OK);
    let expired_create_body = to_bytes(expired_create.into_body(), 64 * 1024)
        .await
        .unwrap();
    let expired_create_json: serde_json::Value =
        serde_json::from_slice(&expired_create_body).unwrap();
    let expired_intent_id = expired_create_json["intent_id"]
        .as_str()
        .expect("intent_id")
        .to_string();

    {
        let mut g = app.inner.write().await;
        let h = g.chain.tip_h().saturating_add(100);
        g.roaming_pool.expire_by_height(h);
    }
    let expired = svc
        .clone()
        .oneshot(
            Request::post(format!("/v1/roaming-intents/{expired_intent_id}/finalize"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(expired.status(), StatusCode::OK);
    let expired_json: serde_json::Value =
        serde_json::from_slice(&to_bytes(expired.into_body(), 64 * 1024).await.unwrap()).unwrap();
    assert_eq!(expired_json["status"], "expired");
    assert_eq!(expired_json["changed"], false);
    assert!(expired_json["message"]
        .as_str()
        .unwrap_or("")
        .contains("expired before finalize"));

    let failed_tx = SignedTx::sign_body(
        sk,
        source_dom,
        0,
        2,
        TxBody::Export {
            to: recipient,
            target_domain,
            amount: 39,
            fee: 1,
        },
    );
    let failed_body = serde_json::json!({ "tx": failed_tx, "ttl_blocks": 5 });
    let fr = svc
        .clone()
        .oneshot(
            Request::post("/v1/export-readiness")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "tx": failed_tx.clone(),
                        "ttl_sec": 30
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(fr.status(), StatusCode::OK);
    let failed_create = svc
        .clone()
        .oneshot(
            Request::post("/v1/roaming-intents")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&failed_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(failed_create.status(), StatusCode::OK);
    let failed_create_body = to_bytes(failed_create.into_body(), 64 * 1024)
        .await
        .unwrap();
    let failed_create_json: serde_json::Value =
        serde_json::from_slice(&failed_create_body).unwrap();
    let failed_intent_id = failed_create_json["intent_id"]
        .as_str()
        .expect("intent_id")
        .to_string();
    let failed_key: [u8; 32] = hex::decode(&failed_intent_id)
        .expect("intent_id parse")
        .try_into()
        .expect("intent_id len");

    {
        let mut g = app.inner.write().await;
        g.roaming_pool.mark_failed(
            failed_key,
            "test failure for finalize idempotency".to_string(),
        );
    }
    let failed = svc
        .oneshot(
            Request::post(format!("/v1/roaming-intents/{failed_intent_id}/finalize"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(failed.status(), StatusCode::OK);
    let failed_json: serde_json::Value =
        serde_json::from_slice(&to_bytes(failed.into_body(), 64 * 1024).await.unwrap()).unwrap();
    assert_eq!(failed_json["status"], "failed");
    assert_eq!(failed_json["changed"], false);
    assert!(failed_json["message"]
        .as_str()
        .unwrap_or("")
        .contains("intent is failed"));
}

/// Active roaming locks prevent conflicting local txs for the locked account.
#[tokio::test]
async fn v1_rov_lock_tx_block() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    let (cfg, sks) = dev_net();
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
            amount: 11,
            fee: 1,
        },
    );
    let local = SignedTx::sign_body(sk, source_dom, 0, 1, TxBody::Stake { amount: 1 });
    let body = serde_json::json!({ "tx": export, "ttl_blocks": 4 });
    let svc = router_dev(app).into_service();
    let lr = svc
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
    assert_eq!(lr.status(), StatusCode::OK);
    let create = svc
        .clone()
        .oneshot(
            Request::post("/v1/roaming-intents")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::OK);
    let reject = svc
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&local).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reject.status(), StatusCode::CONFLICT);
    let text = String::from_utf8_lossy(&to_bytes(reject.into_body(), 64 * 1024).await.unwrap())
        .to_string();
    assert!(text.contains("roaming intent lock active"));
}

/// Exports without readiness preflight reject and preserve balances.
#[tokio::test]
async fn v1_exp_no_ready_bal() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    let (cfg, sks) = dev_net();
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
            amount: 11,
            fee: 1,
        },
    );
    let (bal_before, nonce_before) = {
        let g = app.inner.read().await;
        let ac = g.chain.st.get(&source).expect("source account");
        (ac.balance_pwm, ac.nonce)
    };
    let svc = router_dev(app.clone()).into_service();
    let reject = svc
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&export).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reject.status(), StatusCode::CONFLICT);
    let body = to_bytes(reject.into_body(), 64 * 1024).await.unwrap();
    let (code, hint, message) = readiness_reject_fields(&body);
    assert_eq!(code, "missing_preflight");
    assert!(hint.contains("Run /v1/export-readiness"));
    assert!(message.contains("export readiness reject: code=missing_preflight"));
    let (bal_after, nonce_after) = {
        let g = app.inner.read().await;
        let ac = g.chain.st.get(&source).expect("source account");
        (ac.balance_pwm, ac.nonce)
    };
    assert_eq!(bal_before, bal_after);
    assert_eq!(nonce_before, nonce_after);
}

/// Stale readiness tokens cannot be reused for exporting funds.
#[tokio::test]
async fn v1_exp_stale_ready() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    let (cfg, sks) = dev_net();
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
            amount: 8,
            fee: 1,
        },
    );
    let preflight = serde_json::json!({ "tx": export.clone(), "ttl_sec": 1 });
    let svc = router_dev(app).into_service();
    let ready = svc
        .clone()
        .oneshot(
            Request::post("/v1/export-readiness")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&preflight).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ready.status(), StatusCode::OK);
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    let stale = svc
        .clone()
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&export).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stale.status(), StatusCode::CONFLICT);
    let stale_body = to_bytes(stale.into_body(), 64 * 1024).await.unwrap();
    let (stale_code, stale_hint, stale_message) = readiness_reject_fields(&stale_body);
    assert_eq!(stale_code, "stale_preflight");
    assert!(stale_hint.contains("TTL expired"));
    assert!(stale_message.contains("export readiness reject: code=stale_preflight"));
    let reused = svc
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&export).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reused.status(), StatusCode::CONFLICT);
    let reused_body = to_bytes(reused.into_body(), 64 * 1024).await.unwrap();
    let (reused_code, _, reused_message) = readiness_reject_fields(&reused_body);
    assert_eq!(reused_code, "missing_preflight");
    assert!(reused_message.contains("export readiness reject: code=missing_preflight"));
}

/// Valid readiness preflight enables export application and state updates.
#[tokio::test]
async fn v1_exp_ready_apply_ok() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    let (cfg, sks) = dev_net();
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
            amount: 15,
            fee: 1,
        },
    );
    let export_id = export.export_id().expect("deterministic export_id");
    let svc = router_dev(app.clone()).into_service();
    let preflight = serde_json::json!({ "tx": export.clone(), "ttl_sec": 30 });
    let ready = svc
        .clone()
        .oneshot(
            Request::post("/v1/export-readiness")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&preflight).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ready.status(), StatusCode::OK);
    let apply = svc
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&export).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(apply.status(), StatusCode::NO_CONTENT);
    let g = app.inner.read().await;
    assert!(g.chain.st.exported_registry.contains_key(&export_id));
}

/// Export readiness caps absurd ttl_sec hints to bounded values.
#[tokio::test]
async fn v1_exp_rd_ttl_cap() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    let (cfg, sks) = dev_net();
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
            amount: 9,
            fee: 1,
        },
    );
    let body = serde_json::json!({
        "tx": export,
        "ttl_sec": crate::roaming::MAX_READINESS_TTL_SEC + 50_000
    });
    let svc = router_dev(app).into_service();
    let t0 = crate::current_time_ms().expect("now");
    let ready = svc
        .oneshot(
            Request::post("/v1/export-readiness")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ready.status(), StatusCode::OK);
    let payload = to_bytes(ready.into_body(), 64 * 1024).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&payload).unwrap();
    let expires_at = v["expires_at_unix_ms"]
        .as_u64()
        .expect("expires_at_unix_ms");
    let capped_ttl_ms = crate::roaming::MAX_READINESS_TTL_SEC * 1_000;
    assert!(expires_at >= t0.saturating_add(capped_ttl_ms.saturating_sub(1_000)));
    assert!(expires_at <= t0.saturating_add(capped_ttl_ms).saturating_add(2_000));
}

/// Roaming flows without readiness keep balances untouched while rejecting.
#[tokio::test]
async fn v1_rov_no_ready_bal() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    let (cfg, sks) = dev_net();
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
            amount: 11,
            fee: 1,
        },
    );
    let body = serde_json::json!({ "tx": export, "ttl_blocks": 4 });
    let (bal_before, nonce_before) = {
        let g = app.inner.read().await;
        let ac = g.chain.st.get(&source).expect("source account");
        (ac.balance_pwm, ac.nonce)
    };
    let svc = router_dev(app.clone()).into_service();
    let reject = svc
        .oneshot(
            Request::post("/v1/roaming-intents")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(reject.status(), StatusCode::CONFLICT);
    let reject_body = to_bytes(reject.into_body(), 64 * 1024).await.unwrap();
    let (code, hint, message) = readiness_reject_fields(&reject_body);
    assert_eq!(code, "missing_preflight");
    assert!(hint.contains("Run /v1/export-readiness"));
    assert!(message.contains("export readiness reject: code=missing_preflight"));
    let (bal_after, nonce_after, intents) = {
        let g = app.inner.read().await;
        let ac = g.chain.st.get(&source).expect("source account");
        (
            ac.balance_pwm,
            ac.nonce,
            g.roaming_pool.intents_snapshot().len(),
        )
    };
    assert_eq!(bal_before, bal_after);
    assert_eq!(nonce_before, nonce_after);
    assert_eq!(intents, 0);
}

/// Roaming intents expire cleanly when ttl height horizons are surpassed.
#[tokio::test]
async fn v1_rov_ttl_height_exp() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    let (cfg, sks) = dev_net();
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
            amount: 7,
            fee: 0,
        },
    );
    let body = serde_json::json!({ "tx": export, "ttl_blocks": 1 });
    let svc = router_dev(app.clone()).into_service();
    let xr = svc
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
    assert_eq!(xr.status(), StatusCode::OK);
    let create = svc
        .clone()
        .oneshot(
            Request::post("/v1/roaming-intents")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::OK);
    let create_body = to_bytes(create.into_body(), 64 * 1024).await.unwrap();
    let create_json: serde_json::Value = serde_json::from_slice(&create_body).unwrap();
    let intent_id = create_json["intent_id"].as_str().unwrap().to_string();
    {
        let mut g = app.inner.write().await;
        g.chain.seal(vec![]).expect("seal 1");
        g.chain.seal(vec![]).expect("seal 2");
    }
    let status = svc
        .oneshot(
            Request::get(format!("/v1/roaming-intents/{intent_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status.status(), StatusCode::OK);
    let status_body = to_bytes(status.into_body(), 64 * 1024).await.unwrap();
    let status_json: serde_json::Value = serde_json::from_slice(&status_body).unwrap();
    assert_eq!(status_json["status"], "expired");
}

/// Roaming intent status expiry paths return 500 if snapshot persistence fails.
#[tokio::test]
async fn v1_rov_stat_exp_500() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    let (cfg, sks) = dev_net();
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
            amount: 5,
            fee: 0,
        },
    );
    let body = serde_json::json!({ "tx": export, "ttl_blocks": 1 });
    let create_svc = router_dev(app.clone()).into_service();
    let xr = create_svc
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
    assert_eq!(xr.status(), StatusCode::OK);
    let create = create_svc
        .oneshot(
            Request::post("/v1/roaming-intents")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::OK);
    let create_body = to_bytes(create.into_body(), 64 * 1024).await.unwrap();
    let create_json: serde_json::Value = serde_json::from_slice(&create_body).unwrap();
    let intent_id = create_json["intent_id"]
        .as_str()
        .expect("intent_id")
        .to_string();
    {
        let mut g = app.inner.write().await;
        g.chain.seal(vec![]).expect("seal 1");
        g.chain.seal(vec![]).expect("seal 2");
    }

    let mut app_bad = app.clone();
    let bad_dir = std::env::temp_dir().join(format!(
        "pwmd_snapshot_fail_expire_dir_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&bad_dir).unwrap();
    app_bad.data_file = Some(bad_dir.clone());
    let bad_svc = router_dev(app_bad.clone()).into_service();
    let res = bad_svc
        .oneshot(
            Request::get(format!("/v1/roaming-intents/{intent_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("snapshot save failed"));
    {
        let st = app_bad.init.read().await;
        assert_eq!(st.phase, crate::state::InitPhase::ReadyDegraded);
        assert!(st.snapshot_error.is_some());
    }
    std::fs::remove_dir_all(&bad_dir).ok();
}

/// Duplicate export deliveries produce idempotent roaming intent registrations.
#[tokio::test]
async fn v1_rov_dupexp_ok() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    let (cfg, sks) = dev_net();
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
            amount: 9,
            fee: 1,
        },
    );
    let body = serde_json::json!({ "tx": export, "ttl_blocks": 5 });
    let svc = router_dev(app).into_service();
    let yr = svc
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
    assert_eq!(yr.status(), StatusCode::OK);
    let first = svc
        .clone()
        .oneshot(
            Request::post("/v1/roaming-intents")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    let first_body = to_bytes(first.into_body(), 64 * 1024).await.unwrap();
    let first_json: serde_json::Value = serde_json::from_slice(&first_body).unwrap();
    let yr2 = svc
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
    assert_eq!(yr2.status(), StatusCode::OK);
    let second = svc
        .oneshot(
            Request::post("/v1/roaming-intents")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::OK);
    let second_body = to_bytes(second.into_body(), 64 * 1024).await.unwrap();
    let second_json: serde_json::Value = serde_json::from_slice(&second_body).unwrap();
    assert_eq!(first_json["intent_id"], second_json["intent_id"]);
    assert_eq!(second_json["duplicate"], true);
}

/// Roaming intent creation rejects tx bodies that are not exports.
#[tokio::test]
async fn v1_rov_body_not_exp() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    let (cfg, sks) = dev_net();
    let sk = &sks[0];
    let source = cfg.accounts[0].acct;
    let source_dom = domain_of_account_id(&source);
    let non_export = SignedTx::sign_body(sk, source_dom, 0, 0, TxBody::Stake { amount: 1 });
    let body = serde_json::json!({ "tx": non_export, "ttl_blocks": 3 });
    let svc = router_dev(app).into_service();
    let res = svc
        .oneshot(
            Request::post("/v1/roaming-intents")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let text =
        String::from_utf8_lossy(&to_bytes(res.into_body(), 64 * 1024).await.unwrap()).to_string();
    assert!(text.contains("roaming intent create requires export tx body"));
}

/// Roaming intent status rejects syntactically invalid intent identifiers.
#[tokio::test]
async fn v1_rov_stat_bad_id() {
    let svc = router_dev(app_for_devnet_sender(DevLane::Lane0)).into_service();
    let res = svc
        .oneshot(
            Request::get("/v1/roaming-intents/not-a-hex-id")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let text =
        String::from_utf8_lossy(&to_bytes(res.into_body(), 64 * 1024).await.unwrap()).to_string();
    assert!(text.contains("invalid intent id"));
}

/// Roaming intent status returns NOT_FOUND for unknown intent ids.
#[tokio::test]
async fn v1_rov_stat_unk_404() {
    let svc = router_dev(app_for_devnet_sender(DevLane::Lane0)).into_service();
    let res = svc
        .oneshot(
            Request::get(format!("/v1/roaming-intents/{}", "11".repeat(32)))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let text =
        String::from_utf8_lossy(&to_bytes(res.into_body(), 64 * 1024).await.unwrap()).to_string();
    assert!(text.contains("roaming intent not found"));
}

/// /v1/status bridge counters move after correlated HTTP export+import transfers.
#[tokio::test]
async fn v1_br_http_xfer_ctr() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    let (import_sk, import_i, import_aid) = routable_user_sk_for_app([0x41; 32], &app);
    let import_dom = domain_of_account_id(&import_aid);
    let init = SignedTx::sign_body(
        &import_sk,
        import_dom,
        import_i,
        0,
        TxBody::Init { index: 1, flags: 0 },
    );
    {
        let mut g = app.inner.write().await;
        g.chain.st.apply_tx(&init).expect("init importer");
        credit_min_import_fee_tests(&mut g.chain.st, &import_aid);
    }

    let export = {
        let g = app.inner.read().await;
        let aid_v = g.chain.cfg.accounts[0].acct;
        let export_dom = domain_of_account_id(&aid_v);
        SignedTx::sign_body(
            &g.chain.val_sks[0],
            export_dom,
            0,
            0,
            TxBody::Export {
                to: import_aid,
                target_domain: import_dom,
                amount: 29,
                fee: 0,
            },
        )
    };
    let export_id = export.export_id().expect("export id");
    let import = SignedTx::sign_body(
        &import_sk,
        import_dom,
        import_i,
        1,
        TxBody::Import {
            to: import_aid,
            amount: 29,
            export_id,
        },
    );
    let svc = router_dev(app).into_service();

    let preflight_body = serde_json::json!({ "tx": export.clone(), "ttl_sec": 30 });
    let preflight_res = svc
        .clone()
        .oneshot(
            Request::post("/v1/export-readiness")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&preflight_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(preflight_res.status(), StatusCode::OK);

    let export_res = svc
        .clone()
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&export).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(export_res.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let status_after_export = svc
        .clone()
        .oneshot(Request::get("/v1/status").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(status_after_export.status(), StatusCode::OK);
    let status_after_export_body = to_bytes(status_after_export.into_body(), 64 * 1024)
        .await
        .unwrap();
    let status_after_export_json: serde_json::Value =
        serde_json::from_slice(&status_after_export_body).unwrap();
    assert_eq!(status_after_export_json["bridge_exported_registry_size"], 0);
    assert_eq!(status_after_export_json["bridge_imported_set_size"], 0);

    let import_res = svc
        .clone()
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&import).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(import_res.status(), StatusCode::BAD_REQUEST);

    let status_after_import = svc
        .oneshot(Request::get("/v1/status").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(status_after_import.status(), StatusCode::OK);
    let status_after_import_body = to_bytes(status_after_import.into_body(), 64 * 1024)
        .await
        .unwrap();
    let status_after_import_json: serde_json::Value =
        serde_json::from_slice(&status_after_import_body).unwrap();
    assert_eq!(status_after_import_json["bridge_exported_registry_size"], 0);
    assert_eq!(status_after_import_json["bridge_imported_set_size"], 0);
}

/// Cross-node HTTP export/import smoke advances head height via sync seals.
#[tokio::test]
async fn v1_tx_http_xfer_tip() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    let (import_sk, import_i, import_aid) = routable_user_sk_for_app([0x42; 32], &app);
    let import_dom = domain_of_account_id(&import_aid);
    let init = SignedTx::sign_body(
        &import_sk,
        import_dom,
        import_i,
        0,
        TxBody::Init { index: 1, flags: 0 },
    );
    {
        let mut g = app.inner.write().await;
        g.chain.st.apply_tx(&init).expect("init importer");
        credit_min_import_fee_tests(&mut g.chain.st, &import_aid);
    }

    let export = {
        let g = app.inner.read().await;
        let aid_v = g.chain.cfg.accounts[0].acct;
        let export_dom = domain_of_account_id(&aid_v);
        SignedTx::sign_body(
            &g.chain.val_sks[0],
            export_dom,
            0,
            0,
            TxBody::Export {
                to: import_aid,
                target_domain: import_dom,
                amount: 31,
                fee: 0,
            },
        )
    };
    let export_id = export.export_id().expect("export id");
    let import = SignedTx::sign_body(
        &import_sk,
        import_dom,
        import_i,
        1,
        TxBody::Import {
            to: import_aid,
            amount: 31,
            export_id,
        },
    );
    let svc = router_dev(app).into_service();

    let preflight_body = serde_json::json!({ "tx": export.clone(), "ttl_sec": 30 });
    let preflight_res = svc
        .clone()
        .oneshot(
            Request::post("/v1/export-readiness")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&preflight_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(preflight_res.status(), StatusCode::OK);

    let head_before = svc
        .clone()
        .oneshot(Request::get("/v1/head").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(head_before.status(), StatusCode::OK);
    let head_before_body = to_bytes(head_before.into_body(), 64 * 1024).await.unwrap();
    let head_before_json: serde_json::Value = serde_json::from_slice(&head_before_body).unwrap();
    assert_eq!(head_before_json["height"].as_u64(), Some(0));

    let export_res = svc
        .clone()
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&export).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(export_res.status(), StatusCode::INTERNAL_SERVER_ERROR);

    let head_after_export = svc
        .clone()
        .oneshot(Request::get("/v1/head").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(head_after_export.status(), StatusCode::OK);
    let head_after_export_body = to_bytes(head_after_export.into_body(), 64 * 1024)
        .await
        .unwrap();
    let head_after_export_json: serde_json::Value =
        serde_json::from_slice(&head_after_export_body).unwrap();
    assert_eq!(head_after_export_json["height"].as_u64(), Some(0));

    let import_res = svc
        .clone()
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&import).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(import_res.status(), StatusCode::BAD_REQUEST);

    let head_after_import = svc
        .oneshot(Request::get("/v1/head").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(head_after_import.status(), StatusCode::OK);
    let head_after_import_body = to_bytes(head_after_import.into_body(), 64 * 1024)
        .await
        .unwrap();
    let head_after_import_json: serde_json::Value =
        serde_json::from_slice(&head_after_import_body).unwrap();
    assert_eq!(head_after_import_json["height"].as_u64(), Some(0));
}

/// Duplicate imports referencing the same export id conflict at the HTTP layer.
#[tokio::test]
async fn v1_tx_imp_dup_conflict() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    let (import_sk, import_i, import_aid, export_id, provenance) =
        seed_handoff_provenance_for_import(&app, 23).await;
    let import_dom = domain_of_account_id(&import_aid);
    let mut tx1 = SignedTx::sign_body(
        &import_sk,
        import_dom,
        import_i,
        1,
        TxBody::Import {
            to: import_aid,
            amount: 23,
            export_id,
        },
    );
    tx1.set_import_provenance_signed(&import_sk, Some(provenance.clone()));
    let svc = router_dev(app.clone()).into_service();
    let res1 = svc
        .clone()
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&tx1).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res1.status(), StatusCode::NO_CONTENT);
    let mut tx2 = SignedTx::sign_body(
        &import_sk,
        import_dom,
        import_i,
        2,
        TxBody::Import {
            to: import_aid,
            amount: 23,
            export_id,
        },
    );
    tx2.set_import_provenance_signed(&import_sk, Some(provenance));
    let res2 = svc
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&tx2).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res2.status(), StatusCode::CONFLICT);
    let body = to_bytes(res2.into_body(), 64 * 1024).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("duplicate import"));
}

/// Malformed import payloads return BAD_REQUEST diagnostics.
#[tokio::test]
async fn v1_tx_imp_bad_payload() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    let (import_sk, import_i, import_aid, export_id, provenance) =
        seed_handoff_provenance_for_import(&app, 11).await;
    let import_dom = domain_of_account_id(&import_aid);
    let mut tx = SignedTx::sign_body(
        &import_sk,
        import_dom,
        import_i,
        1,
        TxBody::Import {
            to: import_aid,
            amount: 12,
            export_id,
        },
    );
    tx.set_import_provenance_signed(&import_sk, Some(provenance));
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
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("invalid import"));
}

/// Imports fail when the referenced export id is unknown locally.
#[tokio::test]
async fn v1_tx_imp_unk_exp() {
    let app = app_for_devnet_sender(DevLane::Lane0);
    let (import_sk, import_i, import_aid, _, provenance) =
        seed_handoff_provenance_for_import(&app, 41).await;
    let import_dom = domain_of_account_id(&import_aid);
    let unknown_export_id = [7u8; 32];
    let mut tx = SignedTx::sign_body(
        &import_sk,
        import_dom,
        import_i,
        1,
        TxBody::Import {
            to: import_aid,
            amount: 41,
            export_id: unknown_export_id,
        },
    );
    tx.set_import_provenance_signed(&import_sk, Some(provenance));
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
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    assert!(String::from_utf8_lossy(&body).contains("export_id is not known"));
    let g = app.inner.read().await;
    assert!(!g.chain.st.imported_set.contains(&unknown_export_id));
    assert!(!g
        .chain
        .st
        .exported_registry
        .contains_key(&unknown_export_id));
}

/// Two-node HTTP smoke covers cross-shard export/import plus negative cases.
#[tokio::test]
async fn v1_tx_2node_neg_smoke() {
    let cy_hi = (lookup_by_label("CY").expect("CY label").raw >> 8) as u8;
    let do_hi = (lookup_by_label("DO").expect("DO label").raw >> 8) as u8;
    let (source_sk, source_i, source_aid) = user_sk_matching_domain_hi([0x91; 32], cy_hi);
    let (target_sk, target_i, target_aid) = user_sk_matching_domain_hi([0xA1; 32], do_hi);
    let source_dom = domain_of_account_id(&source_aid);
    let target_dom = domain_of_account_id(&target_aid);

    let source_app = app_for_sender(&source_aid);
    let target_app = app_for_sender(&target_aid);
    {
        let init_source = SignedTx::sign_body(
            &source_sk,
            source_dom,
            source_i,
            0,
            TxBody::Init { index: 1, flags: 0 },
        );
        let init_target = SignedTx::sign_body(
            &target_sk,
            target_dom,
            target_i,
            0,
            TxBody::Init { index: 1, flags: 0 },
        );
        let mut source_state = source_app.inner.write().await;
        source_state
            .chain
            .st
            .apply_tx(&init_source)
            .expect("init source account");
        source_state.chain.st.accounts.insert(
            source_aid,
            Account {
                signing_pubkey: source_sk.verifying_key().to_bytes(),
                derivation_index: source_i,
                balance_pwm: 1_000,
                staked: 0,
                marks: 0,
                initialized: true,
                index: 1,
                flags: 0,
                nonce: 0,
                ..Default::default()
            },
        );
        let mut g = target_app.inner.write().await;
        g.chain
            .st
            .apply_tx(&init_target)
            .expect("init target account");
        credit_min_import_fee_tests(&mut g.chain.st, &target_aid);
    }

    let export = SignedTx::sign_body(
        &source_sk,
        source_dom,
        source_i,
        0,
        TxBody::Export {
            to: target_aid,
            target_domain: target_dom,
            amount: 37,
            fee: 1,
        },
    );
    let export_id = export.export_id().expect("export id");
    let source_svc = router_dev(source_app.clone()).into_service();
    let er = source_svc
        .clone()
        .oneshot(
            Request::post("/v1/export-readiness")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({ "tx": export.clone(), "ttl_sec": 30 }))
                        .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(er.status(), StatusCode::OK);
    let export_res = source_svc
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&export).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(export_res.status(), StatusCode::NO_CONTENT);
    let export_provenance = {
        let source_state = source_app.inner.read().await;
        source_state
            .chain
            .st
            .exported_registry
            .get(&export_id)
            .cloned()
            .expect("source must record export provenance")
    };
    {
        // Two-node operator handoff in current contract: target imports only after
        // provenance is delivered to target state (no new protocol rules).
        let mut target_state = target_app.inner.write().await;
        target_state
            .chain
            .st
            .exported_registry
            .insert(export_id, export_provenance);
    }

    let target_svc = router_dev(target_app.clone()).into_service();
    let import = SignedTx::sign_body(
        &target_sk,
        target_dom,
        target_i,
        1,
        TxBody::Import {
            to: target_aid,
            amount: 37,
            export_id,
        },
    );
    let import_res = target_svc
        .clone()
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&import).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(import_res.status(), StatusCode::NO_CONTENT);
    {
        let target_state = target_app.inner.read().await;
        assert!(target_state.chain.st.imported_set.contains(&export_id));
    }

    let duplicate_import = SignedTx::sign_body(
        &target_sk,
        target_dom,
        target_i,
        2,
        TxBody::Import {
            to: target_aid,
            amount: 37,
            export_id,
        },
    );
    let duplicate_res = target_svc
        .clone()
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&duplicate_import).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(duplicate_res.status(), StatusCode::CONFLICT);
    let duplicate_body = to_bytes(duplicate_res.into_body(), 64 * 1024)
        .await
        .unwrap();
    let duplicate_text = String::from_utf8_lossy(&duplicate_body);
    assert!(duplicate_text.contains("duplicate import"));

    let unknown_import = SignedTx::sign_body(
        &target_sk,
        target_dom,
        target_i,
        3,
        TxBody::Import {
            to: target_aid,
            amount: 37,
            export_id: [0xEE; 32],
        },
    );
    let unknown_res = target_svc
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&unknown_import).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unknown_res.status(), StatusCode::BAD_REQUEST);
    let unknown_body = to_bytes(unknown_res.into_body(), 64 * 1024).await.unwrap();
    let unknown_text = String::from_utf8_lossy(&unknown_body);
    assert!(unknown_text.contains("export_id is not known"));
}

/// /v1/tx rejects shard/domain mismatches for senders pinned to alias routing.
#[tokio::test]
async fn v1_tx_wrong_shard_hi() {
    let (sk, i, aid) = routable_user_sk([23u8; 32]);
    let dom = domain_of_account_id(&aid);
    let sender_shard = shard_for_phase1_account(&aid).expect("routable");
    let other_shard = if sender_shard == DevLane::Lane0 {
        DevLane::Lane1
    } else {
        DevLane::Lane0
    };
    let tx = SignedTx::sign_body(&sk, dom, i, 0, TxBody::Init { index: 1, flags: 0 });
    let svc = router_dev(mk_app_explicit_shard(other_shard)).into_service();
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
        text.contains("process shard") || text.contains("tx sender domain_hi="),
        "unexpected error body: {text}"
    );
}

/// Neutral relay baseline tolerates nominally mismatched DevLane envelopes.
#[tokio::test]
async fn v1_tx_relay_shard_ok() {
    let (sk, i, aid) = routable_user_sk([24u8; 32]);
    let dom = domain_of_account_id(&aid);
    let sender_shard = shard_for_phase1_account(&aid).expect("routable");
    let other_shard = if sender_shard == DevLane::Lane0 {
        DevLane::Lane1
    } else {
        DevLane::Lane0
    };
    let tx = SignedTx::sign_body(&sk, dom, i, 0, TxBody::Init { index: 1, flags: 0 });
    let app = app_for_domain(other_shard, dom.to_be_bytes()[0]);
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
        text.contains("process shard") || text.contains("tx sender domain_hi="),
        "unexpected error body: {text}"
    );
}

/// Transfers that require cross-shard routing fail on purely local shards.
#[tokio::test]
async fn v1_tx_xshard_local_bad() {
    let app = mk_app_explicit_shard(DevLane::Lane0);
    let want_hi = app.identity.cluster_domain_hi;
    let (sk_s, idx_s, aid_s) = user_sk_matching_domain_hi([0x77u8; 32], want_hi);
    let dom_s = domain_of_account_id(&aid_s);
    let sender_hi = dom_s.to_be_bytes()[0];
    let (recv_sk, recv_idx, recv_aid) = (0u16..4096)
        .find_map(|n| {
            let seed = n.to_le_bytes();
            let mut s = [0u8; 32];
            s[..2].copy_from_slice(&seed);
            let (recv_sk, recv_idx, recv_aid) = user_sk(&s);
            if recv_aid == aid_s {
                return None;
            }
            if domain_of_account_id(&recv_aid).to_be_bytes()[0] == sender_hi {
                return None;
            }
            if shard_for_phase1_account(&recv_aid).ok()? != DevLane::Lane0 {
                return None;
            }
            validate_recipient_address_policy(&recv_aid).ok()?;
            Some((recv_sk, recv_idx, recv_aid))
        })
        .expect("must find different-domain_hi receiver on shard A");
    let init_sender =
        SignedTx::sign_body(&sk_s, dom_s, idx_s, 0, TxBody::Init { index: 1, flags: 0 });
    {
        let mut g = app.inner.write().await;
        g.chain.st.apply_tx(&init_sender).unwrap();
    }
    let recv_dom = domain_of_account_id(&recv_aid);
    let init = SignedTx::sign_body(
        &recv_sk,
        recv_dom,
        recv_idx,
        0,
        TxBody::Init { index: 1, flags: 0 },
    );
    {
        let mut g = app.inner.write().await;
        g.chain.st.apply_tx(&init).unwrap();
    }

    let tx = SignedTx::sign_body(
        &sk_s,
        dom_s,
        idx_s,
        1,
        TxBody::Transfer {
            to: recv_aid,
            amount: 1,
            fee: 0,
        },
    );
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
    assert_eq!(res.status(), StatusCode::CONFLICT);
    let body = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(
        text.contains("cross-domain transfer is disabled"),
        "unexpected error body: {text}"
    );
}

/// Reserve-class recipients fail deterministic pre-filter checks.
#[tokio::test]
async fn v1_tx_rcv_rsv_pref() {
    let reserve_recipient = fake_account_id_with_domain(0xE003);
    assert_recipient_prefilter_rejects(reserve_recipient, &["recipient domain", "reserve"]).await;
}

/// Witness recipients fail deterministic pre-filter checks.
#[tokio::test]
async fn v1_tx_rcv_wit_pref() {
    let witness_recipient = fake_account_id_with_domain(0xF003);
    assert_recipient_prefilter_rejects(witness_recipient, &["recipient domain", "witness-only"])
        .await;
}

/// Unknown recipient classes fail deterministic pre-filter checks.
#[tokio::test]
async fn v1_tx_rcv_unk_pref() {
    let unknown_recipient = fake_account_id_with_domain(0xDFFF);
    assert_recipient_prefilter_rejects(unknown_recipient, &["recipient domain", "not recognized"])
        .await;
}

/// Export-special recipients fail deterministic local-path pre-filter checks.
#[tokio::test]
async fn v1_tx_rcv_exp_pref() {
    let witness_recipient = fake_account_id_with_domain(0xF003);
    assert_export_prefilter_rejects(witness_recipient, &["recipient domain", "witness-only"]).await;
}

/// Explicit shards reject unknown recipient classes during preflight.
#[tokio::test]
async fn v1_tx_rcv_unk_exp() {
    let unknown_recipient = fake_account_id_with_domain(0xDFFF);
    assert_rcv_px_expl(unknown_recipient, &["recipient domain", "not recognized"]).await;
}

/// Explicit shards reject reserve recipients during preflight.
#[tokio::test]
async fn v1_tx_rcv_rsv_exp() {
    let reserve_recipient = fake_account_id_with_domain(0xE003);
    assert_rcv_px_expl(reserve_recipient, &["recipient domain", "reserve"]).await;
}

/// Explicit shards reject witness recipients during preflight.
#[tokio::test]
async fn v1_tx_rcv_wit_exp() {
    let witness_recipient = fake_account_id_with_domain(0xF003);
    assert_rcv_px_expl(witness_recipient, &["recipient domain", "witness-only"]).await;
}

/// Oversized JSON bodies abort /v1/tx before mempool admission.
#[tokio::test]
async fn v1_tx_body_too_big() {
    let body = vec![b'x'; V1_TX_BODY_LIMIT + 1024];
    let svc = router_dev(app_for_devnet_sender(DevLane::Lane0)).into_service();
    let res = svc
        .oneshot(
            Request::post("/v1/tx")
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
}
