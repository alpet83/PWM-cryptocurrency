//! Shared helpers for HTTP handlers.

use super::types::{AcctOut, ExportHandoffOut};
use crate::roaming::{IntentStatus, ReadinessCode};
use crate::snapshot::SnapshotBackend;
use crate::transport::is_peer_liveish;
use crate::App;
use axum::http::StatusCode;
use ed25519_dalek::{Signature, Signer, Verifier, VerifyingKey};
use pwm_core::hd::domain_of_account_id;
use pwm_core::tx::{TxBody, TxError};
use pwm_core::SignedTx;
use serde::Serialize;
use serde_json::json;
use tracing::error;

#[derive(Serialize)]
struct ReadinessRejectOut {
    code: &'static str,
    hint: &'static str,
    message: String,
}

pub(super) fn handoff_msg(
    network_id: &str,
    source_domain_hi: u8,
    source_cluster_id: &str,
    source_node_id: &str,
    intent_id: &str,
    export_id: &str,
    source: &str,
    to: &str,
    target_domain: u16,
    amount: &str,
    status: IntentStatus,
) -> Vec<u8> {
    format!(
        "pwm-export-handoff-v1|{network_id}|{source_domain_hi:02X}|{source_cluster_id}|{source_node_id}|{intent_id}|{export_id}|{source}|{to}|{target_domain}|{amount}|{}",
        status.as_str()
    )
    .into_bytes()
}

pub(super) fn handoff_from_intent_status(
    app: &App,
    intent: &crate::roaming::RoamingIntent,
    status: IntentStatus,
) -> ExportHandoffOut {
    let key = crate::transport::local_hello_signing_key(&app.identity);
    let source_node_pubkey = hex::encode(key.verifying_key().to_bytes());
    let intent_id = hex32(&intent.intent_id);
    let export_id = hex32(&intent.export_id);
    let source = hex32(&intent.source);
    let to = hex32(&intent.to);
    let amount = intent.amount.to_string();
    let msg = handoff_msg(
        &app.identity.network_id,
        app.identity.cluster_domain_hi,
        &app.identity.cluster_id,
        &app.identity.node_id,
        &intent_id,
        &export_id,
        &source,
        &to,
        intent.target_domain,
        &amount,
        status,
    );
    let signature = key.sign(&msg);
    ExportHandoffOut {
        proof_version: 1,
        network_id: app.identity.network_id.clone(),
        source_domain_hi: app.identity.cluster_domain_hi,
        source_cluster_id: app.identity.cluster_id.clone(),
        source_node_id: app.identity.node_id.clone(),
        source_node_pubkey,
        intent_id,
        export_id,
        source,
        to,
        target_domain: intent.target_domain,
        amount,
        status,
        signature: hex::encode(signature.to_bytes()),
    }
}

pub(super) fn parse_handoff_id(field: &str, value: &str) -> Result<[u8; 32], (StatusCode, String)> {
    parse_id(value).map_err(|_| (StatusCode::BAD_REQUEST, format!("invalid {field}")))
}

pub(super) fn parse_handoff_amount(value: &str) -> Result<u128, (StatusCode, String)> {
    value.trim().parse::<u128>().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "invalid handoff amount".to_string(),
        )
    })
}

pub(super) fn verify_handoff(input: &ExportHandoffOut) -> Result<(), (StatusCode, String)> {
    if input.proof_version != 1 {
        return Err((
            StatusCode::BAD_REQUEST,
            "invalid handoff proof_version".to_string(),
        ));
    }
    if input.status != IntentStatus::Relayed {
        return Err((
            StatusCode::BAD_REQUEST,
            "export handoff must be finalized with status=relayed".to_string(),
        ));
    }
    let pk_raw = hex::decode(input.source_node_pubkey.trim()).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "invalid handoff source_node_pubkey".to_string(),
        )
    })?;
    if pk_raw.len() != 32 {
        return Err((
            StatusCode::BAD_REQUEST,
            "invalid handoff source_node_pubkey length".to_string(),
        ));
    }
    let mut pk = [0u8; 32];
    pk.copy_from_slice(&pk_raw);
    let vk = VerifyingKey::from_bytes(&pk).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "invalid handoff source_node_pubkey".to_string(),
        )
    })?;
    let sig_raw = hex::decode(input.signature.trim()).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "invalid handoff signature".to_string(),
        )
    })?;
    if sig_raw.len() != 64 {
        return Err((
            StatusCode::BAD_REQUEST,
            "invalid handoff signature length".to_string(),
        ));
    }
    let mut sig = [0u8; 64];
    sig.copy_from_slice(&sig_raw);
    let sig = Signature::from_bytes(&sig);
    let msg = handoff_msg(
        &input.network_id,
        input.source_domain_hi,
        &input.source_cluster_id,
        &input.source_node_id,
        &input.intent_id,
        &input.export_id,
        &input.source,
        &input.to,
        input.target_domain,
        &input.amount,
        input.status,
    );
    vk.verify(&msg, &sig).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "invalid handoff signature".to_string(),
        )
    })
}

pub(super) async fn ensure_trusted_handoff_source(
    app: &App,
    input: &ExportHandoffOut,
) -> Result<(), (StatusCode, String)> {
    let pk_raw = hex::decode(input.source_node_pubkey.trim()).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            "invalid handoff source_node_pubkey".to_string(),
        )
    })?;
    if pk_raw.len() != 32 {
        return Err((
            StatusCode::BAD_REQUEST,
            "invalid handoff source_node_pubkey length".to_string(),
        ));
    }
    let mut pk = [0u8; 32];
    pk.copy_from_slice(&pk_raw);
    let hs = app.handshake.read().await;
    let Some(peer) = hs.trusted_peers.get(&input.source_node_id) else {
        return Err((
            StatusCode::FORBIDDEN,
            "export handoff source peer is not trusted by handshake state".to_string(),
        ));
    };
    if peer.node_id != input.source_node_id
        || peer.cluster_id != input.source_cluster_id
        || peer.domain_hi != input.source_domain_hi
        || peer.pubkey != pk
    {
        return Err((
            StatusCode::FORBIDDEN,
            "export handoff source identity does not match trusted peer state".to_string(),
        ));
    }
    let Some(live) = hs.peers.get(&input.source_node_id) else {
        return Err((
            StatusCode::FORBIDDEN,
            "export handoff source peer is not live".to_string(),
        ));
    };
    if !is_peer_liveish(&live.status) {
        return Err((
            StatusCode::FORBIDDEN,
            "export handoff source peer is not live".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn hex32(b: &[u8; 32]) -> String {
    hex::encode(b)
}

pub(super) fn parse_id(s: &str) -> Result<[u8; 32], ()> {
    let v = hex::decode(s.trim()).map_err(|_| ())?;
    if v.len() != 32 {
        return Err(());
    }
    let mut a = [0u8; 32];
    a.copy_from_slice(&v);
    Ok(a)
}

pub(super) fn tx_kind(tx: &SignedTx) -> &'static str {
    match tx.body {
        TxBody::Init { .. } => "init",
        TxBody::Transfer { .. } => "transfer",
        TxBody::Stake { .. } => "stake",
        TxBody::Unstake { .. } => "unstake",
        TxBody::BurnMark { .. } => "burn_mark",
        TxBody::Claim { .. } => "claim",
        TxBody::Export { .. } => "export",
        TxBody::Import { .. } => "import",
        TxBody::Policy { .. } => "policy",
    }
}

pub(crate) fn reject_tx_kind(tx: &SignedTx) -> &'static str {
    match tx.body {
        TxBody::BurnMark { .. } => "burn",
        TxBody::Claim { .. } => "claim",
        TxBody::Import { .. } => "import",
        TxBody::Export { .. } => "export",
        TxBody::Transfer { .. } => "transfer",
        TxBody::Init { .. } => "init",
        TxBody::Stake { .. } => "stake",
        TxBody::Unstake { .. } => "unstake",
        TxBody::Policy { .. } => "policy",
    }
}

pub(crate) fn tx_err_wire(e: &TxError, tx_kind: &str) -> (&'static str, &'static str) {
    use TxError::*;
    match e {
        // Burn stable errors (RFC 0014 baseline)
        InvalidPurposeLength | InvalidPurposeChars if tx_kind == "burn" => {
            ("E_BURN_SCHEMA_INVALID", "VALIDATION_ERROR")
        }
        InsufficientMarks if tx_kind == "burn" => ("E_BURN_OVER_BALANCE", "STATE_CONFLICT"),
        DomainMismatch if tx_kind == "burn" => ("E_BURN_POLICY_REJECT", "POLICY_REJECT"),

        // Claim stable errors (RFC 0013/0014 baseline)
        ClaimFeeModeConflict => ("E_MODE_FEE_CONFLICT", "POLICY_REJECT"),
        ClaimDeltaInvalid => ("E_CLAIM_UNITS_INVALID", "VALIDATION_ERROR"),
        ClaimAnchorRangeInvalid => ("E_ANCHOR_RANGE_INVALID", "STATE_CONFLICT"),
        ClaimAnchorContinuityBroken => ("E_ANCHOR_CONTINUITY_BROKEN", "STATE_CONFLICT"),
        ClaimOverMatured => ("E_CLAIM_OVER_MATURED", "STATE_CONFLICT"),
        FreeClaimDailyLimit => ("E_FREE_CLAIM_DAILY_LIMIT", "POLICY_REJECT"),

        // Import fee baseline.
        ImportFeeTooLow => ("E_IMPORT_FEE_TOO_LOW", "POLICY_REJECT"),

        PolicySchemaInvalid => ("E_POLICY_SCHEMA_INVALID", "VALIDATION_ERROR"),
        PolicyNotInstalled => ("E_POLICY_NOT_INSTALLED", "POLICY_REJECT"),
        PolicyNotActive => ("E_POLICY_NOT_ACTIVE", "POLICY_REJECT"),
        PolicyDenied => ("E_POLICY_DENIED", "POLICY_REJECT"),
        PolicySenderFiltered => ("E_POLICY_SENDER_FILTERED", "POLICY_REJECT"),
        PolicyRoutingDenied => ("E_POLICY_ROUTING_DENIED", "POLICY_REJECT"),
        PolicyMissingCosign => ("E_POLICY_MISSING_COSIGN", "POLICY_REJECT"),
        PolicyRescueRequired => ("E_POLICY_RESCUE_REQUIRED", "POLICY_REJECT"),
        PolicyEmergencyCosignRequired => ("E_POLICY_EMERGENCY_COSIGN_REQUIRED", "POLICY_REJECT"),
        PolicyAccountFinalized => ("E_POLICY_ACCOUNT_FINALIZED", "POLICY_REJECT"),
        PolicyIrreversible => ("E_POLICY_IRREVERSIBLE", "POLICY_REJECT"),

        // Keep fixed fallback for non-freeze, generic schema failures.
        _ => ("E_SCHEMA_INVALID", "VALIDATION_ERROR"),
    }
}

pub(crate) fn tx_reject_json(
    tx: &SignedTx,
    phase: &'static str,
    e: &TxError,
    message: String,
) -> String {
    let tx_kind = reject_tx_kind(tx);
    let trace_id = tx_id_hex(tx);
    let (code, response_class) = tx_err_wire(e, tx_kind);
    json!({
        "ok": false,
        "phase": phase,
        "tx_kind": tx_kind,
        "response_class": response_class,
        "error": {
            "code": code,
            "message": message,
            "trace_id": trace_id,
        },
    })
    .to_string()
}

pub(super) fn tx_id_hex(tx: &SignedTx) -> String {
    hex::encode(tx.tx_hash())
}

pub(super) fn push_readiness_reject_flow(
    g: &mut crate::state::Inner,
    tx: &SignedTx,
    code: ReadinessCode,
    now_h: u64,
) {
    push_tx_flow(
        g,
        tx,
        now_h,
        "rejected:export_readiness",
        Some(format!("code={}; hint={}", code.as_str(), code.hint())),
    );
}

pub(super) fn readiness_reject_json(code: ReadinessCode) -> String {
    serde_json::to_string(&ReadinessRejectOut {
        code: code.as_str(),
        hint: code.hint(),
        message: format!(
            "export readiness reject: code={}; hint={}",
            code.as_str(),
            code.hint()
        ),
    })
    .unwrap_or_else(|_| {
        format!(
            r#"{{"code":"{}","hint":"{}","message":"export readiness reject"}}"#,
            code.as_str(),
            code.hint()
        )
    })
}

pub(super) fn latest_readiness_reject(g: &crate::state::Inner) -> Option<ReadinessCode> {
    for row in g.recent_flow.iter().rev() {
        if !row.kind.starts_with("rejected:export_readiness") {
            continue;
        }
        let note = row.note.as_deref().unwrap_or_default();
        let Some((code_raw, _)) = note
            .split_once("code=")
            .and_then(|(_, tail)| tail.split_once("; hint="))
        else {
            continue;
        };
        return match code_raw {
            "missing_preflight" => Some(ReadinessCode::MissingPreflight),
            "stale_preflight" => Some(ReadinessCode::StalePreflight),
            "binding_mismatch" => Some(ReadinessCode::BindingMismatch),
            "nonce_mismatch" => Some(ReadinessCode::NonceMismatch),
            "height_mismatch" => Some(ReadinessCode::HeightMismatch),
            _ => None,
        };
    }
    None
}

pub(super) fn push_tx_flow(
    g: &mut crate::state::Inner,
    tx: &SignedTx,
    at_height: u64,
    kind: &str,
    note: Option<String>,
) {
    let export_id = tx.export_id().map(hex::encode);
    let intent_id = export_id.clone();
    g.push_flow(crate::state::FlowTraceRow {
        at_height,
        kind: format!("{kind}:{}", tx_kind(tx)),
        tx_id: tx_id_hex(tx),
        export_id,
        intent_id,
        note,
    });
}

/// Saves a snapshot while holding the inner state lock.
pub(super) fn snap_save_locked(
    app: &App,
    g: &crate::state::Inner,
) -> Option<(Option<std::path::PathBuf>, Result<(), String>)> {
    let backend = SnapshotBackend::from_data_file(app.data_file.as_ref())?;
    Some((backend.init_state_path(), backend.save(g)))
}

pub(crate) struct CommitBak {
    blocks: std::collections::VecDeque<pwm_core::block::Block>,
    canonical_h: u64,
    st: pwm_core::State,
    roaming_pool: crate::roaming::RoamingPool,
    cross_shard: crate::ledger::CrossShardLedger,
    recent_flow: std::collections::VecDeque<crate::state::FlowTraceRow>,
}

pub(crate) fn take_bak(g: &crate::state::Inner) -> CommitBak {
    CommitBak {
        blocks: g.chain.blocks.clone(),
        canonical_h: g.chain.tip_h(),
        st: g.chain.st.clone(),
        roaming_pool: g.roaming_pool.clone(),
        cross_shard: g.cross_shard.clone(),
        recent_flow: g.recent_flow.clone(),
    }
}

pub(crate) fn rollback_commit(g: &mut crate::state::Inner, bak: CommitBak) {
    g.chain.blocks = bak.blocks;
    g.chain.set_canon_h(bak.canonical_h);
    g.chain.st = bak.st;
    g.roaming_pool = bak.roaming_pool;
    g.cross_shard = bak.cross_shard;
    g.recent_flow = bak.recent_flow;
}

pub(super) fn acct_view(st: &pwm_core::State, id: &[u8; 32]) -> (u128, u64) {
    st.get(id)
        .map(|x| (x.balance_pwm, x.nonce))
        .unwrap_or((0, 0))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum HomeLookupState {
    Ok,
    NotFound,
    Stale,
    Unavailable,
}

pub(super) fn acct_out_for_runtime(
    acct_id: &[u8; 32],
    ac: &pwm_core::types::Account,
    local_domain_hi: u8,
    peer_view: Option<&crate::state::PeerAccountView>,
    home_lookup_state: HomeLookupState,
) -> AcctOut {
    let local_state_balance = ac.balance_pwm.to_string();
    let is_foreign = domain_of_account_id(acct_id).to_be_bytes()[0] != local_domain_hi;
    let legacy_balance = if is_foreign {
        "0".to_string()
    } else {
        local_state_balance.clone()
    };
    let (authoritative_home_balance, authoritative_home_initialized, home_lookup_status) =
        if is_foreign {
            match home_lookup_state {
                HomeLookupState::Ok => {
                    if let Some(view) = peer_view {
                        (
                            Some(view.balance_pwm.to_string()),
                            Some(view.initialized),
                            Some("ok"),
                        )
                    } else {
                        (None, None, Some("unavailable"))
                    }
                }
                HomeLookupState::NotFound => (None, None, Some("not_found")),
                HomeLookupState::Stale => (None, None, Some("stale")),
                HomeLookupState::Unavailable => (None, None, Some("unavailable")),
            }
        } else {
            (None, None, Some("local"))
        };
    AcctOut {
        id: hex32(acct_id),
        balance_pwm: legacy_balance,
        local_state_balance,
        authoritative_home_balance,
        authoritative_home_initialized,
        home_lookup_status,
        spendable_on_this_shard: (!is_foreign).then(|| ac.balance_pwm.to_string()),
        local_view_only: is_foreign,
        staked: ac.staked.to_string(),
        marks: ac.marks,
        initialized: ac.initialized,
        nonce: ac.nonce,
        rescue_address: ac.rescue_address.as_ref().map(hex::encode),
        active_policies: ac.active_policies,
        dormant_policies: ac.dormant_policies,
        finalized: ac.finalized,
        owner_kind: ac.owner_kind.clone(),
        owner_display_name: ac.owner_display_name.clone(),
        owner_country_hint: ac.owner_country_hint.clone(),
        company_metadata_commitment: ac.company_metadata_commitment.as_ref().map(hex::encode),
        external_verification_ref: ac.external_verification_ref.clone(),
        requested_domain_lo: ac.requested_domain_lo,
    }
}

fn account_fresh_window_ms(heartbeat_interval_ms: u64, heartbeat_timeout_ms: u64) -> u64 {
    heartbeat_timeout_ms
        .saturating_mul(2)
        .max(heartbeat_interval_ms.saturating_mul(4))
        .max(500)
}

pub(super) async fn foreign_home_lookup_state(
    a: &App,
    home_hi: u8,
    has_peer_view: bool,
    view_source_node_id: Option<&str>,
    now_ms: u64,
) -> HomeLookupState {
    let cfg = a.transport_config.read().await;
    let fresh_window_ms =
        account_fresh_window_ms(cfg.heartbeat_interval_ms, cfg.heartbeat_timeout_ms);
    drop(cfg);
    let hs = a.handshake.read().await;
    if hs.bridge_trust.refused {
        return HomeLookupState::Unavailable;
    }
    let live_trusted_nodes: Vec<&str> = hs
        .trusted_peers
        .iter()
        .filter_map(|(node_id, trusted)| {
            if trusted.domain_hi != home_hi {
                return None;
            }
            hs.peers
                .get(node_id)
                .filter(|p| is_peer_liveish(&p.status))
                .map(|_| node_id.as_str())
        })
        .collect();
    if live_trusted_nodes.is_empty() {
        return HomeLookupState::Unavailable;
    }
    let mut has_stale = false;
    let mut has_fresh = false;
    let mut source_fresh = false;
    for node_id in live_trusted_nodes {
        let Some(stream) = hs.trusted_account_streams.get(node_id) else {
            continue;
        };
        let is_fresh = now_ms.saturating_sub(stream.last_update_ms) <= fresh_window_ms;
        if is_fresh {
            has_fresh = true;
        } else {
            has_stale = true;
        }
        if Some(node_id) == view_source_node_id && is_fresh {
            source_fresh = true;
        }
    }
    if has_peer_view && source_fresh {
        HomeLookupState::Ok
    } else if has_fresh && !has_peer_view {
        HomeLookupState::NotFound
    } else if has_stale {
        HomeLookupState::Stale
    } else {
        HomeLookupState::Unavailable
    }
}

/// Persists the current snapshot; maps errors to HTTP 500.
pub(super) async fn persist_snap(
    app: &App,
    save_result: Option<(Option<std::path::PathBuf>, Result<(), String>)>,
    flow: &'static str,
) -> Result<(), (StatusCode, String)> {
    let Some((path, result)) = save_result else {
        return Ok(());
    };
    if let Err(e) = result {
        error!(
            "snapshot save {flow} failed path={}: {}",
            path.as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "-".into()),
            e
        );
        {
            let mut st = app.init.write().await;
            *st = crate::state::InitState::ready_degraded(path.clone(), e.clone());
        }
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("state persisted in memory but snapshot save failed: {e}"),
        ));
    }
    {
        let mut st = app.init.write().await;
        *st = crate::state::InitState::ready(path);
    }
    Ok(())
}

pub(super) async fn mark_relay_ok(
    app: &App,
    intent_id: [u8; 32],
    export_id: [u8; 32],
) -> Result<IntentStatus, (StatusCode, String)> {
    let mut g = app.inner.write().await;
    let h = g.chain.tip_h();
    let tx_id = hex32(&export_id);
    g.roaming_pool.mark_relayed(intent_id);
    g.push_flow(crate::state::FlowTraceRow {
        at_height: h,
        kind: "finalized:roaming_intent".to_string(),
        tx_id: tx_id.clone(),
        export_id: Some(tx_id.clone()),
        intent_id: Some(hex32(&intent_id)),
        note: Some(
            "intent finalized after peer relay delivered provenance to target peer".to_string(),
        ),
    });
    g.push_flow(crate::state::FlowTraceRow {
        at_height: h,
        kind: "roaming_status:relayed".to_string(),
        tx_id,
        export_id: Some(hex32(&export_id)),
        intent_id: Some(hex32(&intent_id)),
        note: Some("intent marked relayed after target provenance delivery".to_string()),
    });
    let status = g
        .roaming_pool
        .get(&intent_id)
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "roaming intent disappeared after relay".to_string(),
        ))?
        .status;
    let save_result = snap_save_locked(app, &g);
    drop(g);
    persist_snap(app, save_result, "after_roaming_relay").await?;
    Ok(status)
}

pub(super) async fn mark_relay_err(
    app: &App,
    intent_id: [u8; 32],
    export_id: [u8; 32],
    err: String,
) -> Result<(), (StatusCode, String)> {
    let mut g = app.inner.write().await;
    let h = g.chain.tip_h();
    g.roaming_pool.mark_relay_error(intent_id, err.clone());
    g.push_flow(crate::state::FlowTraceRow {
        at_height: h,
        kind: "relay_error:export_provenance".to_string(),
        tx_id: hex32(&export_id),
        export_id: Some(hex32(&export_id)),
        intent_id: Some(hex32(&intent_id)),
        note: Some(err),
    });
    let save_result = snap_save_locked(app, &g);
    drop(g);
    persist_snap(app, save_result, "after_roaming_relay_error").await
}

pub(super) async fn ensure_ready(app: &App) -> Result<(), (StatusCode, String)> {
    let s = app.init.read().await;
    if s.is_ready() {
        return Ok(());
    }
    Err((
        StatusCode::SERVICE_UNAVAILABLE,
        format!("node is not ready (phase={})", s.phase.as_str()),
    ))
}

pub(super) async fn ensure_user_tx_allowed(app: &App) -> Result<(), (StatusCode, String)> {
    ensure_ready(app).await?;
    {
        let s = app.init.read().await;
        if matches!(s.phase, crate::state::InitPhase::ReadyDegraded) {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                "user tx blocked: snapshot persistence is degraded (save/load failed); fix storage before submitting txs"
                    .into(),
            ));
        }
    }
    let hs = app.handshake.read().await;
    if hs.genesis_guard.blocked {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "user tx blocked: genesis/hash mismatch detected during shard join; check /v1/status genesis diagnostics and recover via stop -> fix bundle -> restart verify".to_string(),
        ));
    }
    Ok(())
}

pub(super) async fn ensure_bridge_federation_ok(app: &App) -> Result<(), (StatusCode, String)> {
    let hs = app.handshake.read().await;
    if hs.bridge_trust.refused {
        return Err((
            StatusCode::CONFLICT,
            hs.bridge_trust
                .refusal_reason
                .clone()
                .unwrap_or_else(|| "bridge_federation_trust_refused".to_string()),
        ));
    }
    Ok(())
}
