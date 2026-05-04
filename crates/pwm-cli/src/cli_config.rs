//! CLI bootstrap: RPC client timeout and default wallet path resolution.

use pwm_core::{
    blocking_http_client_rpc, parse_rpc_timeout_ms, resolve_wallet_out_path as core_resolve,
};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

pub(crate) static RPC_CLIENT_FALLBACK_WARNED: AtomicBool = AtomicBool::new(false);
pub(crate) const DEFAULT_WALLET_OUT_REL: &str = "~/.pwm-crypto/default-wallet.yaml";

pub(crate) fn rpc_http_timeout() -> Duration {
    const DEFAULT_MS: u64 = 10_000;
    parse_rpc_timeout_ms(
        std::env::var("PWM_CLI_RPC_TIMEOUT_MS").ok().as_deref(),
        DEFAULT_MS,
    )
}

pub(crate) fn resolve_wallet_out_path(path: Option<PathBuf>) -> Result<PathBuf, String> {
    core_resolve(path, DEFAULT_WALLET_OUT_REL)
}

pub(crate) fn http_client_for_rpc() -> reqwest::blocking::Client {
    let t = rpc_http_timeout();
    blocking_http_client_rpc(
        t,
        &RPC_CLIENT_FALLBACK_WARNED,
        "pwm-cli",
        "PWM_CLI_RPC_TIMEOUT_MS",
    )
}
