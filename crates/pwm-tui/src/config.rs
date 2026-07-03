//! CLI args, RPC base URLs, HTTP client, and related UX strings.

use clap::Parser;
use pwm_core::{blocking_http_client_rpc, parse_rpc_timeout_ms};
use serde_json::Value;
use std::path::{Path, PathBuf};
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

pub const TX_HISTORY_DIR: &str = "tx-history";

pub fn resolve_wallet_dir(wallet_path: &Path) -> Option<PathBuf> {
    if wallet_path.is_file() {
        return wallet_path.parent().map(Path::to_path_buf);
    }
    wallet_path
        .is_dir()
        .then(|| wallet_path.to_path_buf())
        .and_then(|dir| wallet_file_in_dir(&dir).is_some().then_some(dir))
}

pub fn resolve_wallet_file(wallet_path: &Path) -> Option<PathBuf> {
    if wallet_path.is_file() {
        return Some(wallet_path.to_path_buf());
    }
    wallet_path
        .is_dir()
        .then(|| wallet_file_in_dir(wallet_path))
        .flatten()
}

pub fn init_tx_history_dir(wallet_path: Option<&Path>) {
    let Some(wallet_dir) = wallet_path.and_then(resolve_wallet_dir) else {
        return;
    };
    let _ = std::fs::create_dir_all(wallet_dir.join(TX_HISTORY_DIR));
}

pub fn wallet_dir(args: &Args) -> Option<PathBuf> {
    args.wallet.as_deref().and_then(resolve_wallet_dir)
}

fn wallet_file_in_dir(dir: &Path) -> Option<PathBuf> {
    let wallet_json = dir.join("wallet.json");
    if wallet_json.is_file() {
        return Some(wallet_json);
    }
    let stem = dir.file_name()?.to_string_lossy();
    let named = dir.join(format!("{stem}.json"));
    named.is_file().then_some(named)
}

pub fn base_url() -> String {
    std::env::var("PWM_RPC").unwrap_or_else(|_| "http://127.0.0.1:3030".into())
}

/// HTTP RPC of the **counterparty** shard for `GET /v1/account` (nonce + balance) on the recipient.
/// Override with `PWM_TUI_TARGET_RPC`; otherwise flip `:3030` ↔ `:3031` in `PWM_RPC` (demo layout).
/// Returns the cross-shard target RPC base URL from config.
pub fn xshard_rpc_base() -> String {
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

/// Derives a shard-domain hint from the RPC base URL.
pub fn shard_hint_rpc(rpc_url: &str) -> &'static str {
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
        .unwrap_or_else(|| shard_hint_rpc(rpc_url).to_string());
    format!("RPC={} ({})", rpc_url, shard_label)
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
pub const SEND_FLOW_STEP_TIMEOUT: Duration = Duration::from_secs(5);

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

#[cfg(test)]
mod tests {
    use super::{init_tx_history_dir, resolve_wallet_dir, resolve_wallet_file, TX_HISTORY_DIR};
    use std::fs;
    use std::path::PathBuf;

    fn tmp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "pwm_tui_wallet_dir_{name}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("mkdir temp wallet dir");
        dir
    }

    #[test]
    fn wallet_dir_file_parent() {
        let dir = tmp_dir("file_parent");
        let wallet = dir.join("wallet.json");
        fs::write(&wallet, "{}").expect("write wallet file");

        assert_eq!(resolve_wallet_dir(&wallet), Some(dir.clone()));
        assert_eq!(resolve_wallet_file(&wallet), Some(wallet));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn wallet_dir_contains_json() {
        let dir = tmp_dir("dir_wallet_json");
        let wallet = dir.join("wallet.json");
        fs::write(&wallet, "{}").expect("write wallet file");

        assert_eq!(resolve_wallet_dir(&dir), Some(dir.clone()));
        assert_eq!(resolve_wallet_file(&dir), Some(wallet));
        assert!(!dir.join(TX_HISTORY_DIR).exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn wallet_dir_named_json() {
        let dir = tmp_dir("named_json");
        let name = dir.file_name().expect("dir name").to_string_lossy();
        let wallet = dir.join(format!("{name}.json"));
        fs::write(&wallet, "{}").expect("write named wallet file");

        assert_eq!(resolve_wallet_dir(&dir), Some(dir.clone()));
        assert_eq!(resolve_wallet_file(&dir), Some(wallet));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn init_history_creates_dir() {
        let dir = tmp_dir("history_init");
        let wallet = dir.join("wallet.json");
        fs::write(&wallet, "{}").expect("write wallet file");

        init_tx_history_dir(Some(&wallet));

        assert!(dir.join(TX_HISTORY_DIR).is_dir());
        let _ = fs::remove_dir_all(dir);
    }
}
