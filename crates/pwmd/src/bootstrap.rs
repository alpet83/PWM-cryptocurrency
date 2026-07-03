//! Constructs `App` from genesis bundles, dev-net presets, and dev-lane identity.

use crate::block_writer::BlockWriter;
use crate::config::{DebugDumpCfg, GenesisSource};
use crate::handshake::{DeploymentProfile, SealRole};
use crate::identity::{default_dev_lane_identity, storage_namespace, DevLane, RuntimeIdentity};
use crate::ledger::CrossShardLedger;
use crate::offchain::OffchainStore;
use crate::pipeline::{
    DispatchQueues, HotIndex, QueueMetrics, TxEvent, ValidatedTx, WorkerCtx, WorkerPool,
    WorkerReads,
};
use crate::roaming::RoamingPool;
use crate::snapshot::incremental::sync_epoch_to_tip;
use crate::snapshot::{load_genesis_bundle, SnapshotBackend, SnapshotLoadOpts};
use crate::state::SealManualState;
use crate::state::StateSnapshot;
use crate::state::{App, InitState, Inner};
use crate::transport::HandshakeState;
use crate::TransportConfig;
use ed25519_dalek::SigningKey;
use pwm_core::genesis::GenCfg;
use pwm_core::{absorb_blocks_tail, dev_net, digest, Chain, Mpool};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{info, warn};

fn block_writer(data_file: &Option<PathBuf>) -> Option<BlockWriter> {
    data_file
        .as_ref()
        .and_then(|path| match BlockWriter::new(path.clone()) {
            Ok(writer) => Some(writer),
            Err(err) => {
                warn!("block writer disabled path={}: {}", path.display(), err);
                None
            }
        })
}

fn mk_node_instance_id(identity: &RuntimeIdentity) -> String {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|v| v.as_millis() as u64)
        .unwrap_or(0);
    format!("{}-{}-{now_ms}", identity.node_id, std::process::id())
}

fn validator_hash_from_cfg(cfg: &GenCfg) -> String {
    cfg.vals
        .set
        .first()
        .map(|v| hex::encode(v.pubkey))
        .unwrap_or_else(|| "unknown-validator".to_string())
}

struct WorkerParts {
    queues: Arc<DispatchQueues>,
    tip_height: Arc<AtomicU64>,
    pool: Arc<WorkerPool>,
    validated_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<ValidatedTx>>>,
}

fn tx_events() -> broadcast::Sender<TxEvent> {
    broadcast::channel(256).0
}

fn worker_counts(logical: usize) -> (usize, usize) {
    let general = logical.saturating_sub(2).max(1);
    (1, general)
}

fn host_worker_counts() -> (usize, usize) {
    let logical = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(2);
    worker_counts(logical)
}

#[cfg(test)]
mod worker_count_tests {
    use super::worker_counts;

    #[test]
    fn worker_counts_scale() {
        assert_eq!(worker_counts(1), (1, 1));
        assert_eq!(worker_counts(4), (1, 2));
        assert_eq!(worker_counts(16), (1, 14));
    }
}

fn worker_parts(
    chain: &Chain,
    snapshot: Arc<StateSnapshot>,
    hot_index: Arc<HotIndex>,
    metrics: Arc<QueueMetrics>,
) -> WorkerParts {
    const WORKER_Q_CAP: usize = 256;
    const VALIDATED_Q_CAP: usize = 4096;
    let (queues, receivers) =
        DispatchQueues::new_with_workers(WORKER_Q_CAP, WORKER_Q_CAP, WORKER_Q_CAP);
    let (valid_tx, valid_rx) = mpsc::channel(VALIDATED_Q_CAP);
    let tip_height = Arc::new(AtomicU64::new(chain.tip_h()));
    let reads = WorkerReads::new(
        snapshot,
        hot_index,
        Arc::new(chain.cfg.clone()),
        Arc::clone(&tip_height),
    );
    let ctx = WorkerCtx::new(reads, valid_tx, metrics);
    let (affinity, general) = host_worker_counts();
    WorkerParts {
        queues: Arc::new(queues),
        tip_height,
        pool: Arc::new(WorkerPool::new(affinity, general, Arc::new(receivers), ctx)),
        validated_rx: Arc::new(tokio::sync::Mutex::new(valid_rx)),
    }
}

pub fn app_from_dev_net() -> App {
    let (cfg, sks) = dev_net();
    app_from_chain_boot(cfg, sks, None, DevLane::Lane0, None)
}

/// Constructs an App from a devnet lane configuration.
pub fn app_from_devnet(dev_lane: DevLane) -> App {
    let (cfg, sks) = dev_net();
    app_from_chain_boot(cfg, sks, None, dev_lane, None)
}

pub(crate) fn app_from_chain_boot(
    cfg: GenCfg,
    sks: Vec<SigningKey>,
    data_file: Option<PathBuf>,
    dev_lane: DevLane,
    runtime_identity: Option<RuntimeIdentity>,
) -> App {
    let identity = runtime_identity.unwrap_or_else(|| default_dev_lane_identity(dev_lane));
    let state_namespace = storage_namespace(&identity);
    let local_domain_hi = identity.cluster_domain_hi;
    let network_id = identity.network_id.clone();
    let validation_ctx = crate::handshake::HandshakeValidationCtx {
        expected_network_id: network_id,
        expected_genesis_hash: Some(hex::encode(digest(&cfg.state0()))),
        skew_window_ms: 30_000,
    };
    let dev_profile = identity.network_id.starts_with("dev");
    let chain = Chain::boot(cfg, sks);
    let validator_identity_hash = validator_hash_from_cfg(&chain.cfg);
    let node_instance_id = mk_node_instance_id(&identity);
    let lease_runtime = crate::lease::LeaseRuntime::new(node_instance_id.clone());
    let pool = Mpool::new(4096);
    let roaming_pool = RoamingPool::default();
    let inner = Inner {
        chain,
        pool,
        roaming_pool,
        cross_shard: CrossShardLedger::default(),
        federation: Default::default(),
        peer_account_views: std::collections::HashMap::new(),
        recent_flow: VecDeque::new(),
    };
    let state_snapshot = Arc::new(StateSnapshot::new(Arc::new(inner.chain.st.clone())));
    let hot_index = Arc::new(HotIndex::new(&inner.chain.st));
    let pipeline_metrics = Arc::new(QueueMetrics::default());
    let worker = worker_parts(
        &inner.chain,
        Arc::clone(&state_snapshot),
        Arc::clone(&hot_index),
        Arc::clone(&pipeline_metrics),
    );
    let autosnapshot_backend = data_file
        .as_ref()
        .map(|p| SnapshotBackend::JsonFile { path: p.clone() });
    let block_writer = block_writer(&data_file);
    App {
        inner: Arc::new(RwLock::new(inner)),
        state_snapshot,
        hot_index,
        tx_ingress: Arc::new(crate::pipeline::TxIngressChannel::new(256)),
        worker_queues: worker.queues,
        offchain: Arc::new(OffchainStore::new()),
        worker_tip_height: worker.tip_height,
        _worker_pool: worker.pool,
        _validated_rx: worker.validated_rx,
        pipeline_metrics,
        tx_events: tx_events(),
        init: Arc::new(RwLock::new(InitState::ready(data_file.clone()))),
        data_file,
        autosnapshot_backend,
        block_writer,
        shard: dev_lane,
        handshake: Arc::new(RwLock::new(HandshakeState::new(
            validation_ctx,
            local_domain_hi,
        ))),
        dev_profile,
        snapshot_verify_chain: true,
        exit_on_fatal_snapshot: false,
        broke_trust_test: false,
        debug_stop_height: None,
        debug_align_mid: false,
        debug_disable_seal_loop: false,
        deployment_profile: DeploymentProfile::SingleSealer,
        seal_role: SealRole::Active,
        lease_cfg: crate::lease::LeaseCfg::default(),
        lease_mode: crate::lease::LeaseBackendMode::ProcessLocal,
        lease_path: None,
        lease_last_err: Arc::new(Mutex::new(None)),
        lease_backend: Arc::new(crate::lease_backend::ProcessLocalLeaseBackend),
        lease_runtime: Arc::new(Mutex::new(lease_runtime)),
        lease_stats: Arc::new(crate::lease::LeaseStats::default()),
        lease_renew_log_tip: Arc::new(AtomicU64::new(0)),
        cluster_cfg: crate::ClusterCfg::default(),
        cluster_prep_wait_ms: Arc::new(AtomicU64::new(0)),
        cluster_prop_nudge: Arc::new(AtomicBool::new(false)),
        seal_wake: Arc::new(tokio::sync::Notify::new()),
        seal_manual: Arc::new(RwLock::new(SealManualState::default())),
        lab_seal_api: false,
        validator_identity_hash,
        node_instance_id,
        debug_dump: DebugDumpCfg::default(),
        dump_count: Arc::new(AtomicU64::new(0)),
        last_snapshot_height: Arc::new(AtomicU64::new(0)),
        identity,
        state_namespace,
        hello_nonce_ctr: Arc::new(AtomicU64::new(1)),
        transport_config: Arc::new(RwLock::new(TransportConfig::default())),
        block_timing: None,
        shutdown_requested: Arc::new(AtomicBool::new(false)),
        shutdown_tx: Arc::new(Mutex::new(None)),
        log_ctl: crate::logging::runtime_log_ctl(),
        log_ovr: Arc::new(RwLock::new(None)),
        log_ovr_rev: Arc::new(AtomicU64::new(0)),
        op_token: None,
        rpc_allow: crate::rpc_allow::RpcAllowState::default(),
    }
}

/// Constructs an App from genesis using default lane 0.
pub fn app_from_genesis_def(source: &GenesisSource) -> Result<App, String> {
    app_from_genesis(source, DevLane::Lane0)
}

/// Constructs an App by loading genesis for the given lane.
pub fn app_from_genesis(source: &GenesisSource, dev_lane: DevLane) -> Result<App, String> {
    app_from_genesis_id(source, dev_lane, None, None)
}

/// Constructs an App from genesis using the provided lane identity.
pub(crate) fn app_from_genesis_id(
    source: &GenesisSource,
    dev_lane: DevLane,
    data_file: Option<PathBuf>,
    runtime_identity: Option<RuntimeIdentity>,
) -> Result<App, String> {
    let (cfg, sks) = match source {
        GenesisSource::DevNet => dev_net(),
        GenesisSource::JsonFile { path, passphrase } => {
            load_genesis_bundle(path, Some(passphrase.as_str()))?
        }
    };
    Ok(app_from_chain_boot(
        cfg,
        sks,
        data_file,
        dev_lane,
        runtime_identity,
    ))
}

/// Constructs an App from genesis with explicit data path.
pub fn app_from_genesis_data(
    source: &GenesisSource,
    data_file: Option<PathBuf>,
) -> Result<App, String> {
    app_from_genesis_shard(source, data_file, DevLane::Lane0)
}

/// Constructs an App from genesis data bound to a specific lane.
pub fn app_from_genesis_shard(
    source: &GenesisSource,
    data_file: Option<PathBuf>,
    dev_lane: DevLane,
) -> Result<App, String> {
    let (cfg, sks) = match source {
        GenesisSource::DevNet => dev_net(),
        GenesisSource::JsonFile { path, passphrase } => {
            load_genesis_bundle(path, Some(passphrase.as_str()))?
        }
    };
    let identity = default_dev_lane_identity(dev_lane);
    let state_namespace = storage_namespace(&identity);
    let mut chain = Chain::boot(cfg, sks);
    if let Some(path) = data_file.as_deref() {
        let backend = SnapshotBackend::JsonFile {
            path: path.to_path_buf(),
        };
        if let (Some(snap), _) = backend.load(&chain.cfg, SnapshotLoadOpts::verify_full())? {
            let (blocks, state, roaming_pool, cross_shard) = snap.into_runtime()?;
            chain.blocks = absorb_blocks_tail(blocks);
            chain.st = state;
            chain.sync_canon_h();
            info!("loaded snapshot from {}", path.display());
            let pool = Mpool::new(4096);
            let inner = Inner {
                chain,
                pool,
                roaming_pool,
                cross_shard,
                federation: Default::default(),
                peer_account_views: std::collections::HashMap::new(),
                recent_flow: VecDeque::new(),
            };
            if crate::snapshot::epoch::manifest_file_path(path).exists() {
                sync_epoch_to_tip(path, &inner)?;
            }
            let state_snapshot = Arc::new(StateSnapshot::new(Arc::new(inner.chain.st.clone())));
            let hot_index = Arc::new(HotIndex::new(&inner.chain.st));
            let pipeline_metrics = Arc::new(QueueMetrics::default());
            let worker = worker_parts(
                &inner.chain,
                Arc::clone(&state_snapshot),
                Arc::clone(&hot_index),
                Arc::clone(&pipeline_metrics),
            );
            let validation_ctx = crate::handshake::HandshakeValidationCtx {
                expected_network_id: identity.network_id.clone(),
                expected_genesis_hash: Some(hex::encode(digest(&inner.chain.cfg.state0()))),
                skew_window_ms: 30_000,
            };
            let autosnapshot_backend = data_file
                .as_ref()
                .map(|p| SnapshotBackend::JsonFile { path: p.clone() });
            let block_writer = block_writer(&data_file);
            let validator_identity_hash = validator_hash_from_cfg(&inner.chain.cfg);
            let node_instance_id = mk_node_instance_id(&identity);
            return Ok(App {
                inner: Arc::new(RwLock::new(inner)),
                state_snapshot,
                hot_index,
                tx_ingress: Arc::new(crate::pipeline::TxIngressChannel::new(256)),
                worker_queues: worker.queues,
                offchain: Arc::new(OffchainStore::new()),
                worker_tip_height: worker.tip_height,
                _worker_pool: worker.pool,
                _validated_rx: worker.validated_rx,
                pipeline_metrics,
                tx_events: tx_events(),
                init: Arc::new(RwLock::new(InitState::ready(data_file.clone()))),
                data_file,
                shard: dev_lane,
                handshake: Arc::new(RwLock::new(HandshakeState::new(
                    validation_ctx,
                    identity.cluster_domain_hi,
                ))),
                dev_profile: true,
                snapshot_verify_chain: true,
                exit_on_fatal_snapshot: false,
                broke_trust_test: false,
                debug_stop_height: None,
                debug_align_mid: false,
                debug_disable_seal_loop: false,
                deployment_profile: DeploymentProfile::SingleSealer,
                seal_role: SealRole::Active,
                lease_cfg: crate::lease::LeaseCfg::default(),
                lease_mode: crate::lease::LeaseBackendMode::ProcessLocal,
                lease_path: None,
                lease_last_err: Arc::new(Mutex::new(None)),
                lease_backend: Arc::new(crate::lease_backend::ProcessLocalLeaseBackend),
                lease_runtime: Arc::new(Mutex::new(crate::lease::LeaseRuntime::new(
                    node_instance_id.clone(),
                ))),
                lease_stats: Arc::new(crate::lease::LeaseStats::default()),
                lease_renew_log_tip: Arc::new(AtomicU64::new(0)),
                cluster_cfg: crate::ClusterCfg::default(),
                cluster_prep_wait_ms: Arc::new(AtomicU64::new(0)),
                cluster_prop_nudge: Arc::new(AtomicBool::new(false)),
                seal_wake: Arc::new(tokio::sync::Notify::new()),
                seal_manual: Arc::new(RwLock::new(SealManualState::default())),
                lab_seal_api: false,
                validator_identity_hash,
                node_instance_id,
                debug_dump: DebugDumpCfg::default(),
                dump_count: Arc::new(AtomicU64::new(0)),
                last_snapshot_height: Arc::new(AtomicU64::new(0)),
                identity,
                state_namespace,
                hello_nonce_ctr: Arc::new(AtomicU64::new(1)),
                transport_config: Arc::new(RwLock::new(TransportConfig::default())),
                block_timing: None,
                shutdown_requested: Arc::new(AtomicBool::new(false)),
                autosnapshot_backend,
                block_writer,
                shutdown_tx: Arc::new(Mutex::new(None)),
                log_ctl: crate::logging::runtime_log_ctl(),
                log_ovr: Arc::new(RwLock::new(None)),
                log_ovr_rev: Arc::new(AtomicU64::new(0)),
                op_token: None,
                rpc_allow: crate::rpc_allow::RpcAllowState::default(),
            });
        }
    }
    let pool = Mpool::new(4096);
    let roaming_pool = RoamingPool::default();
    let inner = Inner {
        chain,
        pool,
        roaming_pool,
        cross_shard: CrossShardLedger::default(),
        federation: Default::default(),
        peer_account_views: std::collections::HashMap::new(),
        recent_flow: VecDeque::new(),
    };
    let state_snapshot = Arc::new(StateSnapshot::new(Arc::new(inner.chain.st.clone())));
    let hot_index = Arc::new(HotIndex::new(&inner.chain.st));
    let pipeline_metrics = Arc::new(QueueMetrics::default());
    let worker = worker_parts(
        &inner.chain,
        Arc::clone(&state_snapshot),
        Arc::clone(&hot_index),
        Arc::clone(&pipeline_metrics),
    );
    let validation_ctx = crate::handshake::HandshakeValidationCtx {
        expected_network_id: identity.network_id.clone(),
        expected_genesis_hash: Some(hex::encode(digest(&inner.chain.cfg.state0()))),
        skew_window_ms: 30_000,
    };
    let autosnapshot_backend = data_file
        .as_ref()
        .map(|p| SnapshotBackend::JsonFile { path: p.clone() });
    let block_writer = block_writer(&data_file);
    let validator_identity_hash = validator_hash_from_cfg(&inner.chain.cfg);
    let node_instance_id = mk_node_instance_id(&identity);
    Ok(App {
        inner: Arc::new(RwLock::new(inner)),
        state_snapshot,
        hot_index,
        tx_ingress: Arc::new(crate::pipeline::TxIngressChannel::new(256)),
        offchain: Arc::new(OffchainStore::new()),
        worker_queues: worker.queues,
        worker_tip_height: worker.tip_height,
        _worker_pool: worker.pool,
        _validated_rx: worker.validated_rx,
        pipeline_metrics,
        tx_events: tx_events(),
        init: Arc::new(RwLock::new(InitState::ready(data_file.clone()))),
        data_file,
        shard: dev_lane,
        handshake: Arc::new(RwLock::new(HandshakeState::new(
            validation_ctx,
            identity.cluster_domain_hi,
        ))),
        dev_profile: true,
        snapshot_verify_chain: true,
        exit_on_fatal_snapshot: false,
        broke_trust_test: false,
        debug_stop_height: None,
        debug_align_mid: false,
        debug_disable_seal_loop: false,
        deployment_profile: DeploymentProfile::SingleSealer,
        seal_role: SealRole::Active,
        lease_cfg: crate::lease::LeaseCfg::default(),
        lease_mode: crate::lease::LeaseBackendMode::ProcessLocal,
        lease_path: None,
        lease_last_err: Arc::new(Mutex::new(None)),
        lease_backend: Arc::new(crate::lease_backend::ProcessLocalLeaseBackend),
        lease_runtime: Arc::new(Mutex::new(crate::lease::LeaseRuntime::new(
            node_instance_id.clone(),
        ))),
        lease_stats: Arc::new(crate::lease::LeaseStats::default()),
        lease_renew_log_tip: Arc::new(AtomicU64::new(0)),
        cluster_cfg: crate::ClusterCfg::default(),
        cluster_prep_wait_ms: Arc::new(AtomicU64::new(0)),
        cluster_prop_nudge: Arc::new(AtomicBool::new(false)),
        seal_wake: Arc::new(tokio::sync::Notify::new()),
        seal_manual: Arc::new(RwLock::new(SealManualState::default())),
        lab_seal_api: false,
        validator_identity_hash,
        node_instance_id,
        debug_dump: DebugDumpCfg::default(),
        dump_count: Arc::new(AtomicU64::new(0)),
        last_snapshot_height: Arc::new(AtomicU64::new(0)),
        identity,
        state_namespace,
        hello_nonce_ctr: Arc::new(AtomicU64::new(1)),
        transport_config: Arc::new(RwLock::new(TransportConfig::default())),
        block_timing: None,
        autosnapshot_backend,
        block_writer,
        shutdown_requested: Arc::new(AtomicBool::new(false)),
        shutdown_tx: Arc::new(Mutex::new(None)),
        log_ctl: crate::logging::runtime_log_ctl(),
        log_ovr: Arc::new(RwLock::new(None)),
        log_ovr_rev: Arc::new(AtomicU64::new(0)),
        op_token: None,
        rpc_allow: crate::rpc_allow::RpcAllowState::default(),
    })
}
