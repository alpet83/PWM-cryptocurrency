//! Crate-local tests (`crate::tests`), split from legacy `lib.rs` inline module.

mod prelude;

mod helpers;
mod http_export;
mod http_operator_log;
mod http_status;
#[cfg(feature = "clickhouse-snapshot")]
mod snapshot_backend_replay;
mod snapshot_roaming;
mod transport_peer;
