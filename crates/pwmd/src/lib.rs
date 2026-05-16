//! Devnet node library: REST `/v1/*`, router builder, seal loop for the `pwmd` binary.

use axum::http::{HeaderValue, Method, StatusCode};
use ed25519_dalek::SigningKey;
use pwm_core::digest;
use serde::Serialize;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use tokio::time::Duration;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tracing::{info, warn};

mod api;
mod bootstrap;
mod bridge_trust;
mod config;
mod debug_dump;
mod federation;
pub mod handshake;
mod identity;
mod lease;
mod lease_backend;
mod ledger;
mod lifecycle;
mod logging;
mod peer_list;
mod relay;
mod roaming;
#[doc(hidden)]
pub mod snap_bench_hlp;
mod snapshot;
mod state;
mod transport;
mod tx_policy;
mod wire_serde;
pub use api::{
    router, AcctListOut, AcctOut, HeadOut, PeerHelloOut, PeerStatsOut, StatusOut, V1_TX_BODY_LIMIT,
};
pub use bootstrap::{
    app_from_dev_net, app_from_devnet, app_from_genesis, app_from_genesis_data,
    app_from_genesis_def, app_from_genesis_shard,
};
pub use config::{
    ClusterCfg, ConsoleColorMode, DebugDumpCfg, GenesisSource, LogFileMode, LoggingConfig,
    PersistSnapKind, PwmdConfig, TransportConfig,
};
pub use identity::{
    default_runtime_identity_neutral, neutral_listen_dir_tag, parse_cluster_domain_hi,
    resolve_runtime_identity, runtime_shard_label, storage_namespace, DevLane, RuntimeIdentity,
    RuntimeIdentityInput, RuntimeIdentityMode,
};
/// Hidden hooks for `pwmd_lease_probe` multi-process harness.
#[doc(hidden)]
pub use lease::step_lease;
pub use lease::{
    LeaseBackendMode, LeaseCfg, LeaseRuntime, LeaseSignal, LeaseState, LeaseStats, LeaseStatsOut,
};
#[doc(hidden)]
pub use lease_backend::FileLeaseBackend;
pub use lifecycle::{run, run_with, spawn_seal_loop};
pub use logging::{init_logging, logger, NodeLogger};
pub use peer_list::{
    default_peer_file, drop_self_seed, load_peer_file, merge_peer_seeds, pick_peer_file,
    save_peer_file, PeerDocState, PeerFileLoad,
};
#[cfg(feature = "clickhouse-snapshot")]
pub use snapshot::ch_http::{
    norm_ch_http_base, pwmd_snap_row_key, resolve_ch_database, snap_ch_db_net, snap_ch_sql_id,
    snap_ch_tbl_pair, snap_ch_tbl_validators, SnapChCfg,
};
pub use snapshot::load_genesis_bundle;
pub use snapshot::{repair_json_epochs, SnapRepairOpts, SnapRepairReport};
pub use state::{App, InitPhase, Inner};
pub use transport::{
    spawn_peer_listener_loop, spawn_real_transport_loop, spawn_stateful_transport_loop,
    spawn_transport_loop,
};
pub use transport::{
    ChurnSnapshot, PeerClass, PeerPolicyConfig, PeerPolicyCounters, PeerPolicySnapshot, PeerRecord,
    PeerStatus, SoakConfidenceSnapshot, TransportCounters, TransportSnapshot,
};

pub(crate) fn current_time_ms() -> Result<u64, (StatusCode, String)> {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("system clock error: {e}"),
            )
        })?;
    Ok(dur.as_millis() as u64)
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
        .allow_methods([Method::GET, Method::POST, Method::DELETE])
        .allow_headers(Any)
        .allow_origin(AllowOrigin::list(origins)))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod slice20_e2e_tests;
