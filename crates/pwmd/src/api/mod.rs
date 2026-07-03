//! REST `/v1/*` facade (split from monolithic `api.rs`).

pub(crate) mod common;
mod handlers_account;
mod handlers_backfill;
pub(crate) mod handlers_bridge;
mod http_admin_rpc;
mod handlers_federation;
mod handlers_lab_seal;
mod handlers_offchain;
mod handlers_operator_log;
mod handlers_peer;
mod handlers_perfmon;
mod handlers_roaming;
pub(crate) mod handlers_shutdown;
mod handlers_status;
mod handlers_tx;
mod handlers_version;
mod router;
mod types;

pub use router::router;
pub use types::{
    AcctListOut, AcctOut, ExportHandoffOut, HeadOut, PeerHelloOut, PeerStatsOut, StatusOut,
    V1_TX_BODY_LIMIT,
};
