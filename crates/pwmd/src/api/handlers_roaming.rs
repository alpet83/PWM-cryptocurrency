//! Roaming and export-readiness endpoints.

use super::common::{
    acct_view, ensure_bridge_federation_ok, ensure_ready, ensure_trusted_handoff_source,
    ensure_user_tx_allowed, handoff_from_intent_status, hex32, mark_relay_err, mark_relay_ok,
    parse_handoff_amount, parse_handoff_id, parse_id, persist_snap, push_readiness_reject_flow,
    push_tx_flow, readiness_reject_json, rollback_commit, snap_save_locked, take_bak, tx_id_hex,
    tx_kind, verify_handoff,
};
use super::types::{
    CreateRoamingIntentIn, CreateRoamingIntentOut, ExportReadinessIn, ExportReadinessOut,
    FinalizeRoamingIntentOut, IntentStatusOut, RegisterHandoffOut,
};
use crate::ledger::{summary_log_line, SUMMARY_BLOCK_INTERVAL};
use crate::relay::RELAY_MODE;
use crate::roaming::{
    IntentStatus, DEFAULT_INTENT_TTL_BLOCKS, DEFAULT_READINESS_TTL_SEC, MAX_READINESS_TTL_SEC,
};
use crate::App;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use pwm_core::hd::domain_of_account_id;
use pwm_core::tx::TxBody;
use pwm_core::{validate_recipient_address_policy, validate_tx_shape};
use tracing::{error, info, warn};

pub(super) async fn v1_export_readiness(
    State(a): State<App>,
    Json(input): Json<ExportReadinessIn>,
) -> Result<Json<ExportReadinessOut>, (StatusCode, String)> {
    ensure_user_tx_allowed(&a).await?;
    ensure_bridge_federation_ok(&a).await?;
    let tx = input.tx;
    if !matches!(tx.body, TxBody::Export { .. }) {
        return Err((
            StatusCode::BAD_REQUEST,
            "export readiness requires export tx body".to_string(),
        ));
    }
    crate::tx_policy::enforce_recipient_prefilter(&tx)?;
    if a.identity.mode.is_shard_enforced() {
        crate::tx_policy::enforce_local_tx_guards(&tx, a.shard, a.identity.cluster_domain_hi)?;
    }
    validate_tx_shape(&tx).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("tx validation failed: {e}"),
        )
    })?;
    let now_ms = crate::current_time_ms()?;
    let mut g = a.inner.write().await;
    let now_h = g.chain.tip_h();
    let sender = tx.computed_account_id();
    let (_, sender_nonce) = acct_view(&g.chain.st, &sender);
    let ttl_sec = input
        .ttl_sec
        .unwrap_or(DEFAULT_READINESS_TTL_SEC)
        .max(1)
        .min(MAX_READINESS_TTL_SEC);
    let ready = g
        .roaming_pool
        .register_readiness(&tx, now_ms, ttl_sec, sender_nonce, now_h)
        .map_err(|e| (StatusCode::CONFLICT, e.to_string()))?;
    push_tx_flow(
        &mut g,
        &tx,
        now_h,
        "checked:export_readiness",
        Some(format!(
            "code=ready; hint=submit export before ttl_ms={}",
            ready.expires_at_unix_ms.saturating_sub(now_ms)
        )),
    );
    Ok(Json(ExportReadinessOut {
        ready: true,
        export_id: hex32(&ready.export_id),
        expires_at_unix_ms: ready.expires_at_unix_ms,
        reason_code: "ready",
        recovery_hint: "Submit the exact EXPORT payload before TTL expires.",
    }))
}

pub(super) async fn v1_roaming_intent_create(
    State(a): State<App>,
    Json(input): Json<CreateRoamingIntentIn>,
) -> Result<(StatusCode, Json<CreateRoamingIntentOut>), (StatusCode, String)> {
    ensure_user_tx_allowed(&a).await?;
    ensure_bridge_federation_ok(&a).await?;
    let tx = input.tx;
    if !matches!(tx.body, TxBody::Export { .. }) {
        return Err((
            StatusCode::BAD_REQUEST,
            "roaming intent create requires export tx body".to_string(),
        ));
    }
    crate::tx_policy::enforce_recipient_prefilter(&tx)?;
    if a.identity.mode.is_shard_enforced() {
        crate::tx_policy::enforce_local_tx_guards(&tx, a.shard, a.identity.cluster_domain_hi)?;
    }
    validate_tx_shape(&tx).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("tx validation failed: {e}"),
        )
    })?;
    let mut g = a.inner.write().await;
    let now_h = g.chain.tip_h();
    g.roaming_pool.expire_by_height(now_h);
    let now_ms = crate::current_time_ms()?;
    let sender = tx.computed_account_id();
    let (_, sender_nonce) = acct_view(&g.chain.st, &sender);
    if let Err(reject) = g
        .roaming_pool
        .consume_readiness(&tx, now_ms, sender_nonce, now_h)
    {
        push_readiness_reject_flow(&mut g, &tx, reject.code, now_h);
        return Err((StatusCode::CONFLICT, readiness_reject_json(reject.code)));
    }
    let bak = take_bak(&g);
    let ttl = input.ttl_blocks.unwrap_or(DEFAULT_INTENT_TTL_BLOCKS);
    let (intent_id, duplicate) = g
        .roaming_pool
        .register_export(&tx, now_h, ttl)
        .map_err(|e| (StatusCode::CONFLICT, e.to_string()))?;
    if !duplicate {
        let sender = tx.computed_account_id();
        let (bal_before, nonce_before) = acct_view(&g.chain.st, &sender);
        if let Err((msg, _)) = g.chain.seal(vec![tx.clone()]) {
            rollback_commit(&mut g, bak);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("seal after roaming tx failed: {msg}"),
            ));
        }
        let h = g.chain.tip_h();
        g.record_cross_shard_tx(&tx, h);
        push_tx_flow(
            &mut g,
            &tx,
            h,
            "applied",
            Some("sealed with tx payload".to_string()),
        );
        g.roaming_pool.mark_exported(intent_id);
        push_tx_flow(
            &mut g,
            &tx,
            h,
            "exported",
            Some("export registry updated".to_string()),
        );
        push_tx_flow(
            &mut g,
            &tx,
            h,
            "roaming_status",
            Some("intent marked exported".to_string()),
        );
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
        push_tx_flow(&mut g, &tx, h, "sealed", None);
        if h > 0 && h % SUMMARY_BLOCK_INTERVAL == 0 {
            info!("{}", summary_log_line(&g.cross_shard.summary()));
        }
    }
    let handoff_to_relay = if !duplicate {
        g.roaming_pool
            .get(&intent_id)
            .cloned()
            .map(|intent| handoff_from_intent_status(&a, &intent, IntentStatus::Relayed))
    } else {
        None
    };
    let intent = g.roaming_pool.get(&intent_id).ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        "roaming intent disappeared after create".to_string(),
    ))?;
    let mut out = CreateRoamingIntentOut {
        intent_id: hex32(&intent.intent_id),
        export_id: hex32(&intent.export_id),
        status: intent.status,
        created_height: intent.created_height,
        expires_at_height: intent.expires_at_height,
        duplicate,
    };
    let h = g.chain.tip_h();
    push_tx_flow(
        &mut g,
        &tx,
        h,
        "accepted",
        Some(if duplicate {
            "duplicate export delivery; existing intent reused".to_string()
        } else {
            "roaming intent created".to_string()
        }),
    );
    let save_result = snap_save_locked(&a, &g);
    let mut ready_snap_path: Option<Option<std::path::PathBuf>> = None;
    if let Some((path, result)) = save_result {
        if let Err(e) = result {
            rollback_commit(&mut g, bak);
            error!(
                "snapshot save after_roaming_intent failed path={}: {}",
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
        ready_snap_path = Some(path);
    }
    drop(g);
    if let Some(path) = ready_snap_path {
        let mut st = a.init.write().await;
        *st = crate::state::InitState::ready(path);
    }
    if let Some(handoff) = handoff_to_relay {
        match crate::relay::relay_handoff(&a, &handoff).await {
            Ok(()) => {
                out.status =
                    mark_relay_ok(&a, intent_id, tx.export_id().unwrap_or([0u8; 32])).await?;
            }
            Err(e) => {
                crate::relay::log_relay_absence(
                    &a,
                    "handoff",
                    &e,
                    Some(handoff.export_id.as_str()),
                    Some(handoff.intent_id.as_str()),
                )
                .await;
                mark_relay_err(
                    &a,
                    intent_id,
                    tx.export_id().unwrap_or([0u8; 32]),
                    e.message,
                )
                .await?;
            }
        }
    }
    Ok((StatusCode::OK, Json(out)))
}

pub(super) async fn v1_roaming_intent_finalize(
    State(a): State<App>,
    Path(id): Path<String>,
) -> Result<Json<FinalizeRoamingIntentOut>, (StatusCode, String)> {
    ensure_user_tx_allowed(&a).await?;
    ensure_bridge_federation_ok(&a).await?;
    let key =
        parse_id(&id).map_err(|_| (StatusCode::BAD_REQUEST, "invalid intent id".to_string()))?;
    let mut g = a.inner.write().await;
    let h = g.chain.tip_h();
    g.roaming_pool.expire_by_height(h);
    let intent = g.roaming_pool.get(&key).ok_or((
        StatusCode::NOT_FOUND,
        "roaming intent not found".to_string(),
    ))?;
    let export_id = intent.export_id;
    let tx_id = hex::encode(export_id);
    let before = intent.status;
    let (should_relay, mut message) = match before {
        IntentStatus::Queued | IntentStatus::Exported => (
            true,
            "handoff generated; target provenance registration is pending".to_string(),
        ),
        IntentStatus::Relayed => (
            false,
            "intent already relayed; waiting for IMPORT on target shard".to_string(),
        ),
        IntentStatus::Imported => (
            false,
            "intent already imported on target shard; finalize is idempotent".to_string(),
        ),
        IntentStatus::Expired => (
            false,
            "intent expired before finalize; create a new roaming intent".to_string(),
        ),
        IntentStatus::Failed => (
            false,
            "intent is failed; inspect status.last_error before retrying".to_string(),
        ),
    };
    let mut status = g
        .roaming_pool
        .get(&key)
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "roaming intent disappeared during finalize".to_string(),
        ))?
        .status;
    let mut changed = false;
    let handoff_intent = g
        .roaming_pool
        .get(&key)
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "roaming intent disappeared before handoff export".to_string(),
        ))?
        .clone();
    let handoff = handoff_from_intent_status(&a, &handoff_intent, IntentStatus::Relayed);
    let bak = take_bak(&g);
    g.push_flow(crate::state::FlowTraceRow {
        at_height: h,
        kind: "finalized:roaming_intent".to_string(),
        tx_id,
        export_id: Some(hex32(&export_id)),
        intent_id: Some(hex32(&key)),
        note: Some(message.clone()),
    });
    let save_result = snap_save_locked(&a, &g);
    drop(g);
    if let Some((path, result)) = save_result {
        match result {
            Ok(()) => {
                let mut st = a.init.write().await;
                *st = crate::state::InitState::ready(path);
            }
            Err(e) => {
                error!(
                    path = %path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "-".into()),
                    err = %e,
                    "roaming finalize: snapshot save failed after push_flow; rolling back"
                );
                let mut g = a.inner.write().await;
                rollback_commit(&mut g, bak);
                drop(g);
                let mut st = a.init.write().await;
                *st = crate::state::InitState::ready_degraded(path.clone(), e.clone());
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("snapshot save failed: {e}"),
                ));
            }
        }
    }
    if should_relay {
        match crate::relay::relay_handoff(&a, &handoff).await {
            Ok(()) => {
                status = mark_relay_ok(&a, key, export_id).await?;
                changed = true;
                message = "intent finalized after peer relay delivered provenance to target peer"
                    .to_string();
            }
            Err(e) => {
                crate::relay::log_relay_absence(
                    &a,
                    "handoff",
                    &e,
                    Some(handoff.export_id.as_str()),
                    Some(handoff.intent_id.as_str()),
                )
                .await;
                message = format!("handoff generated; peer relay pending: {}", e.message);
                mark_relay_err(&a, key, export_id, e.message).await?;
            }
        }
    }
    Ok(Json(FinalizeRoamingIntentOut {
        intent_id: hex32(&key),
        export_id: hex32(&export_id),
        status,
        changed,
        message,
        handoff,
    }))
}

pub(super) async fn v1_export_handoff_register(
    State(a): State<App>,
    Json(input): Json<super::types::ExportHandoffOut>,
) -> Result<Json<RegisterHandoffOut>, (StatusCode, String)> {
    ensure_user_tx_allowed(&a).await?;
    info!(
        export_id = %input.export_id,
        intent_id = %input.intent_id,
        "handoff_register: begin"
    );
    if input.network_id != a.identity.network_id {
        warn!(
            export_id = %input.export_id,
            intent_id = %input.intent_id,
            "handoff_register: reject network_id mismatch"
        );
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "export handoff network_id does not match target node export_id={} intent_id={}",
                input.export_id, input.intent_id
            ),
        ));
    }
    verify_handoff(&input)?;
    ensure_trusted_handoff_source(&a, &input).await?;
    let export_id = parse_handoff_id("export_id", &input.export_id)?;
    let source = parse_handoff_id("source", &input.source)?;
    let to = parse_handoff_id("to", &input.to)?;
    let amount = parse_handoff_amount(&input.amount)?;
    validate_recipient_address_policy(&to).map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    if domain_of_account_id(&to) != input.target_domain {
        warn!(
            export_id = %input.export_id,
            intent_id = %input.intent_id,
            "handoff_register: reject recipient domain vs target_domain"
        );
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "export handoff recipient domain does not match target_domain export_id={} intent_id={}",
                input.export_id, input.intent_id
            ),
        ));
    }
    if a.identity.mode.is_shard_enforced()
        && input.target_domain.to_be_bytes()[0] != a.identity.cluster_domain_hi
    {
        warn!(
            export_id = %input.export_id,
            intent_id = %input.intent_id,
            "handoff_register: reject target_domain not local shard"
        );
        return Err((
            StatusCode::CONFLICT,
            format!(
                "export handoff target_domain does not belong to this target node export_id={} intent_id={}",
                input.export_id, input.intent_id
            ),
        ));
    }

    let mut g = a.inner.write().await;
    if g.chain.st.imported_set.contains(&export_id) {
        warn!(
            export_id = %input.export_id,
            intent_id = %input.intent_id,
            "handoff_register: reject already imported"
        );
        return Err((
            StatusCode::CONFLICT,
            format!(
                "export handoff already imported export_id={} intent_id={}",
                input.export_id, input.intent_id
            ),
        ));
    }
    let duplicate = match g
        .cross_shard
        .facts()
        .into_iter()
        .find(|fact| fact.export_id == export_id)
    {
        Some(existing)
            if existing.to == to
                && existing.amount == amount
                && existing.target_domain_hi == input.target_domain.to_be_bytes()[0]
                && existing.source == Some(source) =>
        {
            true
        }
        Some(_) => {
            warn!(
                export_id = %input.export_id,
                intent_id = %input.intent_id,
                "handoff_register: reject provenance conflict"
            );
            return Err((
                StatusCode::BAD_REQUEST,
                format!(
                    "export handoff conflicts with registered provenance export_id={} intent_id={}",
                    input.export_id, input.intent_id
                ),
            ));
        }
        None => false,
    };
    let bak = take_bak(&g);
    let h = g.chain.tip_h();
    let intent_id = parse_handoff_id("intent_id", &input.intent_id).ok();
    g.cross_shard.record_handoff(
        export_id,
        input.source_domain_hi,
        source,
        to,
        input.target_domain,
        amount,
        h,
        intent_id,
    );
    g.push_flow(crate::state::FlowTraceRow {
        at_height: h,
        kind: "registered:export_provenance".to_string(),
        tx_id: input.export_id.clone(),
        export_id: Some(input.export_id.clone()),
        intent_id: Some(input.intent_id.clone()),
        note: Some("operator handoff provenance registered".to_string()),
    });
    let save_result = snap_save_locked(&a, &g);
    drop(g);
    let Some((path, result)) = save_result else {
        if duplicate {
            info!(
                export_id = %input.export_id,
                intent_id = %input.intent_id,
                "handoff_register: ok duplicate provenance idempotent"
            );
        } else {
            info!(
                export_id = %input.export_id,
                intent_id = %input.intent_id,
                "handoff_register: ok registered"
            );
        }
        return Ok(Json(RegisterHandoffOut {
            export_id: input.export_id,
            registered: !duplicate,
            duplicate,
            import_provenance: super::types::ImportProvenanceOut {
                to: hex32(&to),
                target_domain: input.target_domain,
                amount,
            },
        }));
    };
    if let Err(e) = result {
        error!(
            path = %path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "-".into()),
            err = %e,
            "handoff_register: snapshot save failed after state mutation; rolling back"
        );
        let mut g = a.inner.write().await;
        rollback_commit(&mut g, bak);
        drop(g);
        let mut st = a.init.write().await;
        *st = crate::state::InitState::ready_degraded(path.clone(), e.clone());
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("state rolled back; snapshot save failed: {e}"),
        ));
    }
    {
        let mut st = a.init.write().await;
        *st = crate::state::InitState::ready(path);
    }
    if duplicate {
        info!(
            export_id = %input.export_id,
            intent_id = %input.intent_id,
            "handoff_register: ok duplicate provenance idempotent"
        );
    } else {
        info!(
            export_id = %input.export_id,
            intent_id = %input.intent_id,
            "handoff_register: ok registered"
        );
    }
    Ok(Json(RegisterHandoffOut {
        export_id: input.export_id,
        registered: !duplicate,
        duplicate,
        import_provenance: super::types::ImportProvenanceOut {
            to: hex32(&to),
            target_domain: input.target_domain,
            amount,
        },
    }))
}

pub(super) async fn v1_roaming_intent_status(
    State(a): State<App>,
    Path(id): Path<String>,
) -> Result<Json<IntentStatusOut>, (StatusCode, String)> {
    ensure_ready(&a).await?;
    let key =
        parse_id(&id).map_err(|_| (StatusCode::BAD_REQUEST, "invalid intent id".to_string()))?;
    let mut g = a.inner.write().await;
    let h = g.chain.tip_h();
    let expired = g.roaming_pool.expire_by_height(h);
    let intent = g
        .roaming_pool
        .get(&key)
        .ok_or((
            StatusCode::NOT_FOUND,
            "roaming intent not found".to_string(),
        ))?
        .clone();
    let save_result = if expired > 0 {
        snap_save_locked(&a, &g)
    } else {
        None
    };
    drop(g);
    if expired > 0 {
        persist_snap(&a, save_result, "after_roaming_expire").await?;
    }
    Ok(Json(IntentStatusOut {
        intent_id: hex32(&intent.intent_id),
        export_id: hex32(&intent.export_id),
        source: hex32(&intent.source),
        to: hex32(&intent.to),
        target_domain: intent.target_domain,
        amount: intent.amount,
        fee: intent.fee,
        status: intent.status,
        created_height: intent.created_height,
        expires_at_height: intent.expires_at_height,
        last_error: intent.last_error.clone(),
        relay_mode: RELAY_MODE,
        relay_hint: "Source RPC performs peer relay through configured --transport-peer-seed; manual handoff commands remain fallback.",
    }))
}
