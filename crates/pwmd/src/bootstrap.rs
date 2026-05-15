//! Constructs `App` from genesis bundles, dev-net presets, and dev-lane identity.

use crate::config::{DebugDumpCfg, GenesisSource};
use crate::handshake::{DeploymentProfile, SealRole};
use crate::identity::{default_dev_lane_identity, storage_namespace, DevLane, RuntimeIdentity};
use crate::ledger::CrossShardLedger;
use crate::roaming::RoamingPool;
use crate::snapshot::{load_genesis_bundle, SnapshotBackend, SnapshotLoadOpts};
use crate::state::{App, InitState, Inner};
use crate::transport::HandshakeState;
use crate::TransportConfig;
use ed25519_dalek::SigningKey;
use pwm_core::genesis::GenCfg;
use pwm_core::{absorb_blocks_tail, dev_net, digest, Chain, Mpool};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;
use tracing::info;

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
    let autosnapshot_backend = data_file
        .as_ref()
        .map(|p| SnapshotBackend::JsonFile { path: p.clone() });
    App {
        inner: Arc::new(RwLock::new(inner)),
        init: Arc::new(RwLock::new(InitState::ready(data_file.clone()))),
        data_file,
        autosnapshot_backend,
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
        cluster_cfg: crate::ClusterCfg::default(),
        validator_identity_hash,
        node_instance_id,
        debug_dump: DebugDumpCfg::default(),
        dump_count: Arc::new(AtomicU64::new(0)),
        last_snapshot_height: Arc::new(AtomicU64::new(0)),
        identity,
        state_namespace,
        hello_nonce_ctr: Arc::new(AtomicU64::new(1)),
        transport_config: Arc::new(RwLock::new(TransportConfig::default())),
        shutdown_tx: Arc::new(Mutex::new(None)),
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
            let validation_ctx = crate::handshake::HandshakeValidationCtx {
                expected_network_id: identity.network_id.clone(),
                expected_genesis_hash: Some(hex::encode(digest(&inner.chain.cfg.state0()))),
                skew_window_ms: 30_000,
            };
            let autosnapshot_backend = data_file
                .as_ref()
                .map(|p| SnapshotBackend::JsonFile { path: p.clone() });
            let validator_identity_hash = validator_hash_from_cfg(&inner.chain.cfg);
            let node_instance_id = mk_node_instance_id(&identity);
            return Ok(App {
                inner: Arc::new(RwLock::new(inner)),
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
                cluster_cfg: crate::ClusterCfg::default(),
                validator_identity_hash,
                node_instance_id,
                debug_dump: DebugDumpCfg::default(),
                dump_count: Arc::new(AtomicU64::new(0)),
                last_snapshot_height: Arc::new(AtomicU64::new(0)),
                identity,
                state_namespace,
                hello_nonce_ctr: Arc::new(AtomicU64::new(1)),
                transport_config: Arc::new(RwLock::new(TransportConfig::default())),
                autosnapshot_backend,
                shutdown_tx: Arc::new(Mutex::new(None)),
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
    let validation_ctx = crate::handshake::HandshakeValidationCtx {
        expected_network_id: identity.network_id.clone(),
        expected_genesis_hash: Some(hex::encode(digest(&inner.chain.cfg.state0()))),
        skew_window_ms: 30_000,
    };
    let autosnapshot_backend = data_file
        .as_ref()
        .map(|p| SnapshotBackend::JsonFile { path: p.clone() });
    let validator_identity_hash = validator_hash_from_cfg(&inner.chain.cfg);
    let node_instance_id = mk_node_instance_id(&identity);
    Ok(App {
        inner: Arc::new(RwLock::new(inner)),
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
        cluster_cfg: crate::ClusterCfg::default(),
        validator_identity_hash,
        node_instance_id,
        debug_dump: DebugDumpCfg::default(),
        dump_count: Arc::new(AtomicU64::new(0)),
        last_snapshot_height: Arc::new(AtomicU64::new(0)),
        identity,
        state_namespace,
        hello_nonce_ctr: Arc::new(AtomicU64::new(1)),
        transport_config: Arc::new(RwLock::new(TransportConfig::default())),
        autosnapshot_backend,
        shutdown_tx: Arc::new(Mutex::new(None)),
    })
}
