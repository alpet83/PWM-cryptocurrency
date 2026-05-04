//! CLI args, RPC base URLs, HTTP client, and related UX strings.

use clap::Parser;
use pwm_core::{blocking_http_client_rpc, parse_rpc_timeout_ms};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

/// Default auto-lock after unlock for encrypted wallets (seconds).
pub const DEFAULT_WALLET_UNLOCK_SECS: u64 = 300;
/// Upper bound for `PWM_TUI_WALLET_UNLOCK_SECS` / `--wallet-unlock-secs` (1 week).
pub const WALLET_UNLOCK_SECS_MAX: u64 = 604_800;

#[derive(Parser, Debug)]
#[command(name = "pwm-tui", about = "PWM terminal UI")]
pub struct Args {
    #[arg(long, env = "PWM_TUI_WALLET")]
    pub wallet: Option<PathBuf>,
    #[arg(long, env = "PWM_TUI_WALLET_PASSPHRASE")]
    pub wallet_passphrase: Option<String>,
    /// Explicitly upgrade wallet schema v2 -> v3 when loading wallet files.
    #[arg(long)]
    pub upgrade_wallet: bool,
    /// Auto-lock encrypted wallet after this many seconds (min 1, max 604800). Env: `PWM_TUI_WALLET_UNLOCK_SECS`.
    #[arg(long = "wallet-unlock-secs", env = "PWM_TUI_WALLET_UNLOCK_SECS", default_value_t = DEFAULT_WALLET_UNLOCK_SECS)]
    pub wallet_unlock_secs: u64,
}

pub fn base_url() -> String {
    std::env::var("PWM_RPC").unwrap_or_else(|_| "http://127.0.0.1:3030".into())
}

/// HTTP RPC of the **counterparty** shard for `GET /v1/account` (nonce + balance) on the recipient.
/// Override with `PWM_TUI_TARGET_RPC`; otherwise flip `:3030` ↔ `:3031` in `PWM_RPC` (demo layout).
pub fn cross_shard_target_rpc_base() -> String {
    if let Ok(u) = std::env::var("PWM_TUI_TARGET_RPC") {
        let t = u.trim();
        if !t.is_empty() {
            return t.to_string();
        }
    }
    let base = base_url();
    if base.contains(":3030") {
        return base.replace(":3030", ":3031");
    }
    if base.contains(":3031") {
        return base.replace(":3031", ":3030");
    }
    base
}

pub fn shard_hint_from_rpc_url(rpc_url: &str) -> &'static str {
    if rpc_url.contains(":3030") {
        "shard A?"
    } else if rpc_url.contains(":3031") {
        "shard B?"
    } else {
        "unknown shard"
    }
}

pub fn parse_status_shard_label(v: &Value) -> Option<String> {
    v.get("shard")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}

pub fn rpc_context_label(rpc_url: &str, status_shard_label: Option<&str>) -> String {
    let shard_label = status_shard_label
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| shard_hint_from_rpc_url(rpc_url).to_string());
    format!("RPC={} ({})", rpc_url, shard_label)
}

pub fn f5_burn_not_wired_message(rpc_url: &str) -> String {
    format!(
        "F5 burn is not wired in TUI yet. Use CLI on the same RPC:\n\
         pwm --rpc {rpc_url} tx-burn-mark ...\n\
         See docs/tester-guide-cli-tui-scenarios.md §4."
    )
}

pub fn shard_cli_hint(rpc_url: &str) -> String {
    format!(
        "Inter-shard route: source `{rpc_url}` uses roaming-intent relay; manual fallback target needs trusted source seed context, then run `pwm tx-handoff-register` and `pwm tx-import`."
    )
}

pub fn inter_shard_status_short() -> &'static str {
    "Inter-shard send uses roaming-intent lifecycle (queued/exported/relayed/imported/expired/failed)."
}

static RPC_CLIENT_FALLBACK_WARNED: AtomicBool = AtomicBool::new(false);

/// UI-side throttle for debug account JSON pulls.
pub const DEBUG_FETCH_INTERVAL: Duration = Duration::from_millis(800);
/// Keep a short local timeline in-memory (most recent first).
pub const OP_HISTORY_MAX_ITEMS: usize = 20;
pub const SEND_FLOW_AUTO_STEP_TIMEOUT: Duration = Duration::from_secs(5);

pub fn wallet_unlock_secs_clamped(args: &Args) -> u64 {
    args.wallet_unlock_secs.clamp(1, WALLET_UNLOCK_SECS_MAX)
}

pub fn rpc_timeout() -> Duration {
    const DEFAULT_MS: u64 = 3000;
    parse_rpc_timeout_ms(
        std::env::var("PWM_TUI_RPC_TIMEOUT_MS").ok().as_deref(),
        DEFAULT_MS,
    )
}

pub fn rpc_timeout_hint() -> String {
    format!(
        "rpc timeout after {:?} (set PWM_TUI_RPC_TIMEOUT_MS or check PWM_RPC)",
        rpc_timeout()
    )
}

pub fn http_client() -> reqwest::blocking::Client {
    let t = rpc_timeout();
    blocking_http_client_rpc(
        t,
        &RPC_CLIENT_FALLBACK_WARNED,
        "pwm-tui",
        "PWM_TUI_RPC_TIMEOUT_MS",
    )
}
