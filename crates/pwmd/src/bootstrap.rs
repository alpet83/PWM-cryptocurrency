//! Constructs `App` from genesis bundles, dev-net presets, and shard identity.

use crate::config::GenesisSource;
use crate::identity::{
    default_runtime_identity_for_shard, storage_namespace, RuntimeIdentity, ShardId,
};
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

pub fn app_from_dev_net() -> App {
    let (cfg, sks) = dev_net();
    app_from_chain_boot(cfg, sks, None, ShardId::A, None)
}

pub fn app_from_dev_net_shard(shard: ShardId) -> App {
    let (cfg, sks) = dev_net();
    app_from_chain_boot(cfg, sks, None, shard, None)
}

pub(crate) fn app_from_chain_boot(
    cfg: GenCfg,
    sks: Vec<SigningKey>,
    data_file: Option<PathBuf>,
    shard: ShardId,
    runtime_identity: Option<RuntimeIdentity>,
) -> App {
    let identity = runtime_identity.unwrap_or_else(|| default_runtime_identity_for_shard(shard));
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
    let pool = Mpool::new(4096);
    let roaming_pool = RoamingPool::default();
    let mut inner = Inner {
        chain,
        pool,
        roaming_pool,
        cross_shard: CrossShardLedger::default(),
        federation: Default::default(),
        peer_account_views: std::collections::HashMap::new(),
        recent_flow: VecDeque::new(),
    };
    inner.normalize_marks_quota();
    let autosnapshot_backend = data_file
        .as_ref()
        .map(|p| SnapshotBackend::JsonFile { path: p.clone() });
    App {
        inner: Arc::new(RwLock::new(inner)),
        init: Arc::new(RwLock::new(InitState::ready(data_file.clone()))),
        data_file,
        autosnapshot_backend,
        shard,
        handshake: Arc::new(RwLock::new(HandshakeState::new(
            validation_ctx,
            local_domain_hi,
        ))),
        dev_profile,
        snapshot_verify_chain: true,
        exit_on_fatal_snapshot: false,
        broke_trust_test: false,
        identity,
        state_namespace,
        hello_nonce_ctr: Arc::new(AtomicU64::new(1)),
        transport_config: Arc::new(RwLock::new(TransportConfig::default())),
        shutdown_tx: Arc::new(Mutex::new(None)),
    }
}

pub fn app_from_genesis(source: &GenesisSource) -> Result<App, String> {
    app_from_genesis_in_shard(source, ShardId::A)
}

pub fn app_from_genesis_in_shard(source: &GenesisSource, shard: ShardId) -> Result<App, String> {
    app_from_genesis_shard_identity(source, shard, None, None)
}

pub(crate) fn app_from_genesis_shard_identity(
    source: &GenesisSource,
    shard: ShardId,
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
        shard,
        runtime_identity,
    ))
}

pub fn app_from_genesis_with_data(
    source: &GenesisSource,
    data_file: Option<PathBuf>,
) -> Result<App, String> {
    app_from_genesis_data_shard(source, data_file, ShardId::A)
}

pub fn app_from_genesis_data_shard(
    source: &GenesisSource,
    data_file: Option<PathBuf>,
    shard: ShardId,
) -> Result<App, String> {
    let (cfg, sks) = match source {
        GenesisSource::DevNet => dev_net(),
        GenesisSource::JsonFile { path, passphrase } => {
            load_genesis_bundle(path, Some(passphrase.as_str()))?
        }
    };
    let identity = default_runtime_identity_for_shard(shard);
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
            let mut inner = Inner {
                chain,
                pool,
                roaming_pool,
                cross_shard,
                federation: Default::default(),
                peer_account_views: std::collections::HashMap::new(),
                recent_flow: VecDeque::new(),
            };
            inner.normalize_marks_quota();
            let validation_ctx = crate::handshake::HandshakeValidationCtx {
                expected_network_id: identity.network_id.clone(),
                expected_genesis_hash: Some(hex::encode(digest(&inner.chain.cfg.state0()))),
                skew_window_ms: 30_000,
            };
            let autosnapshot_backend = data_file
                .as_ref()
                .map(|p| SnapshotBackend::JsonFile { path: p.clone() });
            return Ok(App {
                inner: Arc::new(RwLock::new(inner)),
                init: Arc::new(RwLock::new(InitState::ready(data_file.clone()))),
                data_file,
                shard,
                handshake: Arc::new(RwLock::new(HandshakeState::new(
                    validation_ctx,
                    identity.cluster_domain_hi,
                ))),
                dev_profile: true,
                snapshot_verify_chain: true,
                exit_on_fatal_snapshot: false,
                broke_trust_test: false,
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
    let mut inner = Inner {
        chain,
        pool,
        roaming_pool,
        cross_shard: CrossShardLedger::default(),
        federation: Default::default(),
        peer_account_views: std::collections::HashMap::new(),
        recent_flow: VecDeque::new(),
    };
    inner.normalize_marks_quota();
    let validation_ctx = crate::handshake::HandshakeValidationCtx {
        expected_network_id: identity.network_id.clone(),
        expected_genesis_hash: Some(hex::encode(digest(&inner.chain.cfg.state0()))),
        skew_window_ms: 30_000,
    };
    let autosnapshot_backend = data_file
        .as_ref()
        .map(|p| SnapshotBackend::JsonFile { path: p.clone() });
    Ok(App {
        inner: Arc::new(RwLock::new(inner)),
        init: Arc::new(RwLock::new(InitState::ready(data_file.clone()))),
        data_file,
        shard,
        handshake: Arc::new(RwLock::new(HandshakeState::new(
            validation_ctx,
            identity.cluster_domain_hi,
        ))),
        dev_profile: true,
        snapshot_verify_chain: true,
        exit_on_fatal_snapshot: false,
        broke_trust_test: false,
        identity,
        state_namespace,
        hello_nonce_ctr: Arc::new(AtomicU64::new(1)),
        transport_config: Arc::new(RwLock::new(TransportConfig::default())),
        autosnapshot_backend,
        shutdown_tx: Arc::new(Mutex::new(None)),
    })
}
