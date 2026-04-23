//! Devnet node library: REST `/v1/*`, router builder, seal loop for the `pwmd` binary.

use axum::{
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderValue, Method, StatusCode},
    routing::{get, post},
    Json, Router,
};
use ed25519_dalek::SigningKey;
use pwm_core::block::{hdr_hash, txs_root, Block};
use pwm_core::chain::prev_gen;
use pwm_core::genesis::GenCfg;
use pwm_core::hd::account_id_from_parts;
use pwm_core::types::Account;
use pwm_core::{dev_net, digest, validate_tx_shape, Chain, Mpool, SignedTx, State as ChainState};
use serde::Deserialize;
use serde::Serialize;
use serde::{Deserializer, Serializer};
use serde_json::Value;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path as FsPath, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tracing::{info, warn};

/// Max JSON body for `POST /v1/tx` (devnet; keeps huge payloads out of the mempool path).
pub const V1_TX_BODY_LIMIT: usize = 256 * 1024;

#[derive(Clone)]
pub struct App {
    pub inner: Arc<RwLock<Inner>>,
    init: Arc<RwLock<InitState>>,
    data_file: Option<PathBuf>,
}

pub struct Inner {
    pub chain: Chain,
    pub pool: Mpool,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InitPhase {
    Starting,
    LoadingSnapshot,
    Ready,
    ReadyDegraded,
}

impl InitPhase {
    fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::LoadingSnapshot => "loading_snapshot",
            Self::Ready => "ready",
            Self::ReadyDegraded => "ready_degraded",
        }
    }
}

#[derive(Clone, Debug)]
struct InitState {
    phase: InitPhase,
    snapshot_file: Option<PathBuf>,
    snapshot_error: Option<String>,
}

impl InitState {
    fn ready(snapshot_file: Option<PathBuf>) -> Self {
        Self {
            phase: InitPhase::Ready,
            snapshot_file,
            snapshot_error: None,
        }
    }

    fn loading(snapshot_file: Option<PathBuf>) -> Self {
        Self {
            phase: InitPhase::LoadingSnapshot,
            snapshot_file,
            snapshot_error: None,
        }
    }

    fn ready_degraded(snapshot_file: Option<PathBuf>, snapshot_error: String) -> Self {
        Self {
            phase: InitPhase::ReadyDegraded,
            snapshot_file,
            snapshot_error: Some(snapshot_error),
        }
    }

    fn is_ready(&self) -> bool {
        matches!(self.phase, InitPhase::Ready | InitPhase::ReadyDegraded)
    }
}

#[derive(Serialize)]
pub struct StatusOut {
    pub phase: &'static str,
    pub ready: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_error: Option<String>,
}

#[derive(Serialize)]
pub struct HeadOut {
    pub height: u64,
    pub tip: String,
}

#[derive(Serialize)]
pub struct AcctOut {
    pub id: String,
    pub balance_pwm: String,
    pub staked: String,
    pub marks: String,
    pub initialized: bool,
    pub nonce: u64,
}

#[derive(Serialize)]
pub struct AcctListOut {
    pub accounts: Vec<AcctOut>,
}

/// Where to load genesis + validator signing keys from.
#[derive(Clone, Debug)]
pub enum GenesisSource {
    DevNet,
    JsonFile(PathBuf),
}

/// Runtime options for `pwmd` (CLI maps here).
#[derive(Clone, Debug)]
pub struct PwmdConfig {
    pub listen: SocketAddr,
    pub genesis: GenesisSource,
    pub data_file: PathBuf,
}

impl Default for PwmdConfig {
    fn default() -> Self {
        Self {
            listen: SocketAddr::from(([127, 0, 0, 1], 3030)),
            genesis: GenesisSource::DevNet,
            data_file: PathBuf::from("pwm-data.json"),
        }
    }
}

#[derive(Deserialize)]
struct GenesisFile {
    gen_cfg: GenCfg,
    validator_seeds_hex: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct SnapshotGenesisRow {
    acct: [u8; 32],
    pubkey: [u8; 32],
    der_idx: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SnapshotData {
    version: u32,
    genesis_rows: Vec<SnapshotGenesisRow>,
    blocks: Vec<Block>,
    #[serde(serialize_with = "serialize_snapshot_state")]
    #[serde(deserialize_with = "deserialize_snapshot_state")]
    state: ChainState,
}

const SNAPSHOT_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SnapshotDataLegacyV0 {
    blocks: Vec<Block>,
    #[serde(serialize_with = "serialize_snapshot_state")]
    #[serde(deserialize_with = "deserialize_snapshot_state")]
    state: ChainState,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SnapshotStateRow {
    id: [u8; 32],
    account: SnapshotAccountWire,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SnapshotStateWire {
    accounts: Vec<SnapshotStateRow>,
    fee_pool: u128,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SnapshotAccountWire {
    signing_pubkey: [u8; 32],
    derivation_index: u32,
    balance_pwm: u128,
    staked: u128,
    marks: u128,
    initialized: bool,
    index: u32,
    flags: u32,
    nonce: u64,
}

impl From<Account> for SnapshotAccountWire {
    fn from(value: Account) -> Self {
        Self {
            signing_pubkey: value.signing_pubkey,
            derivation_index: value.derivation_index,
            balance_pwm: value.balance_pwm,
            staked: value.staked,
            marks: value.marks,
            initialized: value.initialized,
            index: value.index,
            flags: value.flags,
            nonce: value.nonce,
        }
    }
}

impl From<SnapshotAccountWire> for Account {
    fn from(value: SnapshotAccountWire) -> Self {
        Self {
            signing_pubkey: value.signing_pubkey,
            derivation_index: value.derivation_index,
            balance_pwm: value.balance_pwm,
            staked: value.staked,
            marks: value.marks,
            initialized: value.initialized,
            index: value.index,
            flags: value.flags,
            nonce: value.nonce,
        }
    }
}

fn serialize_snapshot_state<S>(state: &ChainState, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let accounts = state
        .accounts
        .iter()
        .map(|(id, account)| SnapshotStateRow {
            id: *id,
            account: account.clone().into(),
        })
        .collect();
    SnapshotStateWire {
        accounts,
        fee_pool: state.fee_pool,
    }
    .serialize(serializer)
}

fn deserialize_snapshot_state<'de, D>(deserializer: D) -> Result<ChainState, D::Error>
where
    D: Deserializer<'de>,
{
    let wire = SnapshotStateWire::deserialize(deserializer)?;
    let mut accounts = BTreeMap::new();
    for row in wire.accounts {
        if accounts.insert(row.id, row.account.into()).is_some() {
            return Err(serde::de::Error::custom(format!(
                "snapshot state contract error: duplicate account id {} in state.accounts",
                hex::encode(row.id)
            )));
        }
    }
    Ok(ChainState {
        accounts,
        fee_pool: wire.fee_pool,
    })
}

fn hex32_from_hex(s: &str) -> Result<[u8; 32], String> {
    let v = hex::decode(s.trim()).map_err(|e| format!("hex: {e}"))?;
    if v.len() != 32 {
        return Err("need 32-byte master seed (hex)".into());
    }
    let mut a = [0u8; 32];
    a.copy_from_slice(&v);
    Ok(a)
}

/// Load `gen_cfg` + validator master seeds (same derivation as `dev_net`: SLIP-0010 `m/0'/0'`).
pub fn load_genesis_bundle(path: &std::path::Path) -> Result<(GenCfg, Vec<SigningKey>), String> {
    let txt = std::fs::read_to_string(path).map_err(|e| format!("read genesis: {e}"))?;
    let b: GenesisFile =
        serde_json::from_str(&txt).map_err(|e| format!("parse genesis JSON: {e}"))?;
    if b.validator_seeds_hex.len() != b.gen_cfg.rows.len() {
        return Err("validator_seeds_hex length must match gen_cfg.rows".into());
    }
    let mut sks = Vec::new();
    for (i, hex_str) in b.validator_seeds_hex.iter().enumerate() {
        let seed = hex32_from_hex(hex_str)?;
        let sk_bytes = slip10_ed25519::derive_ed25519_private_key(&seed, &[0, 0]);
        let sk = SigningKey::from_bytes(&sk_bytes);
        let pk = sk.verifying_key().to_bytes();
        let row = &b.gen_cfg.rows[i];
        if pk != row.pubkey {
            return Err(format!(
                "validator seed {i}: derived pubkey does not match gen_cfg.rows[{i}].pubkey"
            ));
        }
        let aid = account_id_from_parts(&pk, row.der_idx);
        if aid != row.acct {
            return Err(format!(
                "validator seed {i}: derived account id does not match gen_cfg.rows[{i}].acct"
            ));
        }
        sks.push(sk);
    }
    Ok((b.gen_cfg, sks))
}

/// CORS: permissive only when binding to loopback; otherwise require `PWM_CORS_ORIGINS`.
pub fn cors_for_listen(listen: SocketAddr) -> Result<CorsLayer, String> {
    if listen.ip().is_loopback() {
        return Ok(CorsLayer::permissive());
    }
    let raw = std::env::var("PWM_CORS_ORIGINS").unwrap_or_default();
    let mut origins = Vec::new();
    for part in raw.split(',') {
        let t = part.trim();
        if t.is_empty() {
            continue;
        }
        let hv = HeaderValue::from_str(t)
            .map_err(|e| format!("PWM_CORS_ORIGINS invalid origin {t:?}: {e}"))?;
        origins.push(hv);
    }
    if origins.is_empty() {
        return Err(
            "non-loopback --listen requires PWM_CORS_ORIGINS (comma-separated allow_origins)"
                .into(),
        );
    }
    Ok(CorsLayer::new()
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any)
        .allow_origin(AllowOrigin::list(origins)))
}

fn hex32(b: &[u8; 32]) -> String {
    hex::encode(b)
}

fn parse_id(s: &str) -> Result<[u8; 32], ()> {
    let v = hex::decode(s.trim()).map_err(|_| ())?;
    if v.len() != 32 {
        return Err(());
    }
    let mut a = [0u8; 32];
    a.copy_from_slice(&v);
    Ok(a)
}

fn snapshot_genesis_rows(cfg: &GenCfg) -> Vec<SnapshotGenesisRow> {
    cfg.rows
        .iter()
        .map(|r| SnapshotGenesisRow {
            acct: r.acct,
            pubkey: r.pubkey,
            der_idx: r.der_idx,
        })
        .collect()
}

fn validate_snapshot(snapshot: &SnapshotData, cfg: &GenCfg) -> Result<(), String> {
    if cfg.rows.is_empty() {
        return Err("snapshot validation error: genesis config has zero validator rows".into());
    }
    if snapshot.version != SNAPSHOT_VERSION {
        return Err(format!(
            "snapshot version mismatch: got {}, expected {}",
            snapshot.version, SNAPSHOT_VERSION
        ));
    }
    if snapshot.blocks.len() > u64::MAX as usize {
        return Err(
            "snapshot chain mismatch: blocks length exceeds supported u64 height range".into(),
        );
    }
    let want = snapshot_genesis_rows(cfg);
    if snapshot.genesis_rows.len() != want.len() {
        return Err(format!(
            "snapshot genesis mismatch: rows {} != {}",
            snapshot.genesis_rows.len(),
            want.len()
        ));
    }
    for (i, (got, exp)) in snapshot.genesis_rows.iter().zip(want.iter()).enumerate() {
        if got.pubkey != exp.pubkey || got.acct != exp.acct || got.der_idx != exp.der_idx {
            return Err(format!("snapshot genesis mismatch at row {i}"));
        }
    }
    let mut prev = prev_gen();
    let mut replay_state = cfg.state0();
    for (i, blk) in snapshot.blocks.iter().enumerate() {
        let h = (i as u64) + 1;
        if blk.hdr.height != h {
            return Err(format!(
                "snapshot chain mismatch: block[{i}] has height {}, expected {h}",
                blk.hdr.height
            ));
        }
        if blk.hdr.prev_hash != prev {
            return Err(format!(
                "snapshot chain mismatch: block[{i}] prev_hash does not match previous header hash"
            ));
        }
        let want_tx_root = txs_root(&blk.txs);
        if blk.hdr.tx_root != want_tx_root {
            return Err(format!(
                "snapshot chain mismatch: block[{i}] tx_root is invalid"
            ));
        }
        let want_prod_idx = ((h - 1) as usize % cfg.rows.len()) as u32;
        if blk.hdr.prod_idx != want_prod_idx {
            return Err(format!(
                "snapshot chain mismatch: block[{i}] prod_idx {}, expected {}",
                blk.hdr.prod_idx, want_prod_idx
            ));
        }
        let prod = cfg
            .rows
            .get(blk.hdr.prod_idx as usize)
            .ok_or_else(|| format!("snapshot chain mismatch: block[{i}] prod_idx out of range"))?;
        if !blk.hdr.verify_sig(&prod.pubkey) {
            return Err(format!(
                "snapshot chain mismatch: block[{i}] has invalid producer signature"
            ));
        }
        for (tx_i, tx) in blk.txs.iter().enumerate() {
            replay_state.apply_tx(tx).map_err(|e| {
                format!(
                    "snapshot chain mismatch: block[{i}] tx[{tx_i}] is invalid during replay: {e}"
                )
            })?;
        }
        replay_state.accrue_marks(cfg.marks_coeff);
        let prod_acct = cfg.prod_acct(blk.hdr.prod_idx);
        replay_state.reward_producer(&prod_acct, cfg.block_reward);
        let replay_root = digest(&replay_state);
        if blk.hdr.state_root != replay_root {
            return Err(format!(
                "snapshot chain mismatch: block[{i}] state_root does not match replayed state"
            ));
        }
        prev = hdr_hash(&blk.hdr);
    }
    for (id, account) in snapshot.state.accounts.iter() {
        let derived = account_id_from_parts(&account.signing_pubkey, account.derivation_index);
        if derived != *id {
            return Err(format!(
                "snapshot state mismatch: account id {} does not match signing_pubkey/derivation_index",
                hex::encode(id)
            ));
        }
    }
    if let Some(last) = snapshot.blocks.last() {
        if last.hdr.height != snapshot.blocks.len() as u64 {
            return Err(format!(
                "snapshot chain mismatch: tip height {} does not match blocks length {}",
                last.hdr.height,
                snapshot.blocks.len()
            ));
        }
        let st_root = digest(&snapshot.state);
        if last.hdr.state_root != st_root {
            return Err("snapshot state root mismatch with tip block".into());
        }
        let replay_root = digest(&replay_state);
        if replay_root != st_root {
            return Err(
                "snapshot state mismatch: persisted state does not match replayed chain state"
                    .into(),
            );
        }
    } else {
        let genesis_root = digest(&cfg.state0());
        let st_root = digest(&snapshot.state);
        if st_root != genesis_root {
            return Err(
                "snapshot state mismatch: empty block history must match genesis state".into(),
            );
        }
    }
    Ok(())
}

fn load_snapshot(path: &FsPath, cfg: &GenCfg) -> Result<Option<SnapshotData>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let txt = std::fs::read_to_string(path).map_err(|e| format!("read snapshot: {e}"))?;
    let raw: Value = serde_json::from_str(&txt).map_err(|e| format!("parse snapshot JSON: {e}"))?;
    let obj = raw
        .as_object()
        .ok_or_else(|| "parse snapshot JSON: root must be an object".to_string())?;
    let has_version = obj.contains_key("version");
    let has_genesis_rows = obj.contains_key("genesis_rows");
    let has_blocks = obj.contains_key("blocks");
    let has_state = obj.contains_key("state");
    let snap: SnapshotData = if has_version || has_genesis_rows {
        if has_version != has_genesis_rows {
            return Err(
                "snapshot contract error: canonical snapshot requires both 'version' and 'genesis_rows'; regenerate snapshot from current pwmd"
                    .into(),
            );
        }
        if !has_blocks || !has_state {
            return Err(
                "snapshot contract error: canonical snapshot must contain both 'blocks' and 'state'; regenerate snapshot from current pwmd"
                    .into(),
            );
        }
        let mut canonical = serde_json::Map::new();
        canonical.insert(
            "version".to_string(),
            obj.get("version")
                .ok_or_else(|| {
                    "snapshot contract error: canonical snapshot missing required field 'version'"
                        .to_string()
                })?
                .clone(),
        );
        canonical.insert(
            "genesis_rows".to_string(),
            obj.get("genesis_rows")
                .ok_or_else(|| {
                    "snapshot contract error: canonical snapshot missing required field 'genesis_rows'"
                        .to_string()
                })?
                .clone(),
        );
        canonical.insert(
            "blocks".to_string(),
            obj.get("blocks")
                .ok_or_else(|| {
                    "snapshot contract error: canonical snapshot missing required field 'blocks'"
                        .to_string()
                })?
                .clone(),
        );
        canonical.insert(
            "state".to_string(),
            obj.get("state")
                .ok_or_else(|| {
                    "snapshot contract error: canonical snapshot missing required field 'state'"
                        .to_string()
                })?
                .clone(),
        );
        let dropped = obj
            .keys()
            .filter(|k| *k != "version" && *k != "genesis_rows" && *k != "blocks" && *k != "state")
            .cloned()
            .collect::<Vec<_>>();
        if !dropped.is_empty() {
            warn!(
                "snapshot canonical-only mode: ignoring non-canonical top-level fields: {}",
                dropped.join(", ")
            );
        }
        serde_json::from_value(Value::Object(canonical))
            .map_err(|e| format!("parse canonical snapshot JSON: {e}"))?
    } else {
        if !has_blocks || !has_state {
            return Err(
                "snapshot legacy migration error: legacy snapshot must contain both 'blocks' and 'state'; regenerate snapshot from current pwmd"
                    .into(),
            );
        }
        let dropped = obj
            .keys()
            .filter(|k| *k != "blocks" && *k != "state")
            .cloned()
            .collect::<Vec<_>>();
        if !dropped.is_empty() {
            warn!(
                "snapshot legacy migration: ignoring non-canonical top-level fields: {}",
                dropped.join(", ")
            );
        }
        let mut legacy_obj = serde_json::Map::new();
        legacy_obj.insert(
            "blocks".to_string(),
            obj.get("blocks")
                .ok_or_else(|| {
                    "snapshot legacy migration error: legacy snapshot missing required field 'blocks'"
                        .to_string()
                })?
                .clone(),
        );
        legacy_obj.insert(
            "state".to_string(),
            obj.get("state")
                .ok_or_else(|| {
                    "snapshot legacy migration error: legacy snapshot missing required field 'state'"
                        .to_string()
                })?
                .clone(),
        );
        let legacy: SnapshotDataLegacyV0 = serde_json::from_value(Value::Object(legacy_obj))
            .map_err(|e| format!("parse legacy snapshot JSON: {e}"))?;
        SnapshotData {
            version: SNAPSHOT_VERSION,
            genesis_rows: snapshot_genesis_rows(cfg),
            blocks: legacy.blocks,
            state: legacy.state,
        }
    };
    validate_snapshot(&snap, cfg)?;
    Ok(Some(snap))
}

fn save_snapshot(path: &FsPath, inner: &Inner) -> Result<(), String> {
    let snap = SnapshotData {
        version: SNAPSHOT_VERSION,
        genesis_rows: snapshot_genesis_rows(&inner.chain.cfg),
        blocks: inner.chain.blocks.clone(),
        state: inner.chain.st.clone(),
    };
    let txt = serde_json::to_string_pretty(&snap).map_err(|e| format!("encode snapshot: {e}"))?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| format!("create snapshot dir: {e}"))?;
        }
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, txt).map_err(|e| format!("write snapshot temp: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("replace snapshot: {e}"))?;
    Ok(())
}

async fn v1_head(State(a): State<App>) -> Result<Json<HeadOut>, (StatusCode, String)> {
    ensure_ready(&a).await?;
    let g = a.inner.read().await;
    Ok(Json(HeadOut {
        height: g.chain.tip_h(),
        tip: hex32(&g.chain.tip_hash()),
    }))
}

async fn v1_accounts(State(a): State<App>) -> Result<Json<AcctListOut>, (StatusCode, String)> {
    ensure_ready(&a).await?;
    let g = a.inner.read().await;
    let mut accounts = Vec::new();
    for (id, ac) in g.chain.st.accounts.iter() {
        accounts.push(AcctOut {
            id: hex32(id),
            balance_pwm: ac.balance_pwm.to_string(),
            staked: ac.staked.to_string(),
            marks: ac.marks.to_string(),
            initialized: ac.initialized,
            nonce: ac.nonce,
        });
    }
    Ok(Json(AcctListOut { accounts }))
}

async fn v1_account(
    State(a): State<App>,
    Path(id): Path<String>,
) -> Result<Json<AcctOut>, (StatusCode, String)> {
    ensure_ready(&a).await?;
    let key =
        parse_id(&id).map_err(|_| (StatusCode::BAD_REQUEST, "invalid account id".to_string()))?;
    let g = a.inner.read().await;
    let ac = g
        .chain
        .st
        .get(&key)
        .ok_or((StatusCode::NOT_FOUND, "account not found".to_string()))?;
    Ok(Json(AcctOut {
        id: hex32(&key),
        balance_pwm: ac.balance_pwm.to_string(),
        staked: ac.staked.to_string(),
        marks: ac.marks.to_string(),
        initialized: ac.initialized,
        nonce: ac.nonce,
    }))
}

async fn v1_tx(
    State(a): State<App>,
    Json(tx): Json<SignedTx>,
) -> Result<StatusCode, (StatusCode, String)> {
    ensure_ready(&a).await?;
    validate_tx_shape(&tx).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("tx validation failed: {e}"),
        )
    })?;
    let mut g = a.inner.write().await;
    g.pool.push(tx).map_err(|_| {
        (
            StatusCode::INSUFFICIENT_STORAGE,
            "mempool is full".to_string(),
        )
    })?;
    if let Some(path) = a.data_file.as_deref() {
        if let Err(e) = save_snapshot(path, &g) {
            warn!("snapshot save after tx accepted failed: {}", e);
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn v1_status(State(a): State<App>) -> Json<StatusOut> {
    let s = a.init.read().await;
    Json(StatusOut {
        phase: s.phase.as_str(),
        ready: s.is_ready(),
        snapshot_file: s.snapshot_file.as_ref().map(|p| p.display().to_string()),
        snapshot_error: s.snapshot_error.clone(),
    })
}

async fn ensure_ready(app: &App) -> Result<(), (StatusCode, String)> {
    let s = app.init.read().await;
    if s.is_ready() {
        return Ok(());
    }
    Err((
        StatusCode::SERVICE_UNAVAILABLE,
        format!("node is not ready (phase={})", s.phase.as_str()),
    ))
}

/// Fresh in-memory app (`dev_net` genesis).
pub fn app_from_dev_net() -> App {
    let (cfg, sks) = dev_net();
    app_from_chain_boot(cfg, sks, None)
}

fn app_from_chain_boot(cfg: GenCfg, sks: Vec<SigningKey>, data_file: Option<PathBuf>) -> App {
    let chain = Chain::boot(cfg, sks);
    let pool = Mpool::new(4096);
    App {
        inner: Arc::new(RwLock::new(Inner { chain, pool })),
        init: Arc::new(RwLock::new(InitState::ready(data_file.clone()))),
        data_file,
    }
}

/// Boot from genesis source (file or built-in dev net).
pub fn app_from_genesis(source: &GenesisSource) -> Result<App, String> {
    app_from_genesis_with_data(source, None)
}

/// Boot from genesis and load chain snapshot when `data_file` exists.
pub fn app_from_genesis_with_data(
    source: &GenesisSource,
    data_file: Option<PathBuf>,
) -> Result<App, String> {
    let (cfg, sks) = match source {
        GenesisSource::DevNet => dev_net(),
        GenesisSource::JsonFile(p) => load_genesis_bundle(p)?,
    };
    let mut chain = Chain::boot(cfg, sks);
    if let Some(path) = data_file.as_deref() {
        if let Some(snap) = load_snapshot(path, &chain.cfg)? {
            chain.blocks = snap.blocks;
            chain.st = snap.state;
            info!("loaded snapshot from {}", path.display());
        }
    }
    let pool = Mpool::new(4096);
    Ok(App {
        inner: Arc::new(RwLock::new(Inner { chain, pool })),
        init: Arc::new(RwLock::new(InitState::ready(data_file.clone()))),
        data_file,
    })
}

/// After `with_state(app)` this is `Router<()>` (see axum `with_state` docs).
pub fn router(app: App, cors: CorsLayer) -> Router {
    Router::new()
        .route("/v1/status", get(v1_status))
        .route("/v1/head", get(v1_head))
        .route("/v1/accounts", get(v1_accounts))
        .route("/v1/account/:id", get(v1_account))
        .route("/v1/tx", post(v1_tx))
        .layer(DefaultBodyLimit::max(V1_TX_BODY_LIMIT))
        .layer(cors)
        .with_state(app)
}

/// Spawn the 2s PoA seal loop (same as production binary).
pub fn spawn_seal_loop(app: App) {
    tokio::spawn(async move {
        let mut iv = tokio::time::interval(std::time::Duration::from_secs(2));
        loop {
            iv.tick().await;
            if !app.init.read().await.is_ready() {
                continue;
            }
            let mut g = app.inner.write().await;
            let txs = g.pool.take(64);
            match g.chain.seal(txs) {
                Ok(()) => {
                    info!("sealed height={}", g.chain.tip_h());
                    if let Some(path) = app.data_file.as_deref() {
                        if let Err(e) = save_snapshot(path, &g) {
                            warn!("snapshot save after seal failed: {}", e);
                        }
                    }
                }
                Err((e, txs)) => {
                    warn!("seal skip: {}", e);
                    g.pool.prepend_block(txs);
                }
            }
        }
    });
}

fn spawn_snapshot_loader(app: App, data_file: PathBuf) {
    tokio::spawn(async move {
        {
            let mut st = app.init.write().await;
            *st = InitState::loading(Some(data_file.clone()));
        }
        info!("snapshot loading started: {}", data_file.display());
        eprintln!(
            "pwmd startup phase: loading_snapshot ({})",
            data_file.display()
        );
        let cfg = {
            let g = app.inner.read().await;
            g.chain.cfg.clone()
        };
        match load_snapshot(&data_file, &cfg) {
            Ok(Some(snap)) => {
                let mut g = app.inner.write().await;
                g.chain.blocks = snap.blocks;
                g.chain.st = snap.state;
                drop(g);
                let mut st = app.init.write().await;
                *st = InitState::ready(Some(data_file.clone()));
                info!("snapshot loaded from {}", data_file.display());
                eprintln!("pwmd startup phase: ready (snapshot loaded)");
            }
            Ok(None) => {
                let mut st = app.init.write().await;
                *st = InitState::ready(Some(data_file.clone()));
                info!("snapshot file not found, fallback to genesis state");
                eprintln!("pwmd startup phase: ready (no snapshot file)");
            }
            Err(e) => {
                warn!("snapshot load failed (fallback to genesis state): {}", e);
                let mut st = app.init.write().await;
                *st = InitState::ready_degraded(Some(data_file.clone()), e.clone());
                eprintln!("pwmd startup phase: ready_degraded (snapshot error: {e})");
            }
        }
    });
}

/// Run with explicit config (listen address, genesis). Returns on server error.
pub async fn run_with(config: PwmdConfig) -> Result<(), String> {
    let cors = cors_for_listen(config.listen)?;
    let app = app_from_genesis(&config.genesis)?;
    {
        let mut st = app.init.write().await;
        *st = InitState {
            phase: InitPhase::Starting,
            snapshot_file: Some(config.data_file.clone()),
            snapshot_error: None,
        };
    }
    spawn_snapshot_loader(app.clone(), config.data_file.clone());
    spawn_seal_loop(app.clone());
    let r = router(app, cors);
    let listener = tokio::net::TcpListener::bind(config.listen)
        .await
        .map_err(|e| format!("bind {}: {e}", config.listen))?;
    let listen_addr = listener
        .local_addr()
        .map_err(|e| format!("local_addr {}: {e}", config.listen))?;
    info!("pwmd listen http://{}", listen_addr);
    eprintln!("pwmd listening on http://{}", listen_addr);
    axum::serve(listener, r)
        .await
        .map_err(|e| format!("serve: {e}"))?;
    Ok(())
}

/// Default `127.0.0.1:3030` + `dev_net`.
pub async fn run() -> Result<(), String> {
    run_with(PwmdConfig::default()).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use ed25519_dalek::SigningKey;
    use pwm_core::hd::{account_id_from_parts, domain_of_account_id};
    use pwm_core::tx::{SignedTx, TxBody};
    use slip10_ed25519::derive_ed25519_private_key;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tower::ServiceExt;

    fn user_sk(seed: &[u8; 32]) -> (SigningKey, u32, pwm_core::AccountId) {
        let sk_bytes = derive_ed25519_private_key(seed, &[0, 0]);
        let sk = SigningKey::from_bytes(&sk_bytes);
        let i = 0u32;
        let pk = sk.verifying_key().to_bytes();
        let aid = account_id_from_parts(&pk, i);
        (sk, i, aid)
    }

    fn router_dev(app: App) -> Router {
        router(app, CorsLayer::permissive())
    }

    fn temp_path(name: &str) -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("pwmd_{name}_{ts}.json"))
    }

    #[test]
    fn cors_loopback_is_permissive() {
        let a = SocketAddr::from(([127, 0, 0, 1], 3030));
        assert!(cors_for_listen(a).is_ok());
    }

    #[test]
    fn cors_non_loopback_requires_origins_env() {
        let key = "PWM_CORS_ORIGINS";
        let old = std::env::var(key).ok();
        std::env::remove_var(key);
        let a = SocketAddr::from(([0, 0, 0, 0], 3030));
        let r = cors_for_listen(a);
        assert!(r.is_err());
        if let Some(v) = old {
            std::env::set_var(key, v);
        }
    }

    #[test]
    fn genesis_json_roundtrip_dev_seed() {
        let (g, sks) = dev_net();
        let bundle = serde_json::json!({
            "gen_cfg": g,
            "validator_seeds_hex": vec![hex::encode([99u8; 32])],
        });
        let p = std::env::temp_dir().join("pwmd_genesis_test_roundtrip.json");
        std::fs::write(&p, serde_json::to_string_pretty(&bundle).unwrap()).unwrap();
        let (g2, sk2) = load_genesis_bundle(&p).unwrap();
        assert_eq!(g, g2);
        assert_eq!(sk2.len(), sks.len());
        assert_eq!(
            sk2[0].verifying_key().to_bytes(),
            sks[0].verifying_key().to_bytes()
        );
        let _ = std::fs::remove_file(&p);
    }

    #[tokio::test]
    async fn v1_head_returns_tip_json() {
        let svc = router_dev(app_from_dev_net()).into_service();
        let res = svc
            .oneshot(Request::get("/v1/head").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["height"].as_u64(), Some(0));
        assert!(v.get("tip").and_then(|t| t.as_str()).map(|s| !s.is_empty()) == Some(true));
    }

    #[tokio::test]
    async fn v1_status_reports_loading_and_head_returns_503() {
        let app = app_from_dev_net();
        {
            let mut st = app.init.write().await;
            *st = InitState::loading(Some(PathBuf::from("pwm-data.json")));
        }
        let svc = router_dev(app).into_service();
        let status = svc
            .clone()
            .oneshot(Request::get("/v1/status").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(status.status(), StatusCode::OK);
        let bytes = to_bytes(status.into_body(), 64 * 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["phase"], "loading_snapshot");
        assert_eq!(v["ready"], false);

        let head = svc
            .oneshot(Request::get("/v1/head").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(head.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn v1_status_reports_ready_degraded_after_snapshot_error() {
        let app = app_from_dev_net();
        {
            let mut st = app.init.write().await;
            *st = InitState::ready_degraded(
                Some(PathBuf::from("pwm-data.json")),
                "snapshot parse failed".to_string(),
            );
        }
        let svc = router_dev(app).into_service();
        let status = svc
            .clone()
            .oneshot(Request::get("/v1/status").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(status.status(), StatusCode::OK);
        let bytes = to_bytes(status.into_body(), 64 * 1024).await.unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["phase"], "ready_degraded");
        assert_eq!(v["ready"], true);
        assert_eq!(v["snapshot_error"], "snapshot parse failed");

        let head = svc
            .oneshot(Request::get("/v1/head").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(head.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn v1_tx_rejects_domain_mismatch() {
        let (sk, i, aid) = user_sk(&[17u8; 32]);
        let d_ok = domain_of_account_id(&aid);
        let d_bad = if d_ok == u16::MAX { 0 } else { d_ok + 1 };
        let tx = SignedTx::sign_body(&sk, d_bad, i, 0, TxBody::Init { index: 0, flags: 0 });
        let svc = router_dev(app_from_dev_net()).into_service();
        let res = svc
            .oneshot(
                Request::post("/v1/tx")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&tx).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn v1_tx_accepts_regulatory_lo_zero_init() {
        let (sk, i, aid) = user_sk(&[29u8; 32]);
        let mut domain = domain_of_account_id(&aid);
        domain = (domain & 0xFF00) | 0x00;
        let tx = SignedTx::sign_body(&sk, domain, i, 0, TxBody::Init { index: 0, flags: 0 });
        let svc = router_dev(app_from_dev_net()).into_service();
        let res = svc
            .oneshot(
                Request::post("/v1/tx")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&tx).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(res.into_body(), 64 * 1024).await.unwrap();
        let text = String::from_utf8_lossy(&body);
        assert!(text.contains("domain mismatch"));
    }

    #[tokio::test]
    async fn v1_tx_accepts_signed_init() {
        let (sk, i, aid) = user_sk(&[23u8; 32]);
        let dom = domain_of_account_id(&aid);
        let tx = SignedTx::sign_body(&sk, dom, i, 0, TxBody::Init { index: 1, flags: 0 });
        let app = app_from_dev_net();
        let svc = router_dev(app.clone()).into_service();
        let res = svc
            .oneshot(
                Request::post("/v1/tx")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_vec(&tx).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);
        let g = app.inner.read().await;
        assert_eq!(g.pool.len(), 1);
    }

    #[tokio::test]
    async fn v1_tx_rejects_oversized_body() {
        let body = vec![b'x'; V1_TX_BODY_LIMIT + 1024];
        let svc = router_dev(app_from_dev_net()).into_service();
        let res = svc
            .oneshot(
                Request::post("/v1/tx")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[test]
    fn snapshot_roundtrip_blocks_and_state() {
        let (cfg, sks) = dev_net();
        let mut chain = Chain::boot(cfg.clone(), sks);
        chain.seal(vec![]).expect("seal #1");
        chain.seal(vec![]).expect("seal #2");
        let inner = Inner {
            chain,
            pool: Mpool::new(16),
        };
        let p = temp_path("snapshot_roundtrip");
        save_snapshot(&p, &inner).expect("save");
        let snap = load_snapshot(&p, &cfg).expect("load").expect("exists");
        assert_eq!(snap.blocks.len(), 2);
        assert_eq!(digest(&snap.state), digest(&inner.chain.st));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn snapshot_rejects_mismatched_genesis() {
        let (cfg, sks) = dev_net();
        let chain = Chain::boot(cfg.clone(), sks);
        let inner = Inner {
            chain,
            pool: Mpool::new(16),
        };
        let p = temp_path("snapshot_mismatch");
        save_snapshot(&p, &inner).expect("save");
        let mut bad_cfg = cfg.clone();
        bad_cfg.rows[0].der_idx += 1;
        let err = load_snapshot(&p, &bad_cfg).expect_err("must reject mismatched genesis");
        assert!(err.contains("snapshot genesis mismatch"));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn snapshot_legacy_v0_is_migrated_safely() {
        let (cfg, sks) = dev_net();
        let mut chain = Chain::boot(cfg.clone(), sks);
        chain.seal(vec![]).expect("seal");
        let inner = Inner {
            chain,
            pool: Mpool::new(16),
        };
        let p = temp_path("snapshot_legacy_v0");
        save_snapshot(&p, &inner).expect("save");
        let raw = std::fs::read_to_string(&p).expect("read");
        let mut v: serde_json::Value = serde_json::from_str(&raw).expect("json");
        let o = v.as_object_mut().expect("object");
        o.remove("version");
        o.remove("genesis_rows");
        o.insert(
            "hints".to_string(),
            serde_json::json!({"operator": "ignore-me"}),
        );
        std::fs::write(&p, serde_json::to_string_pretty(&v).expect("encode")).expect("write");
        let snap = load_snapshot(&p, &cfg).expect("load").expect("exists");
        assert_eq!(snap.version, SNAPSHOT_VERSION);
        assert_eq!(snap.genesis_rows, snapshot_genesis_rows(&cfg));
        assert_eq!(snap.blocks.len(), 1);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn snapshot_rejects_non_canonical_incomplete_contract() {
        let (cfg, sks) = dev_net();
        let chain = Chain::boot(cfg.clone(), sks);
        let inner = Inner {
            chain,
            pool: Mpool::new(16),
        };
        let p = temp_path("snapshot_incomplete_contract");
        save_snapshot(&p, &inner).expect("save");
        let raw = std::fs::read_to_string(&p).expect("read");
        let mut v: serde_json::Value = serde_json::from_str(&raw).expect("json");
        v.as_object_mut().expect("object").remove("genesis_rows");
        std::fs::write(&p, serde_json::to_string_pretty(&v).expect("encode")).expect("write");
        let err = load_snapshot(&p, &cfg).expect_err("must reject");
        assert!(err.contains("canonical snapshot requires both 'version' and 'genesis_rows'"));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn snapshot_ignores_non_canonical_derived_fields() {
        let (cfg, sks) = dev_net();
        let mut chain = Chain::boot(cfg.clone(), sks);
        chain.seal(vec![]).expect("seal");
        let want_state_digest = digest(&chain.st);
        let inner = Inner {
            chain,
            pool: Mpool::new(16),
        };
        let p = temp_path("snapshot_ignores_derived");
        save_snapshot(&p, &inner).expect("save");
        let raw = std::fs::read_to_string(&p).expect("read");
        let mut v: serde_json::Value = serde_json::from_str(&raw).expect("json");
        let o = v.as_object_mut().expect("object");
        o.insert(
            "pretty".to_string(),
            serde_json::json!({"tip": "human-readable-only"}),
        );
        o["state"].as_object_mut().expect("state object").insert(
            "hints".to_string(),
            serde_json::json!({"operator": "ignore-me"}),
        );
        std::fs::write(&p, serde_json::to_string_pretty(&v).expect("encode")).expect("write");
        let snap = load_snapshot(&p, &cfg).expect("load").expect("exists");
        assert_eq!(digest(&snap.state), want_state_digest);
        assert_eq!(snap.blocks.len(), 1);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn snapshot_rejects_invalid_prev_hash_chain() {
        let (cfg, sks) = dev_net();
        let mut chain = Chain::boot(cfg.clone(), sks);
        chain.seal(vec![]).expect("seal #1");
        chain.seal(vec![]).expect("seal #2");
        let inner = Inner {
            chain,
            pool: Mpool::new(16),
        };
        let p = temp_path("snapshot_bad_prev");
        save_snapshot(&p, &inner).expect("save");
        let raw = std::fs::read_to_string(&p).expect("read");
        let mut v: serde_json::Value = serde_json::from_str(&raw).expect("json");
        let blocks = v["blocks"].as_array_mut().expect("blocks");
        let bad = blocks[1]["hdr"]["prev_hash"]
            .as_array_mut()
            .expect("prev_hash bytes");
        bad[0] = serde_json::json!(255u8);
        std::fs::write(&p, serde_json::to_string_pretty(&v).expect("encode")).expect("write");
        let err = load_snapshot(&p, &cfg).expect_err("must reject");
        assert!(err.contains("prev_hash"));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn snapshot_rejects_tampered_block_header() {
        let (cfg, sks) = dev_net();
        let mut chain = Chain::boot(cfg.clone(), sks);
        chain.seal(vec![]).expect("seal #1");
        chain.seal(vec![]).expect("seal #2");
        let inner = Inner {
            chain,
            pool: Mpool::new(16),
        };
        let p = temp_path("snapshot_tampered_header");
        save_snapshot(&p, &inner).expect("save");
        let raw = std::fs::read_to_string(&p).expect("read");
        let mut v: serde_json::Value = serde_json::from_str(&raw).expect("json");
        let bad = v["blocks"][1]["hdr"]["state_root"]
            .as_array_mut()
            .expect("state_root bytes");
        bad[0] = serde_json::json!(42u8);
        std::fs::write(&p, serde_json::to_string_pretty(&v).expect("encode")).expect("write");
        let err = load_snapshot(&p, &cfg).expect_err("must reject");
        assert!(err.contains("invalid producer signature"));
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn snapshot_rejects_duplicate_state_account_ids() {
        let (cfg, sks) = dev_net();
        let mut chain = Chain::boot(cfg.clone(), sks);
        chain.seal(vec![]).expect("seal");
        let inner = Inner {
            chain,
            pool: Mpool::new(16),
        };
        let p = temp_path("snapshot_duplicate_state_ids");
        save_snapshot(&p, &inner).expect("save");
        let raw = std::fs::read_to_string(&p).expect("read");
        let mut v: serde_json::Value = serde_json::from_str(&raw).expect("json");
        let accounts = v["state"]["accounts"]
            .as_array_mut()
            .expect("state.accounts");
        let first = accounts.first().expect("at least one account").clone();
        accounts.push(first);
        std::fs::write(&p, serde_json::to_string_pretty(&v).expect("encode")).expect("write");
        let err = load_snapshot(&p, &cfg).expect_err("must reject duplicate state account ids");
        assert!(err.contains("duplicate account id"));
        let _ = std::fs::remove_file(&p);
    }
}
