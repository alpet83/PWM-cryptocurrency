//! HTTP handlers for signed transaction submission (`POST /v1/tx`).

use super::common::{
    acct_view, ensure_user_tx_allowed, persist_snap, push_readiness_reject_flow, push_tx_flow,
    readiness_reject_json, rollback_commit, snap_save_locked, take_bak, tx_id_hex, tx_kind,
    tx_reject_json,
};
use crate::ledger::{summary_log_line, SUMMARY_BLOCK_INTERVAL};
use crate::relay;
use crate::roaming::ACTIVE_LOCK_ERR_TEXT;
use crate::tx_policy::{
    enforce_import_provenance_prefilter, enforce_local_tx_guards, enforce_recipient_init_gate,
    enforce_recipient_prefilter,
};
use crate::App;
use axum::{extract::State, http::StatusCode, Json};
use pwm_core::tx::{TxBody, TxError};
use pwm_core::{validate_tx_shape, SignedTx};
use tracing::{error, info};

fn tx_tip_precheck_err(tx: &SignedTx, e: TxError) -> (StatusCode, String) {
    use TxError::*;
    let status = match e {
        BadNonce | Insufficient | InsufficientMarks | AlreadyInit | DuplicateImport => {
            StatusCode::CONFLICT
        }
        _ => StatusCode::BAD_REQUEST,
    };
    (
        status,
        tx_reject_json(tx, "preflight", &e, format!("tx cannot apply at tip: {e}")),
    )
}

pub(super) async fn v1_tx(
    State(a): State<App>,
    Json(tx): Json<SignedTx>,
) -> Result<StatusCode, (StatusCode, String)> {
    ensure_user_tx_allowed(&a).await?;
    enforce_recipient_prefilter(&tx)?;
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
                    return Err((e.status, e.message));
                }
            }
        }
        enforce_local_tx_guards(&tx, a.shard, a.identity.cluster_domain_hi)?;
    }
    if let TxBody::Import { export_id, .. } = &tx.body {
        info!(
            export_id = %hex::encode(export_id),
            "v1_tx: local import entering provenance prefilter"
        );
    }
    {
        let g = a.inner.read().await;
        enforce_import_provenance_prefilter(&tx, &g.chain.st, &g.cross_shard)?;
        enforce_recipient_init_gate(&tx, &g.chain.st)?;
    }
    if let TxBody::Import { export_id, .. } = &tx.body {
        info!(
            export_id = %hex::encode(export_id),
            "v1_tx: local import passed prefilter and recipient gate"
        );
    }
    validate_tx_shape(&tx).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            tx_reject_json(&tx, "preflight", &e, format!("tx validation failed: {e}")),
        )
    })?;
    let mut g = a.inner.write().await;
    let now_h = g.chain.tip_h();
    g.roaming_pool.expire_by_height(now_h);
    if g.roaming_pool.lock_conflict_for(&tx).is_some() {
        return Err((StatusCode::CONFLICT, ACTIVE_LOCK_ERR_TEXT.to_string()));
    }
    match &tx.body {
        TxBody::Export { .. } | TxBody::Import { .. } => {
            if matches!(tx.body, TxBody::Export { .. }) {
                let now_ms = crate::current_time_ms()?;
                let sender = tx.computed_account_id();
                let (_, sender_nonce) = acct_view(&g.chain.st, &sender);
                if let Err(reject) =
                    g.roaming_pool
                        .consume_readiness(&tx, now_ms, sender_nonce, now_h)
                {
                    push_readiness_reject_flow(&mut g, &tx, reject.code, now_h);
                    return Err((StatusCode::CONFLICT, readiness_reject_json(reject.code)));
                }
            }
            let bak = take_bak(&g);
            let sender = tx.computed_account_id();
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
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("seal after roaming tx failed: {msg}"),
                ));
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
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("tx commit rolled back: snapshot save failed: {e}"),
                    ));
                }
                drop(g);
                let mut st = a.init.write().await;
                *st = crate::state::InitState::ready(path);
                return Ok(StatusCode::NO_CONTENT);
            }
            return Ok(StatusCode::NO_CONTENT);
        }
        _ => {
            let (next_h, next_ts) = g.chain.next_apply_ctx().map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("precheck context resolve failed: {e}"),
                )
            })?;
            if let Err(e) = g.chain.st.precheck_apply_with_ctx(&tx, next_h, next_ts) {
                return Err(tx_tip_precheck_err(&tx, e));
            }
            g.pool.push(tx.clone()).map_err(|_| {
                (
                    StatusCode::INSUFFICIENT_STORAGE,
                    "mempool is full".to_string(),
                )
            })?;
            let h = g.chain.tip_h();
            push_tx_flow(
                &mut g,
                &tx,
                h,
                "accepted",
                Some("queued in mempool".to_string()),
            );
        }
    }
    let save_result = snap_save_locked(&a, &g);
    drop(g);
    persist_snap(&a, save_result, "after_tx").await?;
    Ok(StatusCode::NO_CONTENT)
}
