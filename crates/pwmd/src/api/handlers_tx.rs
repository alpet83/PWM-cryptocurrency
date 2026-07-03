//! HTTP handlers for signed transaction submission (`POST /v1/tx`).

use super::common::{
    acct_view, ensure_user_tx_allowed, push_readiness_reject_flow, push_tx_flow,
    readiness_reject_json, rollback_commit, snap_save_locked, take_bak, tx_id_hex, tx_kind,
    tx_reject_json,
};
use crate::ledger::{summary_log_line, SUMMARY_BLOCK_INTERVAL};
use crate::pipeline::{counters, dispatch, ClientTxJob, DispatchInput, TxRejectReason};
use crate::relay;
use crate::roaming::ACTIVE_LOCK_ERR_TEXT;
use crate::tx_policy::{
    enforce_import_provenance_prefilter, enforce_local_tx_guards, enforce_recipient_init_gate,
    enforce_recipient_prefilter,
};
use crate::App;
use axum::{extract::State, http::StatusCode, Json};
use pwm_core::tx::TxBody;
use pwm_core::{validate_tx_shape, SignedTx};
use std::sync::Arc;
use tokio::sync::oneshot;
use tracing::{debug_span, error, info};

fn count_reject(err: (StatusCode, String)) -> (StatusCode, String) {
    counters::inc_rejected();
    err
}
fn worker_reject_status(reason: &TxRejectReason) -> StatusCode {
    match reason {
        TxRejectReason::ShapeInvalid(_) => StatusCode::BAD_REQUEST,
        _ => StatusCode::UNPROCESSABLE_ENTITY,
    }
}

fn worker_reject_msg(tx: &SignedTx, reason: &TxRejectReason) -> String {
    match reason {
        TxRejectReason::ShapeInvalid(detail) => tx_reject_json(
            tx,
            "preflight",
            detail,
            format!("tx validation failed: {detail}"),
        ),
        _ => reason.to_string(),
    }
}

pub(super) async fn v1_tx(
    State(a): State<App>,
    Json(tx): Json<SignedTx>,
) -> Result<StatusCode, (StatusCode, String)> {
    counters::inc_incoming();
    ensure_user_tx_allowed(&a).await.map_err(count_reject)?;
    enforce_recipient_prefilter(&tx).map_err(count_reject)?;
    if a.identity.mode.is_shard_enforced() {
        if relay::is_foreign_import(&tx, a.identity.cluster_domain_hi) {
            match relay::relay_import(&a, &tx).await {
                Ok(()) => return Ok(StatusCode::NO_CONTENT),
                Err(e) => {
                    let ex = match &tx.body {
                        TxBody::Import { export_id, .. } => Some(hex::encode(export_id)),
                        _ => None,
                    };
                    relay::log_relay_absence(&a, "import", &e, ex.as_deref(), None).await;
                    return Err(count_reject((e.status, e.message)));
                }
            }
        }
        enforce_local_tx_guards(&tx, a.shard, a.identity.cluster_domain_hi)
            .map_err(count_reject)?;
    }
    if let TxBody::Import { export_id, .. } = &tx.body {
        info!(
            export_id = %hex::encode(export_id),
            "v1_tx: local import entering provenance prefilter"
        );
    }
    {
        let g = a.inner.read().await;
        enforce_import_provenance_prefilter(&tx, &g.chain.st, &g.cross_shard)
            .map_err(count_reject)?;
        enforce_recipient_init_gate(&tx, &g.chain.st).map_err(count_reject)?;
    }
    if let TxBody::Import { export_id, .. } = &tx.body {
        info!(
            export_id = %hex::encode(export_id),
            "v1_tx: local import passed prefilter and recipient gate"
        );
    }
    let lock_conflict = {
        let g = a.inner.read().await;
        g.roaming_pool.lock_conflict_for(&tx).is_some()
    };
    if lock_conflict {
        return Err(count_reject((
            StatusCode::CONFLICT,
            ACTIVE_LOCK_ERR_TEXT.to_string(),
        )));
    }
    let now_h = { a.inner.read().await.chain.tip_h() };
    match &tx.body {
        TxBody::Export { .. } | TxBody::Import { .. } | TxBody::ClaimIPv4Batch { .. } => {
            let mut g = a.inner.write().await;
            // Cancellation contract: this direct-seal branch is not fully cancel-safe once
            // `g.chain.seal` succeeds. Explicit seal/snapshot errors roll back via `bak`,
            // but an HTTP future cancelled after seal can leave the block committed before
            // later cross-shard/roaming bookkeeping or init-state publication completes.
            // Retried submissions are expected to be idempotent at the chain layer via
            // nonce/export-id replay checks (usually BadNonce/DuplicateImport); durable
            // cancellation robustness would need a separate background/idempotent section.
            let sender = tx.computed_account_id();
            if matches!(tx.body, TxBody::Export { .. }) {
                let now_ms = crate::current_time_ms().map_err(count_reject)?;
                let (_, sender_nonce) = acct_view(&g.chain.st, &sender);
                if let Err(reject) =
                    g.roaming_pool
                        .consume_readiness(&tx, now_ms, sender_nonce, now_h)
                {
                    push_readiness_reject_flow(&mut g, &tx, reject.code, now_h);
                    return Err(count_reject((
                        StatusCode::CONFLICT,
                        readiness_reject_json(reject.code),
                    )));
                }
            }
            if let Err(err) = validate_tx_shape(&tx) {
                return Err(count_reject((
                    StatusCode::BAD_REQUEST,
                    tx_reject_json(
                        &tx,
                        "preflight",
                        &err,
                        format!("tx validation failed: {err}"),
                    ),
                )));
            }
            let bak = take_bak(&g);
            let (bal_before, nonce_before) = acct_view(&g.chain.st, &sender);
            let h = g.chain.tip_h();
            push_tx_flow(&mut g, &tx, h, "accepted", None);
            if let TxBody::Import { export_id, .. } = &tx.body {
                info!(
                    export_id = %hex::encode(export_id),
                    tip_h = h,
                    "v1_tx: local import sealing"
                );
            }
            if let Err((msg, _)) = g.chain.seal(vec![tx.clone()]) {
                rollback_commit(&mut g, bak);
                return Err(count_reject((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("seal after roaming tx failed: {msg}"),
                )));
            }
            let h = g.chain.tip_h();
            push_tx_flow(
                &mut g,
                &tx,
                h,
                "applied",
                Some("sealed with tx payload".to_string()),
            );
            if let TxBody::Import { export_id, .. } = &tx.body {
                info!(
                    export_id = %hex::encode(export_id),
                    tip_h = h,
                    "v1_tx: local import sealed"
                );
            }
            match &tx.body {
                TxBody::Export { .. } => {
                    g.record_cross_shard_tx(&tx, h);
                    push_tx_flow(
                        &mut g,
                        &tx,
                        h,
                        "exported",
                        Some("export registry updated".to_string()),
                    );
                    if let Some(export_id) = tx.export_id() {
                        if let Some(intent_id) = g
                            .roaming_pool
                            .get_by_export_id(&export_id)
                            .map(|x| x.intent_id)
                        {
                            g.roaming_pool.mark_exported(intent_id);
                            let h = g.chain.tip_h();
                            push_tx_flow(
                                &mut g,
                                &tx,
                                h,
                                "roaming_status",
                                Some("intent marked exported".to_string()),
                            );
                        }
                    }
                }
                TxBody::Import { export_id, .. } => {
                    g.record_cross_shard_tx(&tx, h);
                    push_tx_flow(
                        &mut g,
                        &tx,
                        h,
                        "imported",
                        Some("import replay guard updated".to_string()),
                    );
                    g.roaming_pool.mark_import_by_export(*export_id);
                    let h = g.chain.tip_h();
                    push_tx_flow(
                        &mut g,
                        &tx,
                        h,
                        "roaming_status",
                        Some("intent marked imported".to_string()),
                    );
                }
                _ => {}
            }
            let (bal_after, nonce_after) = acct_view(&g.chain.st, &sender);
            info!(
                "tx commit delta: kind={} tx_id={} sender={} bal:{}->{} nonce:{}->{}",
                tx_kind(&tx),
                tx_id_hex(&tx),
                hex::encode(sender),
                bal_before,
                bal_after,
                nonce_before,
                nonce_after
            );
            g.roaming_pool.expire_by_height(h);
            push_tx_flow(&mut g, &tx, h, "sealed", None);
            if h > 0 && h % SUMMARY_BLOCK_INTERVAL == 0 {
                info!("{}", summary_log_line(&g.cross_shard.summary()));
            }
            let save_result = snap_save_locked(&a, &g);
            if let Some((path, result)) = save_result {
                if let Err(e) = result {
                    rollback_commit(&mut g, bak);
                    error!(
                        "snapshot save after_tx failed path={}: {}",
                        path.as_ref()
                            .map(|p| p.display().to_string())
                            .unwrap_or_else(|| "-".into()),
                        e
                    );
                    return Err(count_reject((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("tx commit rolled back: snapshot save failed: {e}"),
                    )));
                }
                counters::inc_sealed();
                drop(g);
                let mut st = a.init.write().await;
                *st = crate::state::InitState::ready(path);
                return Ok(StatusCode::NO_CONTENT);
            }
            counters::inc_sealed();
            Ok(StatusCode::NO_CONTENT)
        }
        _ => {
            let tx_id = tx_id_hex(&tx);
            run_worker_precheck(&a, Arc::new(tx)).await?;
            let h = { a.inner.read().await.chain.tip_h() };
            info!(tx_id = %tx_id, h = h, "accepted: queued via worker");
            Ok(StatusCode::NO_CONTENT)
        }
    }
}

async fn run_worker_precheck(a: &App, tx: Arc<SignedTx>) -> Result<(), (StatusCode, String)> {
    let (reply, rx) = oneshot::channel();
    let job = ClientTxJob::new(Arc::clone(&tx), reply);
    let depth = a.pipeline_metrics.start_dispatch();
    {
        let _span = debug_span!("dispatch").entered();
        if dispatch(&a.worker_queues, DispatchInput::ClientTx(job)).is_err() {
            a.pipeline_metrics.cancel_dispatch();
            counters::inc_rejected();
            return Err((
                StatusCode::INSUFFICIENT_STORAGE,
                "tx worker queue is full".to_string(),
            ));
        }
    }
    a.pipeline_metrics.commit_dispatch(depth);
    a.pipeline_metrics.inc_enqueued();
    match rx.await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(reason)) => {
            counters::inc_rejected();
            Err((
                worker_reject_status(&reason),
                worker_reject_msg(tx.as_ref(), &reason),
            ))
        }
        Err(_) => {
            counters::inc_rejected();
            Err((
                StatusCode::SERVICE_UNAVAILABLE,
                "tx worker reply canceled".to_string(),
            ))
        }
    }
}
