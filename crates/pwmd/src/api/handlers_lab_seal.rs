//! Lab-only manual seal RPC for owner-driven cluster debugging.

use super::common::ensure_ready;
use super::types::{
    SealControlIn, SealControlOut, SealGateOut, SealLeaseOut, SealRoundOut, SealStatusOut,
    SealStep, SealStepIn, SealStepOut, SealSyncOut,
};
use crate::lifecycle::{
    cluster_seal_preflight, count_sync_ready_attesters, run_cluster_gate, run_gate_obs,
    run_lease_gate, GateBlock,
};
use crate::transport::record_cluster_prop_tick;
use crate::App;
use axum::extract::{ConnectInfo, State};
use axum::http::StatusCode;
use axum::Json;
use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::time::sleep;
use tracing::info;

const MANUAL_VERBOSE_MS: u64 = 5_000;
const PROP_WAIT_MS: u64 = 10;

pub(super) async fn v1_lab_seal_status(
    State(app): State<App>,
    conn: Option<ConnectInfo<SocketAddr>>,
) -> Result<Json<SealStatusOut>, (StatusCode, String)> {
    ensure_ready(&app).await?;
    ensure_lab_seal_ok(&app, conn.map(|v| v.0))?;
    let now_ms = crate::current_time_ms()?;
    let att = count_sync_ready_attesters(&app).await;
    let tip_h = {
        let g = app.inner.read().await;
        g.chain.tip_h()
    };
    let manual = app.seal_manual.read().await.clone();
    let target_h = manual.target_h.max(tip_h.saturating_add(1));
    let sync_ready = SealSyncOut {
        sync_n: att.sync_n as u64,
        live_n: att.live_n as u64,
        peer_tip_max: att.peer_tip_max,
        max_lag: app.cluster_cfg.att_max_tip_lag,
    };
    let lease = lease_out(&app).await;
    let round = round_out(&app, target_h).await;
    Ok(Json(SealStatusOut {
        mode: manual.mode,
        tip_h,
        target_h,
        sync_ready,
        lease,
        round,
        last_step: manual.last_step,
        last_step_ms: (manual.step_t0_ms > 0).then_some(manual.step_t0_ms),
        verbose_active: manual.verbose_default || now_ms < manual.verbose_until_ms,
    }))
}

pub(super) async fn v1_lab_seal_control(
    State(app): State<App>,
    conn: Option<ConnectInfo<SocketAddr>>,
    Json(input): Json<SealControlIn>,
) -> Result<Json<SealControlOut>, (StatusCode, String)> {
    ensure_ready(&app).await?;
    ensure_lab_seal_ok(&app, conn.map(|v| v.0))?;
    let now_ms = crate::current_time_ms()?;
    {
        let mut manual = app.seal_manual.write().await;
        manual.mode = input.mode;
        manual.verbose_default = input.verbose_default.unwrap_or(false);
        manual.target_h = 0;
        manual.step_t0_ms = now_ms;
        manual.verbose_until_ms = if manual.verbose_default {
            now_ms.saturating_add(MANUAL_VERBOSE_MS)
        } else {
            0
        };
        manual.last_step = Some("control".to_string());
        manual.last_result = Some(format!("mode={:?}", manual.mode));
    }
    let status = status_snapshot(&app).await?;
    let verbose_default = {
        let manual = app.seal_manual.read().await;
        manual.verbose_default
    };
    Ok(Json(SealControlOut {
        mode: status.mode,
        verbose_default,
        tip_h: status.tip_h,
        target_h: status.target_h,
        verbose_active: status.verbose_active,
    }))
}

pub(super) async fn v1_lab_seal_step(
    State(app): State<App>,
    conn: Option<ConnectInfo<SocketAddr>>,
    Json(input): Json<SealStepIn>,
) -> Result<Json<SealStepOut>, (StatusCode, String)> {
    ensure_ready(&app).await?;
    ensure_lab_seal_ok(&app, conn.map(|v| v.0))?;
    let manual_mode = { app.seal_manual.read().await.mode };
    if !manual_mode.is_manual_rpc() {
        return Err((
            StatusCode::CONFLICT,
            "manual seal RPC is disabled; set mode=manual_rpc first".to_string(),
        ));
    }
    run_step(app, input).await
}

async fn run_step(app: App, input: SealStepIn) -> Result<Json<SealStepOut>, (StatusCode, String)> {
    match input.step {
        SealStep::StepAll => run_step_all(app, input).await,
        step => {
            let out = run_one_step(&app, step, &input).await?;
            Ok(Json(out))
        }
    }
}

async fn run_step_all(
    app: App,
    input: SealStepIn,
) -> Result<Json<SealStepOut>, (StatusCode, String)> {
    let mut warnings = Vec::new();
    let preflight = run_one_step(&app, SealStep::Preflight, &input).await?;
    if !preflight.ok {
        return Ok(Json(preflight));
    }
    warnings.extend(preflight.warnings);
    let lease = run_one_step(&app, SealStep::Lease, &input).await?;
    if !lease.ok {
        return Ok(Json(lease));
    }
    warnings.extend(lease.warnings);
    let propose = run_one_step(&app, SealStep::Propose, &input).await?;
    if !propose.ok {
        return Ok(Json(propose));
    }
    warnings.extend(propose.warnings);
    let gate = run_one_step(&app, SealStep::GateWait, &input).await?;
    if !gate.ok {
        return Ok(Json(gate));
    }
    warnings.extend(gate.warnings);
    let seal = run_one_step(&app, SealStep::SealCommit, &input).await?;
    warnings.extend(seal.warnings);
    Ok(Json(SealStepOut {
        ok: true,
        step: SealStep::StepAll,
        target_h: seal.target_h,
        tip_h_after: seal.tip_h_after,
        duration_ms: seal.duration_ms,
        detail: format!("step_all completed via {:?}", seal.step),
        warnings,
        gate: seal.gate,
        sync: seal.sync,
    }))
}

async fn run_one_step(
    app: &App,
    step: SealStep,
    input: &SealStepIn,
) -> Result<SealStepOut, (StatusCode, String)> {
    let now_ms = crate::current_time_ms()?;
    let verbose_default = {
        let manual = app.seal_manual.read().await;
        manual.verbose_default
    };
    let verbose = input.verbose.unwrap_or(verbose_default);
    let target_h = target_height(app, input.target_h).await;
    write_manual_meta(app, step, target_h, now_ms, verbose).await;
    let result = match step {
        SealStep::Preflight => step_preflight(app, target_h).await,
        SealStep::Lease => step_lease(app, target_h).await,
        SealStep::Propose => step_propose(app, target_h, verbose, input.timeout_ms).await,
        SealStep::GatePoll => step_gate_poll(app, target_h).await,
        SealStep::GateWait => step_gate_wait(app, target_h, input.timeout_ms).await,
        SealStep::SealCommit => step_seal_commit(app, target_h).await,
        SealStep::StepAll => unreachable!(),
    }?;
    update_manual_result(app, step, &result.detail, verbose).await;
    Ok(result)
}

async fn step_preflight(app: &App, target_h: u64) -> Result<SealStepOut, (StatusCode, String)> {
    let now_ms = crate::current_time_ms()?;
    let att = count_sync_ready_attesters(app).await;
    let preflight = cluster_seal_preflight(
        app.cluster_cfg.enabled,
        att.sync_n,
        app.cluster_cfg.quorum_k,
    );
    let ok = matches!(preflight, crate::lifecycle::SealPreflight::Ready);
    let detail = format!(
        "preflight={:?} live_synced_attesters={} live_connected_attesters={} peer_tip_max={}",
        preflight, att.sync_n, att.live_n, att.peer_tip_max
    );
    info!(
        target: "pwmd::operator",
        "manual_seal step=preflight target_h={} detail={}",
        target_h,
        detail
    );
    Ok(SealStepOut {
        ok,
        step: SealStep::Preflight,
        target_h,
        tip_h_after: current_tip(app).await,
        duration_ms: crate::current_time_ms()?.saturating_sub(now_ms),
        detail,
        warnings: Vec::new(),
        gate: None,
        sync: Some(SealSyncOut {
            sync_n: att.sync_n as u64,
            live_n: att.live_n as u64,
            peer_tip_max: att.peer_tip_max,
            max_lag: app.cluster_cfg.att_max_tip_lag,
        }),
    })
}

async fn step_lease(app: &App, target_h: u64) -> Result<SealStepOut, (StatusCode, String)> {
    let now_ms = crate::current_time_ms()?;
    let ok = run_lease_gate(app).await;
    let lease = lease_out(app).await;
    let detail = if ok {
        format!("lease=ok owner={} fence={}", lease.owner_id, lease.fence)
    } else {
        format!(
            "lease=fenced owner={} fence={}",
            lease.owner_id, lease.fence
        )
    };
    info!(target: "pwmd::operator", "manual_seal step=lease target_h={} detail={}", target_h, detail);
    Ok(SealStepOut {
        ok,
        step: SealStep::Lease,
        target_h,
        tip_h_after: current_tip(app).await,
        duration_ms: crate::current_time_ms()?.saturating_sub(now_ms),
        detail,
        warnings: Vec::new(),
        gate: None,
        sync: None,
    })
}

async fn step_propose(
    app: &App,
    target_h: u64,
    verbose: bool,
    timeout_ms: Option<u64>,
) -> Result<SealStepOut, (StatusCode, String)> {
    let start_ms = crate::current_time_ms()?;
    {
        let mut manual = app.seal_manual.write().await;
        manual.target_h = target_h;
        manual.step_t0_ms = start_ms;
        manual.verbose_until_ms = if verbose || manual.verbose_default {
            start_ms.saturating_add(MANUAL_VERBOSE_MS)
        } else {
            0
        };
    }
    app.cluster_prop_nudge.store(true, Ordering::Release);
    record_cluster_prop_tick(app).await;
    let timeout_ms = timeout_ms.unwrap_or(2_000);
    let deadline = start_ms.saturating_add(timeout_ms);
    loop {
        if opened_prop_ms(app, target_h).await.is_some() {
            break;
        }
        if crate::current_time_ms()? >= deadline {
            let detail = format!(
                "propose timeout waiting for wire send target_h={}",
                target_h
            );
            update_manual_result(app, SealStep::Propose, &detail, verbose).await;
            return Err((StatusCode::GATEWAY_TIMEOUT, detail));
        }
        sleep(Duration::from_millis(PROP_WAIT_MS)).await;
    }
    let opened_at_ms = opened_prop_ms(app, target_h).await.unwrap_or(start_ms);
    let detail = format!("propose wire_opened_at_ms={}", opened_at_ms);
    info!(target: "pwmd::operator", "manual_seal step=propose target_h={} detail={}", target_h, detail);
    Ok(SealStepOut {
        ok: true,
        step: SealStep::Propose,
        target_h,
        tip_h_after: current_tip(app).await,
        duration_ms: crate::current_time_ms()?.saturating_sub(start_ms),
        detail,
        warnings: Vec::new(),
        gate: None,
        sync: None,
    })
}

async fn step_gate_poll(app: &App, target_h: u64) -> Result<SealStepOut, (StatusCode, String)> {
    let now_ms = crate::current_time_ms()?;
    let gate_ok = run_cluster_gate(app, None).await;
    let obs = run_gate_obs(app).await;
    let detail = match obs {
        Some(GateBlock::Wait) => "gate_obs=Wait".to_string(),
        Some(GateBlock::Timeout) => "gate_obs=Timeout".to_string(),
        None => "gate_obs=Ready".to_string(),
    };
    let got = round_got(app, target_h).await;
    let gate = Some(SealGateOut {
        got,
        need: app.cluster_cfg.quorum_k,
        elapsed_ms: gate_elapsed(app, target_h).await,
        obs: Some(detail.clone()),
    });
    info!(target: "pwmd::operator", "manual_seal step=gate_poll target_h={} detail={}", target_h, detail);
    Ok(SealStepOut {
        ok: gate_ok,
        step: SealStep::GatePoll,
        target_h,
        tip_h_after: current_tip(app).await,
        duration_ms: crate::current_time_ms()?.saturating_sub(now_ms),
        detail,
        warnings: Vec::new(),
        gate,
        sync: None,
    })
}

async fn step_gate_wait(
    app: &App,
    target_h: u64,
    timeout_ms: Option<u64>,
) -> Result<SealStepOut, (StatusCode, String)> {
    let start_ms = crate::current_time_ms()?;
    let timeout_ms = timeout_ms.unwrap_or(app.cluster_cfg.attest_timeout_ms.saturating_add(500));
    let deadline = start_ms.saturating_add(timeout_ms);
    loop {
        let out = step_gate_poll(app, target_h).await?;
        if out.ok {
            return Ok(SealStepOut {
                step: SealStep::GateWait,
                duration_ms: crate::current_time_ms()?.saturating_sub(start_ms),
                ..out
            });
        }
        if crate::current_time_ms()? >= deadline {
            let detail = format!("gate_wait timeout_ms={} target_h={}", timeout_ms, target_h);
            update_manual_result(app, SealStep::GateWait, &detail, false).await;
            return Err((StatusCode::GATEWAY_TIMEOUT, detail));
        }
        sleep(Duration::from_millis(PROP_WAIT_MS)).await;
    }
}

async fn step_seal_commit(app: &App, target_h: u64) -> Result<SealStepOut, (StatusCode, String)> {
    let start_ms = crate::current_time_ms()?;
    if !run_cluster_gate(app, None).await {
        let detail = "seal_commit blocked by cluster gate".to_string();
        return Err((StatusCode::CONFLICT, detail));
    }
    let mut g = app.inner.write().await;
    let now_h = g.chain.tip_h();
    let expired = g.roaming_pool.expire_by_height(now_h);
    let txs = g.pool.take(64);
    let seal_result = g.chain.seal(txs);
    let tip_h_after = g.chain.tip_h();
    let detail = match seal_result {
        Ok(()) => {
            if expired > 0 {
                info!(target: "pwmd::operator", "manual_seal expired_roaming count={} height={}", expired, now_h);
            }
            info!(target: "pwmd::operator", "manual_seal step=seal_commit target_h={} sealed_h={}", target_h, tip_h_after);
            format!("sealed height={}", tip_h_after)
        }
        Err((err, _txs)) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("seal commit failed: {err}"),
            ));
        }
    };
    Ok(SealStepOut {
        ok: true,
        step: SealStep::SealCommit,
        target_h,
        tip_h_after,
        duration_ms: crate::current_time_ms()?.saturating_sub(start_ms),
        detail,
        warnings: Vec::new(),
        gate: None,
        sync: None,
    })
}

async fn current_tip(app: &App) -> u64 {
    let g = app.inner.read().await;
    g.chain.tip_h()
}

async fn target_height(app: &App, requested: Option<u64>) -> u64 {
    if let Some(target_h) = requested {
        return target_h;
    }
    current_tip(app).await.saturating_add(1)
}

async fn round_out(app: &App, target_h: u64) -> Option<SealRoundOut> {
    let hs = crate::transport::handshake_read_traced(app, "api_lab_seal").await;
    let round = hs.cluster_attest.rounds.get(&(target_h, 0))?;
    Some(SealRoundOut {
        height: target_h,
        round: 0,
        got: round.attesters.len() as u64,
        need: app.cluster_cfg.quorum_k,
        propose_opened_at_ms: round.propose_opened_at_ms,
    })
}

async fn round_got(app: &App, target_h: u64) -> u64 {
    round_out(app, target_h).await.map(|r| r.got).unwrap_or(0)
}

async fn gate_elapsed(app: &App, target_h: u64) -> Option<u64> {
    let hs = crate::transport::handshake_read_traced(app, "api_lab_seal").await;
    hs.cluster_attest
        .rounds
        .get(&(target_h, 0))
        .and_then(|round| round.propose_opened_at_ms)
        .map(|opened| {
            crate::current_time_ms()
                .unwrap_or(opened)
                .saturating_sub(opened)
        })
}

async fn opened_prop_ms(app: &App, target_h: u64) -> Option<u64> {
    let hs = crate::transport::handshake_read_traced(app, "api_lab_seal").await;
    hs.cluster_attest
        .rounds
        .get(&(target_h, 0))
        .and_then(|round| round.propose_opened_at_ms)
}

async fn lease_out(app: &App) -> SealLeaseOut {
    match app.lease_runtime.lock() {
        Ok(rt) => SealLeaseOut {
            state: match rt.state {
                crate::lease::LeaseState::ActiveSealing => "active_sealing".to_string(),
                crate::lease::LeaseState::StandbySyncing => "standby_syncing".to_string(),
                crate::lease::LeaseState::SuspectActiveLost => "suspect_active_lost".to_string(),
                crate::lease::LeaseState::FencedStandby => "fenced_standby".to_string(),
            },
            allow_seal: rt.allow_seal,
            owner_id: rt.owner_id.clone(),
            term: rt.term,
            expires_at_ms: rt.expires_at_ms,
            last_tip: rt.last_tip,
            fence: rt.fence,
            reason: rt.last_reason.clone(),
        },
        Err(_) => SealLeaseOut {
            state: "poisoned".to_string(),
            allow_seal: false,
            owner_id: String::new(),
            term: 0,
            expires_at_ms: 0,
            last_tip: 0,
            fence: 0,
            reason: "lease mutex poisoned".to_string(),
        },
    }
}

async fn status_snapshot(app: &App) -> Result<SealStatusOut, (StatusCode, String)> {
    let now_ms = crate::current_time_ms()?;
    let att = count_sync_ready_attesters(app).await;
    let tip_h = current_tip(app).await;
    let manual = app.seal_manual.read().await.clone();
    let target_h = manual.target_h.max(tip_h.saturating_add(1));
    let sync_ready = SealSyncOut {
        sync_n: att.sync_n as u64,
        live_n: att.live_n as u64,
        peer_tip_max: att.peer_tip_max,
        max_lag: app.cluster_cfg.att_max_tip_lag,
    };
    Ok(SealStatusOut {
        mode: manual.mode,
        tip_h,
        target_h,
        sync_ready,
        lease: lease_out(app).await,
        round: round_out(app, target_h).await,
        last_step: manual.last_step,
        last_step_ms: (manual.step_t0_ms > 0).then_some(manual.step_t0_ms),
        verbose_active: manual.verbose_default || now_ms < manual.verbose_until_ms,
    })
}

fn ensure_lab_seal_ok(app: &App, remote: Option<SocketAddr>) -> Result<(), (StatusCode, String)> {
    if !remote.is_some_and(|addr| addr.ip().is_loopback()) {
        return Err((
            StatusCode::CONFLICT,
            "lab seal RPC requires loopback access".to_string(),
        ));
    }
    if app.cluster_cfg.enabled {
        if !matches!(
            app.cluster_cfg.role,
            crate::handshake::ClusterRole::Proposer
        ) {
            return Err((
                StatusCode::CONFLICT,
                "lab seal RPC is only allowed on the cluster proposer".to_string(),
            ));
        }
    } else if !app.lab_seal_api {
        return Err((
            StatusCode::CONFLICT,
            "lab seal RPC is disabled; enable --lab-seal-api or cluster proposer mode".to_string(),
        ));
    }
    Ok(())
}

async fn write_manual_meta(app: &App, step: SealStep, target_h: u64, now_ms: u64, verbose: bool) {
    let mut manual = app.seal_manual.write().await;
    manual.target_h = target_h;
    manual.last_step = Some(step_name(step).to_string());
    manual.last_result = None;
    manual.step_t0_ms = now_ms;
    manual.verbose_until_ms = if verbose || manual.verbose_default {
        now_ms.saturating_add(MANUAL_VERBOSE_MS)
    } else {
        0
    };
}

async fn update_manual_result(app: &App, step: SealStep, result: &str, verbose: bool) {
    let mut manual = app.seal_manual.write().await;
    manual.last_step = Some(step_name(step).to_string());
    manual.last_result = Some(result.to_string());
    if verbose {
        manual.verbose_until_ms = manual.verbose_until_ms.max(
            crate::current_time_ms()
                .unwrap_or(0)
                .saturating_add(MANUAL_VERBOSE_MS),
        );
    }
}

fn step_name(step: SealStep) -> &'static str {
    match step {
        SealStep::Preflight => "preflight",
        SealStep::Lease => "lease",
        SealStep::Propose => "propose",
        SealStep::GatePoll => "gate_poll",
        SealStep::GateWait => "gate_wait",
        SealStep::SealCommit => "seal_commit",
        SealStep::StepAll => "step_all",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app_from_genesis, DevLane, GenesisSource};

    #[tokio::test]
    async fn status_returns_sync() {
        let app = app_from_genesis(&GenesisSource::DevNet, DevLane::Lane0).expect("app");
        {
            let mut manual = app.seal_manual.write().await;
            manual.mode = crate::SealControlMode::ManualRpc;
            manual.verbose_default = true;
        }
        let out = status_snapshot(&app).await.expect("status");
        assert_eq!(out.mode, crate::SealControlMode::ManualRpc);
        assert_eq!(out.target_h, out.tip_h.saturating_add(1));
        assert!(out.sync_ready.max_lag > 0);
    }

    #[tokio::test]
    async fn step_all_waiting_attester() {
        let mut app = app_from_genesis(&GenesisSource::DevNet, DevLane::Lane0).expect("app");
        app.cluster_cfg.enabled = true;
        app.cluster_cfg.role = crate::handshake::ClusterRole::Proposer;
        {
            let mut manual = app.seal_manual.write().await;
            manual.mode = crate::SealControlMode::ManualRpc;
        }
        let out = run_step_all(
            app,
            SealStepIn {
                step: SealStep::StepAll,
                verbose: Some(false),
                timeout_ms: Some(10),
                target_h: None,
            },
        )
        .await
        .expect("step_all");
        assert!(!out.0.ok);
        assert_eq!(out.0.step, SealStep::Preflight);
        assert!(out.0.sync.is_some());
    }
}
