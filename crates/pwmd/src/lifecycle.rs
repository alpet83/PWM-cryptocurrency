//! Block ticks, mempool drains, autosnapshot triggers, and federation sweep hooks.
//! `spawn_snapshot_loader` logs `Instant`-based stages on target `pwmd::startup::snapshot`.

use crate::api::common::{rollback_commit, take_bak};
use crate::bootstrap::app_from_genesis_id;
use crate::config::PwmdConfig;
use crate::debug_dump::{align_mid_on, mid_wait_ms};
use crate::handshake::{ClusterRole, DeploymentProfile, SealRole};
use crate::lease::{step_lease, LeaseCfg, LeaseEvent, LeaseState};
use crate::ledger::{summary_log_line, SUMMARY_BLOCK_INTERVAL};
use crate::runtime_shard_label;
use crate::snapshot::{
    BlocksStored, SealPersistMode, SnapIoTiming, SnapshotBackend, SnapshotLoadOpts,
    SNAP_STARTUP_TARGET,
};
use crate::state::{App, InitState};
use crate::storage_namespace;
use crate::RuntimeIdentityMode;
use crate::{
    cors_for_listen, digest, federation::spawn_federation_sweep_loop, router,
    spawn_peer_listener_loop, spawn_stateful_transport_loop, spawn_transport_loop,
};
use pwm_core::absorb_blocks_tail;
use pwm_core::state::State;
use pwm_core::tx::{SignedTx, TxBody};
use pwm_core::SealTimeMode;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tracing::{error, info, warn};

pub const AUTOSNAPSHOT_BLOCK_INTERVAL: u64 = crate::snapshot::epoch::SNAP_CHK_BLK_IV;
/// Standby sync checkpoint interval (height 1 and then every N blocks).
pub const STANDBY_SYNC_FLUSH_IV: u64 = 100;

pub(crate) fn autosnap_hit(h: u64) -> bool {
    h > 0 && h % AUTOSNAPSHOT_BLOCK_INTERVAL == 0
}

fn snap_startup_mode(backend: &SnapshotBackend, stored: BlocksStored) -> &'static str {
    match backend {
        #[cfg(feature = "clickhouse-snapshot")]
        SnapshotBackend::ClickHouse(_) => "clickhouse",
        SnapshotBackend::JsonFile { .. } => match stored {
            BlocksStored::Epochs => "epochs",
            BlocksStored::Inline => "inline",
        },
    }
}

fn tx_kind(tx: &SignedTx) -> &'static str {
    match tx.body {
        TxBody::Init { .. } => "init",
        TxBody::Transfer { .. } => "transfer",
        TxBody::Stake { .. } => "stake",
        TxBody::Unstake { .. } => "unstake",
        TxBody::BurnMark { .. } => "burn_mark",
        TxBody::Claim { .. } => "claim",
        TxBody::Export { .. } => "export",
        TxBody::Import { .. } => "import",
    }
}

fn tx_addrs(tx: &SignedTx) -> Vec<[u8; 32]> {
    let signer = tx.computed_account_id();
    match tx.body {
        TxBody::Transfer { to, .. } | TxBody::Import { to, .. } | TxBody::Export { to, .. } => {
            vec![signer, to]
        }
        _ => vec![signer],
    }
}

fn tx_bal_diffs(before: &State, after: &State, tx: &SignedTx) -> Vec<([u8; 32], u128, u128)> {
    tx_addrs(tx)
        .into_iter()
        .filter_map(|acc| {
            let prev = before
                .accounts
                .get(&acc)
                .map(|v| v.balance_pwm)
                .unwrap_or(0);
            let next = after.accounts.get(&acc).map(|v| v.balance_pwm).unwrap_or(0);
            (prev != next).then_some((acc, prev, next))
        })
        .collect()
}

fn first_bad_tx_ctx(
    st: &State,
    txs: &[SignedTx],
    blk_h: u64,
    blk_ts: u64,
) -> Option<(usize, String)> {
    let mut sim = st.clone();
    for (i, tx) in txs.iter().enumerate() {
        if let Err(err) = sim.apply_tx_with_ctx(tx, blk_h, blk_ts) {
            return Some((i, err.to_string()));
        }
    }
    None
}

fn log_tx_debug(before: &State, after: &State, height: u64, txs: &[SignedTx]) {
    let log = crate::logger();
    for tx in txs {
        let tx_id = hex::encode(tx.tx_hash());
        for (acc, prev, next) in tx_bal_diffs(before, after, tx) {
            log.debug_tx(height, tx_kind(tx), &tx_id, &hex::encode(acc), prev, next);
        }
    }
}

fn log_tx_commit_delta(before: &State, after: &State, height: u64, txs: &[SignedTx]) {
    for tx in txs {
        let tx_id = hex::encode(tx.tx_hash());
        let sender = tx.computed_account_id();
        let (bal_before, nonce_before) = before
            .get(&sender)
            .map(|a| (a.balance_pwm, a.nonce))
            .unwrap_or((0, 0));
        let (bal_after, nonce_after) = after
            .get(&sender)
            .map(|a| (a.balance_pwm, a.nonce))
            .unwrap_or((0, 0));

        // Keep wording aligned with `/v1/tx`'s export/import commit logs.
        info!(
            "tx commit delta: kind={} tx_id={} sender={} bal:{}->{} nonce:{}->{}",
            tx_kind(tx),
            tx_id,
            hex::encode(sender),
            bal_before,
            bal_after,
            nonce_before,
            nonce_after
        );
    }
    let _ = height; // height is included by logs elsewhere; keep signature symmetry with debug.
}

fn runtime_mode_summary(mode: &RuntimeIdentityMode) -> String {
    match mode {
        RuntimeIdentityMode::Explicit => "shard_enforced(explicit-domain-config)".to_string(),
        RuntimeIdentityMode::Neutral => "relay_baseline(neutral-default)".to_string(),
    }
}

fn derive_seal_role(config: &PwmdConfig) -> SealRole {
    if let Some(role) = config.seal_role_override {
        return role;
    }
    if config.cluster.enabled && matches!(config.cluster.role, ClusterRole::Attester) {
        return SealRole::Standby;
    }
    if config.debug_disable_seal_loop {
        SealRole::Standby
    } else {
        SealRole::Active
    }
}

async fn req_graceful_stop(app: &App, height: u64, stop_h: u64) {
    if let Some(ref backend) = app.autosnapshot_backend {
        let save_res = {
            let inner = app.inner.read().await;
            backend.save_seal_persist(&inner, SealPersistMode::ShutdownFull)
        };
        if let Err(e) = save_res {
            error!(
                "debug-stop-height persist failed at height={} stop_h={}: {}",
                height, stop_h, e
            );
            return;
        }
    }
    if let Ok(mut slot) = app.shutdown_tx.lock() {
        if let Some(tx) = slot.take() {
            let _ = tx.send(());
            info!(
                "debug-stop-height reached; graceful shutdown triggered at height={} stop_h={}",
                height, stop_h
            );
        }
    }
}

async fn apply_snapshot_init_state(
    app: &App,
    path: Option<std::path::PathBuf>,
    result: Result<(), String>,
    height: u64,
) {
    match result {
        Ok(()) => {
            app.last_snapshot_height.store(height, Ordering::Release);
            let mut st = app.init.write().await;
            *st = InitState::ready(path);
        }
        Err(e) => {
            error!(
                "snapshot save after seal failed path={} height={}: {}",
                path.as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "-".into()),
                height,
                e
            );
            let mut st = app.init.write().await;
            *st = InitState::ready_degraded(path, e);
        }
    }
}

pub(crate) fn periodic_snap_save(
    backend: &SnapshotBackend,
    g: &crate::state::Inner,
    height: u64,
    source: &'static str,
) -> Option<(Option<std::path::PathBuf>, Result<(), String>)> {
    if !autosnap_hit(height) {
        return None;
    }
    info!(
        "autosnapshot checkpoint hit source={} interval={} height={}",
        source, AUTOSNAPSHOT_BLOCK_INTERVAL, height
    );
    Some((
        backend.init_state_path(),
        backend.save_seal_persist(g, SealPersistMode::Periodic),
    ))
}

pub(crate) async fn periodic_snap_finish(
    app: &App,
    height: u64,
    bak_opt: Option<crate::api::common::CommitBak>,
    save_result: Option<(Option<std::path::PathBuf>, Result<(), String>)>,
) {
    let Some((path, result)) = save_result else {
        return;
    };
    match result {
        Ok(()) => {
            apply_snapshot_init_state(app, path, Ok(()), height).await;
        }
        Err(e) => {
            if let Some(bak) = bak_opt {
                let mut g = app.inner.write().await;
                rollback_commit(&mut g, bak);
                drop(g);
            }
            apply_snapshot_init_state(app, path, Err(e), height).await;
        }
    }
}

fn now_unix_ms() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(v) => v.as_millis() as u64,
        Err(_) => 0,
    }
}

async fn maybe_align_mid(app: &App) {
    if !app.debug_align_mid {
        return;
    }
    let wait_ms = mid_wait_ms(now_unix_ms());
    if wait_ms == 0 {
        return;
    }
    tokio::time::sleep(std::time::Duration::from_millis(wait_ms)).await;
}

async fn run_lease_gate(app: &App) -> bool {
    if app.deployment_profile != DeploymentProfile::SingleSealer {
        return true;
    }
    let (tip_h, now_ms) = {
        let g = app.inner.read().await;
        (g.chain.tip_h(), now_unix_ms())
    };
    let mut rt = match app.lease_runtime.lock() {
        Ok(v) => v,
        Err(_) => {
            warn!("seal_suppressed_by_fence reason=lease_runtime_poisoned");
            return false;
        }
    };
    let step = step_lease(
        &app.validator_identity_hash,
        &app.node_instance_id,
        now_ms,
        tip_h,
        app.lease_cfg,
        &mut rt,
        app.lease_backend.as_ref(),
    );
    if let Ok(mut slot) = app.lease_last_err.lock() {
        if rt.last_reason.starts_with("lease_backend_error ") {
            *slot = Some(rt.last_reason.clone());
        } else if step.allow_seal {
            *slot = None;
        }
    }
    if let Some(ev) = step.event {
        app.lease_stats.on_event(ev);
        match ev {
            LeaseEvent::Acquire => {
                info!(
                    "seal_lease_acquired owner={} term={} fence={} expires_at_ms={} tip_h={}",
                    rt.owner_id, rt.term, rt.fence, rt.expires_at_ms, rt.last_tip
                );
            }
            LeaseEvent::Renew => {
                let h = rt.last_tip;
                // Cadence matches `sealed height={}` (first block + every 10) — renewals are per-block noise otherwise.
                if h == 1 || h % 10 == 0 {
                    info!(
                        "seal_lease_renewed owner={} term={} fence={} expires_at_ms={} tip_h={}",
                        rt.owner_id, rt.term, rt.fence, rt.expires_at_ms, h
                    );
                }
            }
            LeaseEvent::Takeover => {
                info!(
                    "seal_takeover_committed owner={} term={} fence={} expires_at_ms={} tip_h={}",
                    rt.owner_id, rt.term, rt.fence, rt.expires_at_ms, rt.last_tip
                );
            }
            LeaseEvent::Loss => {
                warn!("seal_lease_lost reason={}", rt.last_reason);
            }
            LeaseEvent::Reject => {
                if rt.last_reason.contains("cas_miss") {
                    warn!("seal_lease_cas_failed reason={}", rt.last_reason);
                }
                info!("seal_suppressed_by_fence reason={}", rt.last_reason);
            }
        }
    }
    step.allow_seal
}

pub(crate) async fn run_cluster_gate(app: &App) -> bool {
    if !app.cluster_cfg.enabled {
        return true;
    }
    let next_h = {
        let g = app.inner.read().await;
        g.chain.tip_h().saturating_add(1)
    };
    let round = 0u32;
    let hs = app.handshake.read().await;
    let Some(state) = hs.cluster_attest.rounds.get(&(next_h, round)) else {
        warn!(
            "seal_suppressed_by_cluster reason=quorum_pending detail=missing_round_state height={} round={} k={} n={}",
            next_h,
            round,
            app.cluster_cfg.quorum_k,
            app.cluster_cfg.quorum_n
        );
        return false;
    };
    let vote_ok = !state.vote_object.trim().is_empty();
    let cand_ok = !state.candidate_hash.trim().is_empty();
    if !vote_ok || !cand_ok {
        warn!(
            "seal_suppressed_by_cluster reason=invalid_proposal detail=binding_incomplete height={} round={} vote_object_present={} candidate_hash_present={}",
            next_h,
            round,
            vote_ok,
            cand_ok
        );
        return false;
    }
    let proposer_ok = state
        .proposer_id
        .as_deref()
        .is_some_and(|id| app.cluster_cfg.members.iter().any(|m| m == id));
    if !proposer_ok {
        warn!(
            "seal_suppressed_by_cluster reason=invalid_proposal detail=proposer_not_member height={} round={}",
            next_h, round
        );
        return false;
    }
    // RFC16 §7: `k` counts distinct attester ACKs and excludes the proposer.
    let proposer = state.proposer_id.as_deref().unwrap_or_default();
    let ack_n = state
        .attesters
        .keys()
        .filter(|id| {
            let member = id.as_str();
            member != proposer && app.cluster_cfg.members.iter().any(|m| m == member)
        })
        .count() as u8;
    if ack_n < app.cluster_cfg.quorum_k {
        let now_ms = crate::current_time_ms().unwrap_or(0);
        if let Some(t0) = state.propose_opened_at_ms {
            if now_ms.saturating_sub(t0) > app.cluster_cfg.attest_timeout_ms {
                warn!(
                    "seal_suppressed_by_cluster reason=quorum_timeout detail=attestations_missing height={} round={} got={} need={} elapsed_ms={} limit_ms={}",
                    next_h,
                    round,
                    ack_n,
                    app.cluster_cfg.quorum_k,
                    now_ms.saturating_sub(t0),
                    app.cluster_cfg.attest_timeout_ms
                );
                return false;
            }
        }
        warn!(
            "seal_suppressed_by_cluster reason=quorum_pending detail=attestations_missing height={} round={} got={} need={}",
            next_h,
            round,
            ack_n,
            app.cluster_cfg.quorum_k
        );
        return false;
    }
    true
}

pub fn spawn_seal_loop(app: App) {
    tokio::spawn(async move {
        let mut iv = tokio::time::interval(std::time::Duration::from_secs(2));
        loop {
            iv.tick().await;
            if !app.init.read().await.allows_chain_progress() {
                continue;
            }
            if app.debug_disable_seal_loop {
                // Follower / replay-only mode: chain height may advance via sync apply, not local seal.
                // Still honor debug-stop-height so harnesses (e.g. Wave A) can shut down cleanly.
                let h = {
                    let g = app.inner.read().await;
                    g.chain.tip_h()
                };
                if let Some(stop_h) = app.debug_stop_height {
                    if h >= stop_h {
                        req_graceful_stop(&app, h, stop_h).await;
                    }
                }
                continue;
            }
            if app.cluster_cfg.enabled && matches!(app.cluster_cfg.role, ClusterRole::Attester) {
                // RFC16: attester is non-committer; no local seal/cluster gate polling.
                let h = {
                    let g = app.inner.read().await;
                    g.chain.tip_h()
                };
                if let Some(stop_h) = app.debug_stop_height {
                    if h >= stop_h {
                        req_graceful_stop(&app, h, stop_h).await;
                    }
                }
                continue;
            }
            if !run_lease_gate(&app).await {
                let h = {
                    let g = app.inner.read().await;
                    g.chain.tip_h()
                };
                if let Some(stop_h) = app.debug_stop_height {
                    if h >= stop_h {
                        req_graceful_stop(&app, h, stop_h).await;
                    }
                }
                continue;
            }
            if !run_cluster_gate(&app).await {
                let h = {
                    let g = app.inner.read().await;
                    g.chain.tip_h()
                };
                if let Some(stop_h) = app.debug_stop_height {
                    if h >= stop_h {
                        req_graceful_stop(&app, h, stop_h).await;
                    }
                }
                continue;
            }
            maybe_align_mid(&app).await;
            let mut g = app.inner.write().await;
            let now_h = g.chain.tip_h();
            let expired = g.roaming_pool.expire_by_height(now_h);
            if expired > 0 {
                info!("expired roaming intents count={} height={}", expired, now_h);
            }
            let txs = g.pool.take(64);
            let st_before = g.chain.st.clone();
            let persist_back = app.autosnapshot_backend.is_some() && autosnap_hit(now_h + 1);
            let bak_opt = persist_back.then(|| take_bak(&g));
            match g.chain.seal(txs) {
                Ok(()) => {
                    let h = g.chain.tip_h();
                    let stop_h = app.debug_stop_height;
                    if h == 1 || h % 10 == 0 {
                        info!("sealed height={}", h);
                    }
                    if let Some(blk) = g.chain.blocks.back() {
                        let txs = blk.txs.clone();
                        log_tx_debug(&st_before, &g.chain.st, h, &txs);
                        log_tx_commit_delta(&st_before, &g.chain.st, h, &txs);
                        for tx in &txs {
                            g.record_cross_shard_tx(tx, h);
                        }
                    }
                    if h > 0 && h % SUMMARY_BLOCK_INTERVAL == 0 {
                        info!("{}", summary_log_line(&g.cross_shard.summary()));
                    }
                    let save_result = app
                        .autosnapshot_backend
                        .as_ref()
                        .and_then(|backend| periodic_snap_save(backend, &g, h, "seal"));
                    drop(g);
                    periodic_snap_finish(&app, h, bak_opt, save_result).await;
                    if let Some(stop_h) = stop_h {
                        if h >= stop_h {
                            req_graceful_stop(&app, h, stop_h).await;
                        }
                    }
                }
                Err((e, txs)) => {
                    let replay = if e.starts_with("tx: ") {
                        let (blk_h, blk_ts) = match g.chain.next_apply_ctx() {
                            Ok(ctx) => ctx,
                            Err(err) => {
                                warn!("seal skip: failed to resolve apply ctx: {}", err);
                                g.pool.prepend_block(txs);
                                continue;
                            }
                        };
                        let drop_at = first_bad_tx_ctx(&g.chain.st, &txs, blk_h, blk_ts);
                        if let Some((i, err)) = drop_at {
                            warn!(
                                "seal skip: evicting unapplicable tx at index {} ({}), requeueing {} others",
                                i,
                                err,
                                txs.len().saturating_sub(1)
                            );
                            let mut kept = Vec::with_capacity(txs.len().saturating_sub(1));
                            kept.extend(txs[..i].iter().cloned());
                            kept.extend(txs[i + 1..].iter().cloned());
                            kept
                        } else {
                            warn!(
                                "seal skip: {} (could not locate failing tx; requeue full batch)",
                                e
                            );
                            txs
                        }
                    } else {
                        warn!("seal skip: {}", e);
                        txs
                    };
                    g.pool.prepend_block(replay);
                }
            }
        }
    });
}

fn shutdown_if_fatal_snapshot(app: &App, stage: &'static str) {
    if !app.exit_on_fatal_snapshot {
        return;
    }
    error!(
        target: SNAP_STARTUP_TARGET,
        stage,
        "pwmd exiting after unrecoverable snapshot error (PWM_KEEP_ALIVE_ON_SNAPSHOT_ERROR=1 or --keep-alive-on-snapshot-error keeps degraded HTTP)"
    );
    std::process::exit(1);
}

pub(crate) fn spawn_snapshot_loader(app: App) {
    tokio::spawn(async move {
        let Some(backend) = app.autosnapshot_backend.clone() else {
            let mut st = app.init.write().await;
            *st = InitState::ready(app.data_file.clone());
            info!("autosnapshot persistence disabled; using genesis-only chain state");
            crate::logger().info("pwmd startup phase: ready (snapshot persistence disabled)");
            return;
        };
        let loader_start = Instant::now();
        let diag = backend.diag_label();
        let snap_path = backend.init_state_path();
        {
            let mut st = app.init.write().await;
            *st = InitState::loading(snap_path.clone());
        }
        info!("snapshot loading started: {}", diag);
        crate::logger().info(&format!("pwmd startup phase: loading_snapshot ({})", diag));
        let cfg = {
            let g = app.inner.read().await;
            g.chain.cfg.clone()
        };
        let opts = SnapshotLoadOpts {
            verify_chain: app.snapshot_verify_chain,
        };
        match backend.load(&cfg, opts) {
            Ok((Some(snap), io_ph)) => {
                let stored = snap.blocks_stored;
                let mode_str = snap_startup_mode(&backend, stored);
                let snap_chk_h = snap.checkpoint_height;
                let t_ir = Instant::now();
                let (blocks, state, roaming_pool, cross_shard) = match snap.into_runtime() {
                    Ok(v) => v,
                    Err(e) => {
                        let g0_digest = hex::encode(digest(&cfg.state0()));
                        let elapsed_ms = loader_start.elapsed().as_millis() as u64;
                        warn!(
                            genesis_state0_digest = %g0_digest,
                            err = %e,
                            "snapshot into_runtime failed; compare genesis params and pwmd build with snapshot writer"
                        );
                        error!(
                            target: SNAP_STARTUP_TARGET,
                            stage = "into_runtime",
                            elapsed_ms,
                            err = %e,
                            path = %diag,
                            mode = mode_str,
                            "snapshot startup degraded"
                        );
                        let mut st = app.init.write().await;
                        *st = InitState::ready_degraded(snap_path.clone(), e.clone());
                        crate::logger().error(&format!(
                            "pwmd startup phase: ready_degraded (snapshot error: {e})"
                        ));
                        shutdown_if_fatal_snapshot(&app, "into_runtime");
                        return;
                    }
                };
                let into_runtime_ms = t_ir.elapsed().as_millis() as u64;
                let tip_h = blocks.last().map(|b| b.hdr.height).unwrap_or(0);
                let reg_n = state.exported_registry.len();
                let imp_n = state.imported_set.len();
                let t_abs = Instant::now();
                let mut g = app.inner.write().await;
                g.chain.blocks = absorb_blocks_tail(blocks);
                g.chain.st = state;
                g.chain.sync_canon_h();
                g.roaming_pool = roaming_pool;
                g.cross_shard = cross_shard;
                drop(g);
                let absorb_tail_ms = t_abs.elapsed().as_millis() as u64;
                let mut st = app.init.write().await;
                *st = InitState::ready(snap_path.clone());
                app.last_snapshot_height.store(tip_h, Ordering::Release);
                let total_ms = loader_start.elapsed().as_millis() as u64;
                let (summary_read_ms, epochs_ms, validate_ms, ch_http_ms, ch_parse_ms, ch_branch) =
                    match &io_ph {
                        SnapIoTiming::Json(j) => (
                            j.summary_read_ms,
                            j.epochs_ms,
                            j.validate_ms,
                            0u64,
                            0u64,
                            "",
                        ),
                        #[cfg(feature = "clickhouse-snapshot")]
                        SnapIoTiming::Ch(c) => (0u64, 0u64, 0u64, c.http_ms, c.parse_ms, c.branch),
                    };
                info!(
                    target: SNAP_STARTUP_TARGET,
                    path = %diag,
                    mode = mode_str,
                    tip_h,
                    canonical_h = snap_chk_h,
                    total_ms,
                    summary_read_ms,
                    epochs_ms,
                    validate_ms,
                    into_runtime_ms,
                    absorb_tail_ms,
                    ch_http_ms,
                    ch_parse_ms,
                    ch_branch,
                    "snapshot startup load ok"
                );
                info!("snapshot loaded from {}", diag);
                let g0_digest = hex::encode(digest(&cfg.state0()));
                info!(
                    tip_h = tip_h,
                    bridge_exported_registry = reg_n,
                    bridge_imported_set = imp_n,
                    genesis_state0_digest = %g0_digest,
                    "snapshot load: cross-shard bridge counters after apply"
                );
                if reg_n > imp_n {
                    warn!(
                        tip_h = tip_h,
                        pending_registered_minus_imported = reg_n.saturating_sub(imp_n),
                        "snapshot load: exported_registry exceeds imported_set; pending imports may need target relay (see handoff_register and v1_tx import logs)"
                    );
                }
                crate::logger().info("pwmd startup phase: ready (snapshot loaded)");
            }
            Ok((None, _io_ph)) => {
                let mut st = app.init.write().await;
                *st = InitState::ready(snap_path.clone());
                app.last_snapshot_height.store(0, Ordering::Release);
                let total_ms = loader_start.elapsed().as_millis() as u64;
                info!(
                    target: SNAP_STARTUP_TARGET,
                    path = %diag,
                    mode = "empty",
                    total_ms,
                    "snapshot startup: no snapshot row or file"
                );
                info!("snapshot store empty or missing row, fallback to genesis state");
                crate::logger()
                    .info("pwmd startup phase: ready (no snapshot row / file for current backend)");
            }
            Err(e) => {
                let g0_digest = hex::encode(digest(&cfg.state0()));
                let elapsed_ms = loader_start.elapsed().as_millis() as u64;
                warn!(
                    genesis_state0_digest = %g0_digest,
                    err = %e,
                    "snapshot load failed (fallback to genesis state); if state_root mismatch, align genesis params and binary with writer"
                );
                error!(
                    target: SNAP_STARTUP_TARGET,
                    stage = "backend_load",
                    elapsed_ms,
                    err = %e,
                    path = %diag,
                    "snapshot startup degraded"
                );
                let mut st = app.init.write().await;
                *st = InitState::ready_degraded(snap_path.clone(), e.clone());
                crate::logger().error(&format!(
                    "pwmd startup phase: ready_degraded (snapshot error: {e})"
                ));
                shutdown_if_fatal_snapshot(&app, "backend_load");
            }
        }
    });
}

pub async fn run_with(config: PwmdConfig) -> Result<(), String> {
    config.validate_persist_snap()?;
    config.validate_cluster_cfg()?;
    if config.transport.enabled && config.transport.peer_listen == config.listen {
        return Err(format!(
            "peer listener must use a dedicated socket; rpc={} peer={}",
            config.listen, config.transport.peer_listen
        ));
    }
    let cors = cors_for_listen(config.listen)?;
    let mut app = app_from_genesis_id(
        &config.genesis,
        config.shard,
        Some(config.data_file.clone()),
        Some(config.identity.clone()),
    )?;
    if app.log_ctl.is_none() {
        app.log_ctl = crate::logging::runtime_log_ctl();
    }
    app.op_token = std::env::var("PWM_ADMIN_TOKEN")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .map(Arc::<str>::from);
    if let Some(ref raw) = config.node_instance_id_override {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            app.node_instance_id = trimmed.to_string();
            let mut rt = app
                .lease_runtime
                .lock()
                .map_err(|_| "lease runtime mutex poisoned".to_string())?;
            rt.owner_id = trimmed.to_string();
        }
    }
    let stable_id_missing = config
        .node_instance_id_override
        .as_ref()
        .map(|s| s.trim().is_empty())
        .unwrap_or(true);
    if config.cluster.enabled && !config.cluster.members.is_empty() && stable_id_missing {
        warn!(
            "cluster_members are static but --node-instance-id is unset: default wire id is node_id-pid-time_ms and changes each run, so quorum labels will not match --cluster-members across restarts; use --node-instance-id for labs and soak scripts"
        );
    }
    {
        let mut hs = app.handshake.write().await;
        hs.validation_ctx.expected_network_id = config.identity.network_id.clone();
        hs.validation_ctx.expected_genesis_hash = Some({
            let g = app.inner.read().await;
            hex::encode(digest(&g.chain.cfg.state0()))
        });
        hs.local_domain_hi = config.identity.cluster_domain_hi;
    }
    {
        let mut tc = app.transport_config.write().await;
        *tc = config.transport.clone();
    }
    {
        let mut st = app.init.write().await;
        *st = InitState::starting(Some(config.data_file.clone()));
    }
    let genesis_d_hex = {
        let g = app.inner.read().await;
        hex::encode(digest(&g.chain.cfg.state0()))
    };
    app.autosnapshot_backend = Some(config.persisted_snap_backend(&genesis_d_hex)?);
    app.snapshot_verify_chain = config.snapshot_verify_chain;
    app.exit_on_fatal_snapshot = config.exit_on_fatal_snapshot;
    app.broke_trust_test = config.broke_trust_test;
    app.debug_stop_height = config.debug_stop_height;
    app.debug_dump = config.debug_dump.clone();
    app.debug_disable_seal_loop = config.debug_disable_seal_loop;
    app.deployment_profile = config.deployment_profile;
    app.seal_role = derive_seal_role(&config);
    app.lease_cfg = LeaseCfg {
        ttl_ms: config.seal_lease_ttl_ms,
        takeover_ms: config.seal_takeover_timeout_ms,
        max_tip_lag: config.seal_takeover_tip_lag,
    };
    app.lease_mode = config.seal_lease_backend;
    app.lease_path = match config.seal_lease_backend {
        crate::lease::LeaseBackendMode::File => Some(config.seal_lease_dir.clone()),
        crate::lease::LeaseBackendMode::ProcessLocal => None,
    };
    app.lease_backend = match config.seal_lease_backend {
        crate::lease::LeaseBackendMode::File => Arc::new(
            crate::lease_backend::FileLeaseBackend::open(config.seal_lease_dir.clone())?,
        ),
        crate::lease::LeaseBackendMode::ProcessLocal => {
            Arc::new(crate::lease_backend::ProcessLocalLeaseBackend)
        }
    };
    app.cluster_cfg = config.cluster.clone();
    if let Ok(mut slot) = app.lease_last_err.lock() {
        *slot = None;
    }
    {
        let mut rt = app
            .lease_runtime
            .lock()
            .map_err(|_| "lease runtime mutex poisoned".to_string())?;
        rt.state = if matches!(app.seal_role, SealRole::Active) {
            LeaseState::ActiveSealing
        } else {
            LeaseState::StandbySyncing
        };
    }
    app.debug_align_mid = align_mid_on(config.debug_align_mid, config.debug_det_seal_time);
    {
        let mut hs = app.handshake.write().await;
        hs.deployment_profile = app.deployment_profile;
        hs.local_seal_role = app.seal_role;
        hs.local_validator_hash = Some(app.validator_identity_hash.clone());
        hs.local_instance_id = Some(app.node_instance_id.clone());
    }
    app.dump_count
        .store(0, std::sync::atomic::Ordering::Relaxed);
    {
        let mut g = app.inner.write().await;
        let mode = if config.debug_det_seal_time {
            SealTimeMode::DeterministicHeight
        } else {
            SealTimeMode::WallClock
        };
        g.chain.set_seal_time_mode(mode);
    }
    if app.broke_trust_test {
        warn!(
            "broke_trust_test: peer NodeHello uses a fake genesis digest; handshakes will be rejected by honest nodes (effective_genesis_hash in HTTP status stays canonical)"
        );
    }
    if let Some(stop_h) = app.debug_stop_height {
        warn!(
            "debug-stop-height active (test-only): node will trigger graceful stop at height>={}",
            stop_h
        );
    }
    if config.debug_det_seal_time {
        warn!(
            "debug-deterministic-seal-time active (test/dev-only): seal ts uses deterministic base+height; season/fee time semantics are artificial in this mode"
        );
    }
    if config.debug_align_mid && config.debug_det_seal_time {
        warn!(
            "debug-align-seal-mid-second ignored because debug-deterministic-seal-time is active (deterministic mode wins)"
        );
    } else if app.debug_align_mid {
        warn!(
            "debug-align-seal-mid-second active (test/dev-only): seal loop is aligned near mid-second with bounded wait"
        );
    }
    if config.debug_disable_seal_loop {
        warn!(
            "debug-disable-seal-loop active (test/dev-only): periodic local sealing is disabled; node follows network sync/catch-up only"
        );
    }
    match app.deployment_profile {
        DeploymentProfile::SingleSealer => {
            if matches!(app.lease_mode, crate::lease::LeaseBackendMode::ProcessLocal) {
                warn!(
                    "deployment_profile=single_sealer process-local lease backend is explicitly enabled; same-key multi-process split-brain protection is disabled"
                );
            }
            info!(
                "deployment_profile=single_sealer seal_role={:?} validator_identity_hash={} node_instance_id={} lease_ttl_ms={} takeover_timeout_ms={} takeover_max_tip_lag={} lease_backend={:?} lease_path={}",
                app.seal_role,
                app.validator_identity_hash,
                app.node_instance_id,
                app.lease_cfg.ttl_ms,
                app.lease_cfg.takeover_ms,
                app.lease_cfg.max_tip_lag,
                app.lease_mode,
                app.lease_path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "-".to_string())
            );
        }
        DeploymentProfile::MultiSealerExperimental => {
            warn!(
                "deployment_profile=multi_sealer_experimental enabled: non-default experimental mode; same-validator active/active protection is relaxed only when explicitly allowed by policy"
            );
            info!(
                "deployment_profile=multi_sealer_experimental seal_role={:?} validator_identity_hash={} node_instance_id={}",
                app.seal_role, app.validator_identity_hash, app.node_instance_id
            );
        }
    }
    if app.cluster_cfg.enabled {
        info!(
            "cluster_attest enabled=true role={:?} members={} quorum={}/{} tx_catchup_ms={} attest_timeout_ms={} note=s2_lease_orthogonal",
            app.cluster_cfg.role,
            app.cluster_cfg.members.join(","),
            app.cluster_cfg.quorum_k,
            app.cluster_cfg.quorum_n,
            app.cluster_cfg.tx_catchup_ms,
            app.cluster_cfg.attest_timeout_ms
        );
    } else {
        info!("cluster_attest enabled=false");
    }
    if config.debug_dump.on_divergence {
        let dir = config
            .debug_dump
            .dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(auto data_file_parent/blocks)".to_string());
        warn!(
            "debug-dump-on-divergence active (debug-only): trigger_streak={} dump_cap={} dump_dir={}",
            config.debug_dump.trigger_streak.max(2),
            config.debug_dump.max_files.max(1),
            dir
        );
    }
    let persist_hint = config.persist_diag_hint();
    info!("pwmd snapshot persist {persist_hint}");
    crate::logger().info(&format!("pwmd snapshot persist {persist_hint}"));
    spawn_snapshot_loader(app.clone());
    spawn_seal_loop(app.clone());
    spawn_federation_sweep_loop(app.clone());
    if config.transport.enabled {
        spawn_peer_listener_loop(app.clone(), config.transport.clone());
    }
    if config.transport.enabled && !config.transport.peer_seeds.is_empty() {
        spawn_stateful_transport_loop(app.clone(), config.transport.clone());
    } else {
        spawn_transport_loop(app.clone());
    }
    let (shutdown_done_tx, shutdown_done_rx) = tokio::sync::oneshot::channel::<()>();
    {
        let mut slot = app
            .shutdown_tx
            .lock()
            .map_err(|_| "shutdown mutex poisoned".to_string())?;
        *slot = Some(shutdown_done_tx);
    }
    let r = router(app, cors);
    let listener = tokio::net::TcpListener::bind(config.listen)
        .await
        .map_err(|e| format!("bind {}: {e}", config.listen))?;
    let listen_addr = listener
        .local_addr()
        .map_err(|e| format!("local_addr {}: {e}", config.listen))?;
    let mode = runtime_mode_summary(&config.identity.mode);
    let shard_label = runtime_shard_label(&config.identity, config.shard);
    info!(
        "pwmd listen http://{} peer={} shard={} state_ns={} identity=({},0x{:02X},{},{}) mode={}",
        listen_addr,
        config.transport.peer_listen,
        shard_label,
        storage_namespace(&config.identity),
        config.identity.network_id,
        config.identity.cluster_domain_hi,
        config.identity.cluster_id,
        config.identity.node_id,
        mode.as_str()
    );
    crate::logger().info(&format!(
        "pwmd listening on http://{} peer={} shard={} state_ns={} identity=({},0x{:02X},{},{}) mode={}",
        listen_addr,
        config.transport.peer_listen,
        shard_label,
        storage_namespace(&config.identity),
        config.identity.network_id,
        config.identity.cluster_domain_hi,
        config.identity.cluster_id,
        config.identity.node_id,
        mode.as_str()
    ));
    axum::serve(
        listener,
        r.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        let _ = shutdown_done_rx.await;
        info!("HTTP server graceful shutdown");
    })
    .await
    .map_err(|e| format!("serve: {e}"))?;
    Ok(())
}

pub async fn run() -> Result<(), String> {
    run_with(PwmdConfig::default()).await
}

#[cfg(test)]
mod tests {
    use super::{
        autosnap_hit, run_cluster_gate, run_lease_gate, spawn_seal_loop, tx_bal_diffs,
        AUTOSNAPSHOT_BLOCK_INTERVAL,
    };
    use crate::bootstrap::app_from_genesis_id;
    use crate::config::GenesisSource;
    use crate::handshake::ClusterRole;
    use crate::identity::default_runtime_identity_neutral;
    use crate::state::InitState;
    use crate::DevLane;
    use ed25519_dalek::SigningKey;
    use pwm_core::block::hdr_hash;
    use pwm_core::hd::{account_id_from_parts, domain_of_account_id};
    use pwm_core::tx::{ClaimMode, SignedTx, TxBody};
    use pwm_core::types::Account;
    use pwm_core::SealTimeMode;
    use slip10_ed25519::derive_ed25519_private_key;
    use std::sync::Arc;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    /// Autosnapshot cadence matches 100-block boundary constant (formerly `autosnapshot_interval_hits_every_100_blocks`).
    #[test]
    fn autosnap_mod100_ok() {
        assert_eq!(AUTOSNAPSHOT_BLOCK_INTERVAL, 100);
        let hits: Vec<u64> = (1..=300).filter(|h| autosnap_hit(*h)).collect();
        assert_eq!(hits, vec![100, 200, 300]);
    }

    /// `tx_bal_diffs` sees both legs of a transfer (formerly `tx_bal_diffs_reports_transfer_changes`).
    #[test]
    fn bal_diff_xfer_two_acct() {
        let (gen, sks) = pwm_core::dev_net();
        let mut before = gen.state0();
        let from = gen.accounts[0].acct;
        let to = [9u8; 32];
        before.accounts.insert(
            to,
            Account {
                initialized: true,
                ..Account::default()
            },
        );
        let mut after = before.clone();
        after.accounts.get_mut(&from).expect("from").balance_pwm -= 12;
        after.accounts.get_mut(&to).expect("to").balance_pwm += 10;
        let tx = SignedTx::sign_body(
            &sks[0],
            domain_of_account_id(&from),
            gen.accounts[0].der_idx,
            0,
            TxBody::Transfer {
                to,
                amount: 10,
                fee: 2,
            },
        );
        let diffs = tx_bal_diffs(&before, &after, &tx);
        assert_eq!(diffs.len(), 2);
    }

    #[test]
    fn seal_skip_ctx_block_h() {
        let (g, sks) = pwm_core::dev_net();
        let mut st = g.state0();
        let signer = g.accounts[0].acct;
        {
            let acc = st.accounts.get_mut(&signer).expect("signer");
            // 2 whole PWM staked (2 * PWM_RAW_SCALE raw units) → matured = 2/h; claim_units=2 valid after ≥1h
            acc.staked = 2 * pwm_core::display::PWM_RAW_SCALE;
            acc.last_claim_unix_time = 0;
            acc.last_stake_change_height = 0;
        }
        let tx = SignedTx::sign_body(
            &sks[0],
            domain_of_account_id(&signer),
            g.accounts[0].der_idx,
            0,
            TxBody::Claim {
                mode: ClaimMode::Free,
                claim_units: 2,
                anchor_ref: 1,
                fee: 0,
            },
        );
        let no_bad = super::first_bad_tx_ctx(&st, &[tx.clone()], 1, 7_200);
        assert!(
            no_bad.is_none(),
            "block-aware ctx should accept claim anchor"
        );
        let bad = super::first_bad_tx_ctx(&st, &[tx], 0, 7_200);
        assert!(bad.is_some(), "zero-height ctx should reject claim anchor");
    }

    /// Seal loop writes snapshot JSON when data path configured (formerly `seal_writes_snapshot_file_when_data_file_is_configured`).
    #[tokio::test]
    async fn seal_snap_if_datafile() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let snapshot_path =
            std::env::temp_dir().join(format!("pwmd-slice19-snapshot-{suffix}.json"));
        let _ = std::fs::remove_file(&snapshot_path);

        let identity = default_runtime_identity_neutral();
        let app = app_from_genesis_id(
            &GenesisSource::DevNet,
            DevLane::Lane0,
            Some(snapshot_path.clone()),
            Some(identity),
        )
        .expect("app boot");

        {
            let mut st = app.init.write().await;
            *st = InitState::ready(Some(snapshot_path.clone()));
        }

        {
            let mut g = app.inner.write().await;
            let (cfg, sks) = pwm_core::dev_net();
            let seed = [99u8; 32];
            let sk1_bytes = derive_ed25519_private_key(&seed, &[0, 1]);
            let sk1 = SigningKey::from_bytes(&sk1_bytes);
            let peer = account_id_from_parts(&sk1.verifying_key().to_bytes(), 1);
            let peer_dom = domain_of_account_id(&peer);
            let init_peer =
                SignedTx::sign_body(&sk1, peer_dom, 1, 0, TxBody::Init { index: 1, flags: 0 });
            g.chain.st.apply_tx(&init_peer).expect("init peer");

            let sender = &sks[0];
            let from = cfg.accounts[0].acct;
            // Prime chain to 99 so first loop seal hits periodic autosnapshot boundary at 100.
            for _ in 0..(AUTOSNAPSHOT_BLOCK_INTERVAL - 1) {
                g.chain.seal(vec![]).expect("prime seal");
            }
            let tx = SignedTx::sign_body(
                sender,
                domain_of_account_id(&from),
                cfg.accounts[0].der_idx,
                0,
                TxBody::Transfer {
                    to: peer,
                    amount: 1,
                    fee: 0,
                },
            );
            g.pool.push(tx).expect("push tx");
        }

        spawn_seal_loop(app.clone());

        // Seal loop uses a 2s periodic tick; allow first seal attempt before polling.
        tokio::time::sleep(Duration::from_millis(2100)).await;

        let mut ok = false;
        for _ in 0..40 {
            let manifest_path = snapshot_path
                .parent()
                .expect("parent")
                .join("epochs")
                .join("pwm-epochs-manifest.json");
            if manifest_path.exists() {
                ok = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
        assert!(ok, "snapshot file was not created after seal");
        let _ = std::fs::remove_file(&snapshot_path);
    }

    #[tokio::test]
    async fn lease_gate_backend_err_closed() {
        use crate::bootstrap::app_from_dev_net;
        use crate::lease::LeaseState;
        use crate::lease_backend::ErrLeaseBackend;

        let mut app = app_from_dev_net();
        app.lease_backend = Arc::new(ErrLeaseBackend {
            msg: "inject_acquire_fail",
        });
        let allow = run_lease_gate(&app).await;
        assert!(!allow);
        let rt = app.lease_runtime.lock().expect("lease mutex");
        assert!(!rt.allow_seal);
        assert!(matches!(rt.state, LeaseState::FencedStandby));
        assert!(
            rt.last_reason.starts_with("lease_backend_error "),
            "reason={}",
            rt.last_reason
        );
        drop(rt);
        let slot = app.lease_last_err.lock().expect("err mutex");
        let got = slot.as_ref().expect("last err");
        assert!(
            got.contains("inject_acquire_fail"),
            "unexpected last err: {got}"
        );
    }

    #[tokio::test]
    async fn cluster_gate_2of2_ok() {
        let mut app = app_from_genesis_id(
            &GenesisSource::DevNet,
            DevLane::Lane0,
            None,
            Some(default_runtime_identity_neutral()),
        )
        .expect("app");
        app.cluster_cfg.enabled = true;
        app.cluster_cfg.role = crate::handshake::ClusterRole::Proposer;
        app.cluster_cfg.members = vec!["node-a".to_string(), "node-b".to_string()];
        app.cluster_cfg.quorum_n = 2;
        app.cluster_cfg.quorum_k = 1;
        let h = app.inner.read().await.chain.tip_h().saturating_add(1);
        {
            let mut hs = app.handshake.write().await;
            let round = hs.cluster_attest.rounds.entry((h, 0)).or_default();
            round.vote_object = "vo1".to_string();
            round.candidate_hash = "aa".repeat(32);
            round.proposer_id = Some("node-a".to_string());
            round.propose_opened_at_ms = Some(crate::current_time_ms().unwrap_or(0));
            round
                .attesters
                .insert("node-b".to_string(), "sig".to_string());
        }
        assert!(run_cluster_gate(&app).await);
    }

    #[tokio::test]
    async fn cluster_gate_2of3_ok() {
        let mut app = app_from_genesis_id(
            &GenesisSource::DevNet,
            DevLane::Lane0,
            None,
            Some(default_runtime_identity_neutral()),
        )
        .expect("app");
        app.cluster_cfg.enabled = true;
        app.cluster_cfg.role = crate::handshake::ClusterRole::Proposer;
        app.cluster_cfg.members = vec![
            "node-a".to_string(),
            "node-b".to_string(),
            "node-c".to_string(),
        ];
        app.cluster_cfg.quorum_n = 3;
        app.cluster_cfg.quorum_k = 1;
        let h = app.inner.read().await.chain.tip_h().saturating_add(1);
        {
            let mut hs = app.handshake.write().await;
            let round = hs.cluster_attest.rounds.entry((h, 0)).or_default();
            round.vote_object = "vo1".to_string();
            round.candidate_hash = "bb".repeat(32);
            round.proposer_id = Some("node-a".to_string());
            round.propose_opened_at_ms = Some(crate::current_time_ms().unwrap_or(0));
            round
                .attesters
                .insert("node-b".to_string(), "sig".to_string());
        }
        assert!(run_cluster_gate(&app).await);
    }

    #[tokio::test]
    async fn cluster_gate_2of3_k2_ok() {
        let mut app = app_from_genesis_id(
            &GenesisSource::DevNet,
            DevLane::Lane0,
            None,
            Some(default_runtime_identity_neutral()),
        )
        .expect("app");
        app.cluster_cfg.enabled = true;
        app.cluster_cfg.role = crate::handshake::ClusterRole::Proposer;
        app.cluster_cfg.members = vec![
            "node-a".to_string(),
            "node-b".to_string(),
            "node-c".to_string(),
        ];
        app.cluster_cfg.quorum_n = 3;
        app.cluster_cfg.quorum_k = 2;
        let h = app.inner.read().await.chain.tip_h().saturating_add(1);
        {
            let mut hs = app.handshake.write().await;
            let round = hs.cluster_attest.rounds.entry((h, 0)).or_default();
            round.vote_object = "vo1".to_string();
            round.candidate_hash = "bb".repeat(32);
            round.proposer_id = Some("node-a".to_string());
            round.propose_opened_at_ms = Some(crate::current_time_ms().unwrap_or(0));
            round
                .attesters
                .insert("node-b".to_string(), "sig-b".to_string());
            round
                .attesters
                .insert("node-c".to_string(), "sig-c".to_string());
        }
        assert!(run_cluster_gate(&app).await);
    }

    #[tokio::test]
    async fn cluster_gate_quorum_timeout() {
        let mut app = app_from_genesis_id(
            &GenesisSource::DevNet,
            DevLane::Lane0,
            None,
            Some(default_runtime_identity_neutral()),
        )
        .expect("app");
        app.cluster_cfg.enabled = true;
        app.cluster_cfg.role = crate::handshake::ClusterRole::Proposer;
        app.cluster_cfg.members = vec!["node-a".to_string(), "node-b".to_string()];
        app.cluster_cfg.quorum_n = 2;
        app.cluster_cfg.quorum_k = 1;
        app.cluster_cfg.attest_timeout_ms = 10;
        let h = app.inner.read().await.chain.tip_h().saturating_add(1);
        {
            let mut hs = app.handshake.write().await;
            let round = hs.cluster_attest.rounds.entry((h, 0)).or_default();
            round.vote_object = "vo1".to_string();
            round.candidate_hash = "aa".repeat(32);
            round.proposer_id = Some("node-a".to_string());
            round.propose_opened_at_ms = Some(0);
        }
        assert!(!run_cluster_gate(&app).await);
    }

    #[tokio::test]
    async fn seal_loop_disable_no_seal() {
        let identity = default_runtime_identity_neutral();
        let app = app_from_genesis_id(&GenesisSource::DevNet, DevLane::Lane0, None, Some(identity))
            .expect("app");
        {
            let mut st = app.init.write().await;
            *st = InitState::ready(None);
        }
        assert_eq!(app.inner.read().await.chain.tip_h(), 0);
        let mut app = app;
        app.debug_disable_seal_loop = true;
        spawn_seal_loop(app.clone());
        tokio::time::sleep(Duration::from_millis(2100)).await;
        assert_eq!(app.inner.read().await.chain.tip_h(), 0);
    }

    #[tokio::test]
    async fn seal_loop_attester_no_seal() {
        let identity = default_runtime_identity_neutral();
        let app = app_from_genesis_id(&GenesisSource::DevNet, DevLane::Lane0, None, Some(identity))
            .expect("app");
        {
            let mut st = app.init.write().await;
            *st = InitState::ready(None);
        }
        assert_eq!(app.inner.read().await.chain.tip_h(), 0);
        let mut app = app;
        app.cluster_cfg.enabled = true;
        app.cluster_cfg.role = ClusterRole::Attester;
        app.debug_disable_seal_loop = false;
        spawn_seal_loop(app.clone());
        tokio::time::sleep(Duration::from_millis(2100)).await;
        assert_eq!(app.inner.read().await.chain.tip_h(), 0);
    }

    #[test]
    fn derive_role_attester_is_standby() {
        let mut cfg = crate::config::PwmdConfig::default();
        cfg.cluster.enabled = true;
        cfg.cluster.role = ClusterRole::Attester;
        cfg.debug_disable_seal_loop = false;
        assert_eq!(
            super::derive_seal_role(&cfg),
            crate::handshake::SealRole::Standby
        );
    }

    #[tokio::test]
    async fn det_mode_stable_hash_apps() {
        let identity = default_runtime_identity_neutral();
        let app1 =
            app_from_genesis_id(&GenesisSource::DevNet, DevLane::Lane0, None, Some(identity))
                .expect("app1");
        let app2 = app_from_genesis_id(
            &GenesisSource::DevNet,
            DevLane::Lane0,
            None,
            Some(default_runtime_identity_neutral()),
        )
        .expect("app2");
        {
            let mut g1 = app1.inner.write().await;
            let mut g2 = app2.inner.write().await;
            g1.chain
                .set_seal_time_mode(SealTimeMode::DeterministicHeight);
            g2.chain
                .set_seal_time_mode(SealTimeMode::DeterministicHeight);
            g1.chain.seal(vec![]).expect("seal app1");
            g2.chain.seal(vec![]).expect("seal app2");
            let b1 = g1.chain.blocks.back().expect("blk1");
            let b2 = g2.chain.blocks.back().expect("blk2");
            assert_eq!(b1.hdr.ts, b2.hdr.ts);
            assert_eq!(hdr_hash(&b1.hdr), hdr_hash(&b2.hdr));
        }
    }
}
