//! `/v1/status`, `/v1/head`, `/v1/flow/recent`.

use super::common::ensure_ready;
use super::types::{cross_shard_summary_out, FlowTraceOut, HeadOut, StatusOut};
use crate::relay::{GENESIS_FETCH_HINT, GENESIS_FETCH_STATUS, RELAY_MODE};
use crate::roaming::IntentStatus;
use crate::runtime_shard_label;
use crate::transport::{is_peer_liveish, trusted_relay_count};
use crate::App;
use axum::{extract::State, http::StatusCode, Json};

use super::common::{hex32, latest_readiness_reject};

pub(super) async fn v1_head(State(a): State<App>) -> Result<Json<HeadOut>, (StatusCode, String)> {
    ensure_ready(&a).await?;
    let g = a.inner.read().await;
    Ok(Json(HeadOut {
        height: g.chain.tip_h(),
        tip: hex32(&g.chain.tip_hash()),
    }))
}

pub(super) async fn v1_flow_recent(
    State(a): State<App>,
) -> Result<Json<FlowTraceOut>, (StatusCode, String)> {
    ensure_ready(&a).await?;
    let g = a.inner.read().await;
    let rows = g.recent_flow.iter().cloned().collect::<Vec<_>>();
    Ok(Json(FlowTraceOut { rows }))
}

pub(super) async fn v1_status(State(a): State<App>) -> Json<StatusOut> {
    let (phase, ready, snapshot_file, snapshot_error) = {
        let s = a.init.read().await;
        (
            s.phase.as_str(),
            s.is_ready(),
            s.snapshot_file.as_ref().map(|p| p.display().to_string()),
            s.snapshot_error.clone(),
        )
    };
    let (
        bridge_exported_registry_size,
        bridge_imported_set_size,
        bridge_registered_without_import,
        cross_shard_summary,
        roaming_intent_pool_size,
        roaming_active_locks_size,
        stuck_exported_without_finalize,
        stuck_relayed_without_import,
        oldest_stuck_age_blocks,
        last_readiness_reject,
    ) = {
        let g = a.inner.read().await;
        let now_h = g.chain.tip_h();
        let intents = g.roaming_pool.intents_snapshot();
        let mut exported = 0u64;
        let mut relayed = 0u64;
        let mut oldest: Option<u64> = None;
        for intent in &intents {
            let age = now_h.saturating_sub(intent.created_height);
            match intent.status {
                IntentStatus::Exported => {
                    exported += 1;
                    oldest = Some(oldest.map(|v| v.max(age)).unwrap_or(age));
                }
                IntentStatus::Relayed => {
                    relayed += 1;
                    oldest = Some(oldest.map(|v| v.max(age)).unwrap_or(age));
                }
                _ => {}
            }
        }
        let exported_registry_size = g.chain.st.exported_registry.len() as u64;
        let imported_set_size = g.chain.st.imported_set.len() as u64;
        (
            Some(exported_registry_size),
            Some(imported_set_size),
            exported_registry_size.saturating_sub(imported_set_size),
            cross_shard_summary_out(g.cross_shard.summary()),
            Some(intents.len() as u64),
            Some(g.roaming_pool.active_locks_snapshot().len() as u64),
            exported,
            relayed,
            oldest,
            latest_readiness_reject(&g),
        )
    };
    let (
        peer_seed_count,
        peer_listen,
        live_peer_count,
        trusted_relay_peer_count,
        peer_session_connected_total,
        peer_session_retrying_total,
        peer_session_disconnected_total,
        peer_session_untrusted_total,
        peer_session_trusted_total,
        next_seed_due_ms,
        last_peer_error,
        peer_error_at_ms,
    ) = {
        let cfg = a.transport_config.read().await;
        let hs = crate::transport::handshake_read_traced(&a, "api_status").await;
        let live = hs
            .peers
            .values()
            .filter(|p| is_peer_liveish(&p.status))
            .count() as u64;
        let trusted_relay = trusted_relay_count(&hs) as u64;
        let next_ms = hs
            .transport
            .seed_peers
            .values()
            .filter(|p| p.next_due_ms > 0)
            .map(|p| p.next_due_ms)
            .min();
        (
            cfg.peer_seeds.len() as u64,
            cfg.peer_listen.to_string(),
            live,
            trusted_relay,
            hs.transport.snapshot.session_connected_total,
            hs.transport.snapshot.session_retrying_total,
            hs.transport.snapshot.session_disconnected_total,
            hs.transport.snapshot.session_untrusted_total,
            hs.transport.snapshot.session_trusted_total,
            next_ms,
            hs.transport.snapshot.last_peer_error.clone(),
            hs.transport.snapshot.peer_error_at_ms,
        )
    };
    let peer_relay_health = if peer_seed_count == 0 {
        "not_configured"
    } else if trusted_relay_peer_count == 0 {
        "no_trusted_seed"
    } else {
        "ok"
    };
    let roaming_relay_hint = if peer_seed_count == 0 {
        "Configure --transport-peer-seed for one-window cross-shard relay; manual commands remain fallback."
            .to_string()
    } else if trusted_relay_peer_count == 0 {
        "Relay has no live trusted seed peer yet; check peer socket/session health and seed trust alignment."
            .to_string()
    } else {
        "Source RPC relays cross-shard handoff/import through live trusted seed peer context."
            .to_string()
    };
    let (
        effective_genesis_hash,
        genesis_guard,
        bridge_federation_trust,
        bridge_refusal_reason,
        genesis_mismatch_total,
        genesis_mismatch_expected_hash,
        genesis_mismatch_received_hash,
        genesis_mismatch_peer_id,
        genesis_mismatch_peer_hint,
        genesis_mismatch_unix_ms,
    ) = {
        let hs = crate::transport::handshake_read_traced(&a, "api_status").await;
        (
            hs.validation_ctx.expected_genesis_hash.clone(),
            if hs.genesis_guard.blocked {
                "blocked"
            } else {
                "ok"
            },
            if hs.bridge_trust.refused {
                "bridge_federation_trust_refused"
            } else {
                "ok"
            },
            hs.bridge_trust.refusal_reason.clone(),
            hs.genesis_guard.mismatch_total,
            hs.genesis_guard
                .last_mismatch
                .as_ref()
                .and_then(|x| x.expected_hash.clone()),
            hs.genesis_guard
                .last_mismatch
                .as_ref()
                .and_then(|x| x.received_hash.clone()),
            hs.genesis_guard
                .last_mismatch
                .as_ref()
                .map(|x| x.peer_node_id.clone()),
            hs.genesis_guard
                .last_mismatch
                .as_ref()
                .map(|x| x.peer_hint.clone()),
            hs.genesis_guard
                .last_mismatch
                .as_ref()
                .map(|x| x.at_unix_ms),
        )
    };
    let genesis_guard_recovery_hint = (genesis_guard == "blocked").then_some(
        "Genesis mismatch detected: stop node, fix genesis bundle/hash alignment, then restart to re-verify before user tx.",
    );
    let (
        lease_state,
        seal_gate_allowed,
        lease_owner_id,
        lease_term,
        lease_expires_at_ms,
        lease_last_tip,
        lease_fence,
        lease_last_reason,
    ) = match a.lease_runtime.lock() {
        Ok(v) => (
            match v.state {
                crate::lease::LeaseState::ActiveSealing => "active_sealing".to_string(),
                crate::lease::LeaseState::StandbySyncing => "standby_syncing".to_string(),
                crate::lease::LeaseState::SuspectActiveLost => "suspect_active_lost".to_string(),
                crate::lease::LeaseState::FencedStandby => "fenced_standby".to_string(),
            },
            v.allow_seal,
            v.owner_id.clone(),
            v.term,
            v.expires_at_ms,
            v.last_tip,
            v.fence,
            v.last_reason.clone(),
        ),
        Err(_) => (
            "fenced_standby".to_string(),
            false,
            "unknown".to_string(),
            0,
            0,
            0,
            0,
            "lease_runtime_poisoned".to_string(),
        ),
    };
    let lease_stats = a.lease_stats.snapshot();
    let lease_last_backend_error = a.lease_last_err.lock().ok().and_then(|v| (*v).clone());
    Json(StatusOut {
        phase,
        ready,
        shard: runtime_shard_label(&a.identity, a.shard),
        state_namespace: a.state_namespace.clone(),
        network_id: a.identity.network_id.clone(),
        snapshot_file,
        snapshot_error,
        cluster_domain_hi: a.identity.cluster_domain_hi,
        bridge_exported_registry_size,
        bridge_imported_set_size,
        bridge_registered_without_import,
        cross_shard_summary,
        roaming_intent_pool_size,
        roaming_active_locks_size,
        stuck_exported_without_finalize,
        stuck_relayed_without_import,
        oldest_stuck_age_blocks,
        roaming_relay_mode: RELAY_MODE,
        roaming_relay_hint,
        peer_seed_count,
        peer_listen,
        live_peer_count,
        trusted_relay_peer_count,
        peer_session_connected_total,
        peer_session_retrying_total,
        peer_session_disconnected_total,
        peer_session_untrusted_total,
        peer_session_trusted_total,
        peer_relay_health,
        next_seed_due_ms,
        last_peer_error,
        peer_error_at_ms,
        genesis_fetch_status: GENESIS_FETCH_STATUS,
        genesis_fetch_hint: GENESIS_FETCH_HINT,
        last_readiness_reject_code: last_readiness_reject.map(|x| x.as_str()),
        last_readiness_reject_hint: last_readiness_reject.map(|x| x.hint()),
        balance_semantics:
            "split:v1(local_state_balance,authoritative_home_balance,spendable_on_this_shard)",
        effective_genesis_hash,
        genesis_guard,
        bridge_federation_trust,
        bridge_refusal_reason,
        genesis_mismatch_total,
        genesis_mismatch_expected_hash,
        genesis_mismatch_received_hash,
        genesis_mismatch_peer_id,
        genesis_mismatch_peer_hint,
        genesis_mismatch_unix_ms,
        genesis_guard_recovery_hint,
        cluster_id: a.identity.cluster_id.clone(),
        node_id: a.identity.node_id.clone(),
        deployment_profile: match a.deployment_profile {
            crate::handshake::DeploymentProfile::SingleSealer => "single_sealer",
            crate::handshake::DeploymentProfile::MultiSealerExperimental => {
                "multi_sealer_experimental"
            }
        }
        .to_string(),
        seal_role: match a.seal_role {
            crate::handshake::SealRole::Active => "active",
            crate::handshake::SealRole::Standby => "standby",
        }
        .to_string(),
        lease_backend_mode: match a.lease_mode {
            crate::lease::LeaseBackendMode::File => "file",
            crate::lease::LeaseBackendMode::ProcessLocal => "process_local",
        }
        .to_string(),
        lease_backend_path: a.lease_path.as_ref().map(|v| v.display().to_string()),
        lease_last_backend_error,
        validator_identity_hash: a.validator_identity_hash.clone(),
        node_instance_id: a.node_instance_id.clone(),
        lease_state,
        seal_gate_allowed,
        lease_owner_id,
        lease_term,
        lease_expires_at_ms,
        lease_last_tip,
        lease_fence,
        lease_last_reason,
        lease_acquire_ok: lease_stats.acquire_ok,
        lease_renew_ok: lease_stats.renew_ok,
        lease_loss_total: lease_stats.loss_total,
        lease_reject_total: lease_stats.reject_total,
        lease_takeover_ok: lease_stats.takeover_ok,
    })
}

#[cfg(test)]
mod tests {
    use super::v1_status;
    use axum::extract::State;

    #[tokio::test]
    async fn status_exposes_identity_signals() {
        let mut app = crate::bootstrap::app_from_dev_net();
        app.deployment_profile = crate::handshake::DeploymentProfile::SingleSealer;
        app.seal_role = crate::handshake::SealRole::Standby;
        app.validator_identity_hash = "vh-status".to_string();
        app.node_instance_id = "inst-status".to_string();
        let out = v1_status(State(app)).await.0;
        assert_eq!(out.deployment_profile, "single_sealer");
        assert_eq!(out.seal_role, "standby");
        assert_eq!(out.lease_backend_mode, "process_local");
        assert_eq!(out.validator_identity_hash, "vh-status");
        assert_eq!(out.node_instance_id, "inst-status");
    }
}
