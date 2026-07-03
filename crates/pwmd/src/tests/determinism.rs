//! Determinism and admission-pressure tests for the V7-S2 tx pipeline.

use super::helpers::*;
use super::prelude::*;
use crate::pipeline::{
    dispatch, ClientTxJob, DispatchInput, DispatchQueues, HotIndex, QueueMetrics, WorkerCtx,
    WorkerPool, WorkerReads,
};
use crate::state::StateSnapshot;
use pwm_core::state::PolicyDecision;
use pwm_core::types::{conservation_flag, cosign_non_dis};
use pwm_core::SealEntry;
use std::collections::BTreeSet;
use std::sync::{atomic::AtomicU64, Arc};
use tokio::sync::{mpsc, oneshot};

struct DetFixture {
    cfg: pwm_core::GenCfg,
    vals: Vec<SigningKey>,
    txs: Vec<SignedTx>,
}

struct WorkerRun {
    entries: Vec<SealEntry>,
    order: Vec<pwm_core::AccountId>,
}

fn find_sender(
    base: u64,
    want_hi: u8,
    seen: &mut BTreeSet<pwm_core::AccountId>,
) -> (SigningKey, u32, pwm_core::AccountId) {
    for n in 0..300_000u64 {
        let mut seed = [0x61u8; 32];
        seed[0..8].copy_from_slice(&base.saturating_mul(300_000).saturating_add(n).to_le_bytes());
        let (sk, idx, aid) = user_sk(&seed);
        if seen.contains(&aid) {
            continue;
        }
        let dom = domain_of_account_id(&aid);
        if dom.to_be_bytes()[0] != want_hi {
            continue;
        }
        if validate_recipient_address_policy(&aid).is_err() {
            continue;
        }
        if conservation_flag(&aid) || cosign_non_dis(&aid) {
            continue;
        }
        let probe = SignedTx::sign_body(&sk, dom, idx, 0, TxBody::Init { index: 0, flags: 0 });
        if validate_tx_shape(&probe).is_ok() {
            seen.insert(aid);
            return (sk, idx, aid);
        }
    }
    panic!("failed to find fixture sender for domain_hi=0x{want_hi:02X}");
}

fn init_batch(count: u64) -> DetFixture {
    let (mut cfg, sks) = dev_net();
    let recipient = cfg.accounts[0].acct;
    let want_hi = domain_of_account_id(&recipient).to_be_bytes()[0];
    let mut seen = BTreeSet::new();
    let mut txs = Vec::new();
    for n in 0..count {
        let (sk, idx, aid) = find_sender(n, want_hi, &mut seen);
        let row = pwm_core::genesis::GRow {
            acct: aid,
            pubkey: sk.verifying_key().to_bytes(),
            der_idx: idx,
            bal: 1_000,
        };
        cfg.funding.accounts.push(row.clone());
        cfg.accounts.push(row);
        txs.push(SignedTx::sign_body(
            &sk,
            domain_of_account_id(&aid),
            idx,
            0,
            TxBody::Transfer {
                to: recipient,
                amount: 1,
                fee: 0,
            },
        ));
    }
    DetFixture {
        cfg,
        vals: sks,
        txs,
    }
}

fn clear_policies(chain: &mut Chain) {
    for account in chain.st.accounts.values_mut() {
        account.active_policies = 0;
        account.dormant_policies = 0;
        account.deferred_policies.clear();
        account.finalized = false;
    }
}

fn run_worker_pool(workers: usize, fix: &DetFixture) -> WorkerRun {
    let mut chain = Chain::boot(fix.cfg.clone(), fix.vals.clone());
    clear_policies(&mut chain);
    let snapshot = Arc::new(StateSnapshot::new(Arc::new(chain.st.clone())));
    let (queues, receivers) = DispatchQueues::new_with_workers(fix.txs.len(), 1, 1);
    let (valid_tx, mut valid_rx) = mpsc::channel(fix.txs.len());
    let reads = WorkerReads::new(
        snapshot,
        Arc::new(HotIndex::new(&chain.st)),
        Arc::new(fix.cfg.clone()),
        Arc::new(AtomicU64::new(chain.tip_h())),
    );
    let ctx = WorkerCtx::new(reads, valid_tx, Arc::new(QueueMetrics::default()));
    let pool = WorkerPool::new(1, workers.saturating_sub(1), Arc::new(receivers), ctx);
    let mut replies = Vec::new();

    for tx in &fix.txs {
        assert!(
            matches!(
                chain.st.evaluate_policy(tx, chain.tip_h()),
                PolicyDecision::Allow
            ),
            "fixture policy reject: {:?}",
            chain.st.evaluate_policy(tx, chain.tip_h())
        );
        let (reply, rx) = oneshot::channel();
        let job = ClientTxJob::new(Arc::new(tx.clone()), reply);
        dispatch(&queues, DispatchInput::ClientTx(job)).expect("dispatch worker tx");
        replies.push(rx);
    }
    for rx in replies {
        assert_eq!(rx.blocking_recv().expect("worker reply"), Ok(()));
    }
    drop(queues);

    let mut valid = Vec::new();
    for _ in 0..fix.txs.len() {
        valid.push(valid_rx.blocking_recv().expect("validated tx"));
    }
    for handle in pool.handles {
        handle.join().expect("worker exits");
    }

    let order = valid.iter().map(|v| v.tx.computed_account_id()).collect();
    let entries = valid
        .into_iter()
        .map(|v| SealEntry::PreValidated {
            tx: v.tx,
            at_height: v.validated_at_height,
        })
        .collect();
    WorkerRun { entries, order }
}

fn seal_batch(fix: &DetFixture, entries: Vec<SealEntry>) -> [u8; 32] {
    let mut chain = Chain::boot(fix.cfg.clone(), fix.vals.clone());
    clear_policies(&mut chain);
    chain.seal_entries(entries).expect("seal worker batch");
    pwm_core::state::digest(&chain.st)
}

#[test]
fn determinism_1_vs_n_workers() {
    let fix = init_batch(50);
    let single = run_worker_pool(1, &fix);
    let parallel = run_worker_pool(8, &fix);

    assert_eq!(single.entries.len(), fix.txs.len());
    assert_eq!(parallel.entries.len(), fix.txs.len());
    assert_ne!(single.order, parallel.order);
    assert_eq!(
        seal_batch(&fix, single.entries),
        seal_batch(&fix, parallel.entries)
    );
}

fn init_tx_for_app(app: &App) -> SignedTx {
    let (sk, idx, aid) = routable_user_sk_for_app([0x71; 32], app);
    SignedTx::sign_body(
        &sk,
        domain_of_account_id(&aid),
        idx,
        0,
        TxBody::Init {
            index: idx,
            flags: 0,
        },
    )
}

fn fill_client_queue(app: &App, tx: &SignedTx) {
    loop {
        let (reply, _rx) = oneshot::channel();
        let job = ClientTxJob::new(Arc::new(tx.clone()), reply);
        if dispatch(&app.worker_queues, DispatchInput::ClientTx(job)).is_err() {
            break;
        }
    }
}

#[tokio::test]
async fn dos_512_post_507_ready() {
    let mut app = app_for_devnet_sender(DevLane::Lane0);
    let (queues, _receivers) = DispatchQueues::new_with_receivers(1, 1, 1);
    app.worker_queues = std::sync::Arc::new(queues);
    let tx = init_tx_for_app(&app);
    fill_client_queue(&app, &tx);

    let svc = router_dev(app.clone()).into_service();
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut joins = tokio::task::JoinSet::new();
        let body = serde_json::to_vec(&tx).expect("tx json");
        for _ in 0..512 {
            let svc = svc.clone();
            let body = body.clone();
            joins.spawn(async move {
                svc.oneshot(
                    Request::post("/v1/tx")
                        .header("content-type", "application/json")
                        .body(Body::from(body))
                        .expect("request"),
                )
                .await
                .expect("response")
                .status()
            });
        }
        while let Some(result) = joins.join_next().await {
            assert_eq!(result.expect("join"), StatusCode::INSUFFICIENT_STORAGE);
        }
    })
    .await
    .expect("DoS flood timed out");

    let status = router_dev(app)
        .into_service()
        .oneshot(Request::get("/v1/status").body(Body::empty()).unwrap())
        .await
        .expect("status response");
    assert_eq!(status.status(), StatusCode::OK);
    let bytes = to_bytes(status.into_body(), 64 * 1024).await.unwrap();
    let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(json["ready"], true);
}
