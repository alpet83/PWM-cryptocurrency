//! Block ticks, mempool drains, autosnapshot triggers, and federation sweep hooks.
//! `spawn_snapshot_loader` logs `Instant`-based stages on target `pwmd::startup::snapshot`.

use crate::api::common::{rollback_commit, take_bak};
use crate::bootstrap::app_from_genesis_shard_identity;
use crate::config::PwmdConfig;
use crate::ledger::{summary_log_line, SUMMARY_BLOCK_INTERVAL};
use crate::runtime_shard_label;
use crate::snapshot::{
    BlocksStored, SnapIoTiming, SnapshotBackend, SnapshotLoadOpts, SNAP_STARTUP_TARGET,
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
use std::time::Instant;
use tracing::{error, info, warn};

pub const AUTOSNAPSHOT_BLOCK_INTERVAL: u64 = crate::snapshot::epoch::SNAP_CHK_BLK_IV;

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
        RuntimeIdentityMode::Alias { shard } => {
            format!("{}(alias:{})", mode.as_runtime_label(), shard.as_str())
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

pub fn spawn_seal_loop(app: App) {
    tokio::spawn(async move {
        let mut iv = tokio::time::interval(std::time::Duration::from_secs(2));
        loop {
            iv.tick().await;
            if !app.init.read().await.allows_chain_progress() {
                continue;
            }
            let mut g = app.inner.write().await;
            let now_h = g.chain.tip_h();
            let expired = g.roaming_pool.expire_by_height(now_h);
            if expired > 0 {
                info!("expired roaming intents count={} height={}", expired, now_h);
            }
            let txs = g.pool.take(64);
            let st_before = g.chain.st.clone();
            let persist_back = app.autosnapshot_backend.is_some();
            let bak_opt = persist_back.then(|| take_bak(&g));
            match g.chain.seal(txs) {
                Ok(()) => {
                    let h = g.chain.tip_h();
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
                    let mut save_result: Option<(Option<std::path::PathBuf>, Result<(), String>)> =
                        None;
                    if let Some(ref backend) = app.autosnapshot_backend {
                        let periodic_hit = h > 0 && h % AUTOSNAPSHOT_BLOCK_INTERVAL == 0;
                        if periodic_hit {
                            info!(
                                "autosnapshot checkpoint hit interval={} height={}",
                                AUTOSNAPSHOT_BLOCK_INTERVAL, h
                            );
                        }
                        save_result =
                            Some((backend.init_state_path(), backend.save_seal_persist(&g)));
                    }
                    drop(g);
                    if let Some((path, result)) = save_result {
                        match result {
                            Ok(()) => {
                                apply_snapshot_init_state(&app, path, Ok(()), h).await;
                            }
                            Err(e) => {
                                if let Some(bak) = bak_opt {
                                    let mut g = app.inner.write().await;
                                    rollback_commit(&mut g, bak);
                                    drop(g);
                                }
                                apply_snapshot_init_state(&app, path, Err(e), h).await;
                            }
                        }
                    }
                }
                Err((e, txs)) => {
                    let replay = if e.starts_with("tx: ") {
                        let mut st = g.chain.st.clone();
                        let mut drop_at = None;
                        for (i, tx) in txs.iter().enumerate() {
                            if let Err(err) = st.apply_tx(tx) {
                                drop_at = Some((i, err));
                                break;
                            }
                        }
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
    if config.transport.enabled && config.transport.peer_listen == config.listen {
        return Err(format!(
            "peer listener must use a dedicated socket; rpc={} peer={}",
            config.listen, config.transport.peer_listen
        ));
    }
    let cors = cors_for_listen(config.listen)?;
    let mut app = app_from_genesis_shard_identity(
        &config.genesis,
        config.shard,
        Some(config.data_file.clone()),
        Some(config.identity.clone()),
    )?;
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
    if app.broke_trust_test {
        warn!(
            "broke_trust_test: peer NodeHello uses a fake genesis digest; handshakes will be rejected by honest nodes (effective_genesis_hash in HTTP status stays canonical)"
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
    axum::serve(listener, r)
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
    use super::{spawn_seal_loop, tx_bal_diffs, AUTOSNAPSHOT_BLOCK_INTERVAL};
    use crate::bootstrap::app_from_genesis_shard_identity;
    use crate::config::GenesisSource;
    use crate::identity::default_runtime_identity_neutral;
    use crate::state::InitState;
    use crate::ShardId;
    use ed25519_dalek::SigningKey;
    use pwm_core::hd::{account_id_from_parts, domain_of_account_id};
    use pwm_core::tx::{SignedTx, TxBody};
    use pwm_core::types::Account;
    use slip10_ed25519::derive_ed25519_private_key;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    /// Autosnapshot cadence matches 100-block boundary constant (formerly `autosnapshot_interval_hits_every_100_blocks`).
    #[test]
    fn autosnap_mod100_ok() {
        assert_eq!(AUTOSNAPSHOT_BLOCK_INTERVAL, 100);
        let hits: Vec<u64> = (1..=300)
            .filter(|h| *h > 0 && *h % AUTOSNAPSHOT_BLOCK_INTERVAL == 0)
            .collect();
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
        let app = app_from_genesis_shard_identity(
            &GenesisSource::DevNet,
            ShardId::A,
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
}
