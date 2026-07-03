//! `/v1/version` — static build information.

use axum::Json;
use serde::Serialize;

/// Build-time version information baked in by `build.rs`.
const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
const GIT_REF: &str = env!("PWM_GIT_REF");
const BUILD_TS: &str = env!("PWM_BUILD_TS");
const PROTOCOL_VERSION: &str = crate::handshake::PWM_PROTOCOL_VERSION;

#[derive(Serialize)]
pub(super) struct VersionOut {
    /// Cargo package version from `Cargo.toml`.
    version: &'static str,
    /// Short git hash, optionally suffixed with `+dirty`.
    git: &'static str,
    /// ISO-8601 UTC timestamp of the build.
    build_ts: &'static str,
    /// Wire protocol version negotiated between peers.
    protocol: &'static str,
}

pub(super) async fn v1_version() -> Json<VersionOut> {
    Json(VersionOut {
        version: PKG_VERSION,
        git: GIT_REF,
        build_ts: BUILD_TS,
        protocol: PROTOCOL_VERSION,
    })
}
