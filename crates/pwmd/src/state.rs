//! Tokio-guarded node state: chain, mempool, roaming pool, handshake snapshot.

use crate::identity::RuntimeIdentity;
use crate::ledger::CrossShardLedger;
use crate::roaming::RoamingPool;
use crate::transport::HandshakeState;
use crate::TransportConfig;
use pwm_core::tx::TxBody;
use pwm_core::{Chain, Mpool, SignedTx};
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct App {
    pub inner: Arc<RwLock<Inner>>,
    pub(crate) init: Arc<RwLock<InitState>>,
    pub(crate) data_file: Option<PathBuf>,
    /// Autosnapshot target (explicit CH or JSON beside `data_file`).
    pub(crate) autosnapshot_backend: Option<crate::snapshot::SnapshotBackend>,
    pub(crate) shard: crate::identity::ShardId,
    pub(crate) handshake: Arc<RwLock<HandshakeState>>,
    pub(crate) dev_profile: bool,
    /// When true, JsonFile epoch snapshots run full chain replay on load. Production default is false (trust checkpoint + tail) via `PwmdConfig`.
    pub(crate) snapshot_verify_chain: bool,
    /// Fatal snapshot errors call `process::exit` when true (daemon); tests keep this false.
    pub(crate) exit_on_fatal_snapshot: bool,
    /// Debug: advertise a fake genesis digest in transport `NodeHello` so honest peers reject handshakes.
    pub(crate) broke_trust_test: bool,
    pub(crate) identity: RuntimeIdentity,
    pub(crate) state_namespace: String,
    pub(crate) hello_nonce_ctr: Arc<AtomicU64>,
    pub(crate) transport_config: Arc<RwLock<TransportConfig>>,
    /// One-shot sender wired in `run_with`; used by `POST /v1/shutdown` for graceful HTTP stop.
    pub(crate) shutdown_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
}

pub struct Inner {
    pub chain: Chain,
    pub pool: Mpool,
    pub(crate) roaming_pool: RoamingPool,
    pub(crate) cross_shard: CrossShardLedger,
    pub(crate) federation: crate::federation::FederationTable,
    pub(crate) peer_account_views: HashMap<pwm_core::AccountId, PeerAccountView>,
    pub(crate) recent_flow: VecDeque<FlowTraceRow>,
}

#[derive(Clone, Debug, Serialize, serde::Deserialize)]
pub(crate) struct PeerAccountViewWire {
    pub(crate) id: pwm_core::AccountId,
    pub(crate) domain_hi: u8,
    #[serde(deserialize_with = "crate::wire_serde::de_u128_compat")]
    pub(crate) balance_pwm: u128,
    pub(crate) initialized: bool,
    pub(crate) nonce: u64,
    pub(crate) observed_at_ms: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct PeerAccountView {
    pub(crate) balance_pwm: u128,
    pub(crate) initialized: bool,
    pub(crate) nonce: u64,
    pub(crate) observed_at_ms: u64,
    pub(crate) source_node_id: String,
}

impl Inner {
    pub(crate) fn normalize_marks_quota(&mut self) {
        self.chain.st.normalize_marks_quota();
    }

    pub(crate) fn push_flow(&mut self, row: FlowTraceRow) {
        const RECENT_FLOW_CAP: usize = 256;
        self.recent_flow.push_back(row);
        while self.recent_flow.len() > RECENT_FLOW_CAP {
            self.recent_flow.pop_front();
        }
    }

    pub(crate) fn record_cross_shard_tx(&mut self, tx: &SignedTx, height: u64) {
        match &tx.body {
            TxBody::Export { .. } => {
                let intent_id = tx
                    .export_id()
                    .and_then(|id| self.roaming_pool.get_by_export_id(&id).map(|i| i.intent_id));
                self.cross_shard.record_export(tx, height, intent_id);
            }
            TxBody::Import { export_id, .. } => {
                let provenance = tx
                    .import_provenance
                    .as_ref()
                    .or_else(|| self.chain.st.exported_registry.get(export_id));
                self.cross_shard.record_import(tx, height, provenance);
            }
            _ => {}
        }
    }

    pub(crate) fn merge_cross_shard_facts(
        &mut self,
        facts: Vec<crate::ledger::CrossShardFact>,
    ) -> usize {
        facts
            .into_iter()
            .filter(|fact| self.cross_shard.insert_peer_fact(fact.clone()))
            .count()
    }

    pub(crate) fn local_account_views(
        &self,
        local_domain_hi: u8,
        observed_at_ms: u64,
    ) -> Vec<PeerAccountViewWire> {
        const PEER_ACCOUNT_VIEWS_CAP: usize = 4096;
        self.chain
            .st
            .accounts
            .iter()
            .filter(|(id, _)| {
                pwm_core::hd::domain_of_account_id(id).to_be_bytes()[0] == local_domain_hi
            })
            .take(PEER_ACCOUNT_VIEWS_CAP)
            .map(|(id, ac)| PeerAccountViewWire {
                id: *id,
                domain_hi: local_domain_hi,
                balance_pwm: ac.balance_pwm,
                initialized: ac.initialized,
                nonce: ac.nonce,
                observed_at_ms,
            })
            .collect()
    }

    pub(crate) fn merge_peer_acct_views(
        &mut self,
        rows: Vec<PeerAccountViewWire>,
        source_node_id: &str,
        expected_domain_hi: u8,
    ) -> usize {
        const PEER_ACCOUNT_CACHE_CAP: usize = 8192;
        let mut changed = 0usize;
        for row in rows.into_iter() {
            if row.domain_hi != expected_domain_hi {
                continue;
            }
            let next = PeerAccountView {
                balance_pwm: row.balance_pwm,
                initialized: row.initialized,
                nonce: row.nonce,
                observed_at_ms: row.observed_at_ms,
                source_node_id: source_node_id.to_string(),
            };
            let is_changed = match self.peer_account_views.get(&row.id) {
                Some(prev) => {
                    prev.balance_pwm != next.balance_pwm
                        || prev.initialized != next.initialized
                        || prev.nonce != next.nonce
                        || prev.observed_at_ms < next.observed_at_ms
                        || prev.source_node_id != next.source_node_id
                }
                None => true,
            };
            if is_changed {
                self.peer_account_views.insert(row.id, next);
                changed = changed.saturating_add(1);
            }
        }
        while self.peer_account_views.len() > PEER_ACCOUNT_CACHE_CAP {
            if let Some((drop_id, _)) = self
                .peer_account_views
                .iter()
                .min_by_key(|(_, v)| v.observed_at_ms)
                .map(|(id, v)| (*id, v.observed_at_ms))
            {
                self.peer_account_views.remove(&drop_id);
            } else {
                break;
            }
        }
        changed
    }
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct FlowTraceRow {
    pub at_height: u64,
    pub kind: String,
    pub tx_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub export_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
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
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::LoadingSnapshot => "loading_snapshot",
            Self::Ready => "ready",
            Self::ReadyDegraded => "ready_degraded",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct InitState {
    pub(crate) phase: InitPhase,
    pub(crate) snapshot_file: Option<PathBuf>,
    pub(crate) snapshot_error: Option<String>,
}

impl InitState {
    pub(crate) fn starting(snapshot_file: Option<PathBuf>) -> Self {
        Self {
            phase: InitPhase::Starting,
            snapshot_file,
            snapshot_error: None,
        }
    }

    pub(crate) fn ready(snapshot_file: Option<PathBuf>) -> Self {
        Self {
            phase: InitPhase::Ready,
            snapshot_file,
            snapshot_error: None,
        }
    }

    pub(crate) fn loading(snapshot_file: Option<PathBuf>) -> Self {
        Self {
            phase: InitPhase::LoadingSnapshot,
            snapshot_file,
            snapshot_error: None,
        }
    }

    pub(crate) fn ready_degraded(snapshot_file: Option<PathBuf>, snapshot_error: String) -> Self {
        Self {
            phase: InitPhase::ReadyDegraded,
            snapshot_file,
            snapshot_error: Some(snapshot_error),
        }
    }

    pub(crate) fn is_ready(&self) -> bool {
        matches!(self.phase, InitPhase::Ready | InitPhase::ReadyDegraded)
    }

    /// Seal loop / canonical advancement only while persistence has not reported fatal degradation.
    pub(crate) fn allows_chain_progress(&self) -> bool {
        matches!(self.phase, InitPhase::Ready)
    }
}
