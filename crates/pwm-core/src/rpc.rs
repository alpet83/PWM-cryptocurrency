//! RPC HTTP timeout parsing and shared blocking `reqwest` client setup.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Upper bound for env-driven RPC timeouts (CLI/TUI align on this cap).
pub const RPC_TIMEOUT_MS_CAP: u64 = 120_000;

/// Parse `PWM_*_RPC_TIMEOUT_MS`-style millisecond values into a [`Duration`].
pub fn parse_rpc_timeout_ms(raw: Option<&str>, default_ms: u64) -> Duration {
    raw.and_then(|s| s.parse::<u64>().ok())
        .filter(|&ms| ms > 0 && ms <= RPC_TIMEOUT_MS_CAP)
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_millis(default_ms))
}

/// Blocking HTTP client with matching connect/request timeouts; warns once then falls back.
pub fn blocking_http_client_rpc(
    timeout: Duration,
    warned_once: &AtomicBool,
    tool_label: &'static str,
    env_var_hint: &'static str,
) -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .connect_timeout(timeout)
        .timeout(timeout)
        .build()
        .unwrap_or_else(|e| {
            if !warned_once.swap(true, Ordering::Relaxed) {
                eprintln!(
                    "{tool_label}: failed to build HTTP client with timeout {timeout:?}: {e}; \
                     falling back to reqwest defaults (timeout behavior may differ, env {env_var_hint})",
                );
            }
            reqwest::blocking::Client::new()
        })
}

#[cfg(test)]
mod tests {
    use super::parse_rpc_timeout_ms;
    use std::time::Duration;

    /// `parse_rpc_timeout_ms` clamps to cap and restores defaults on bad input (formerly `parse_rpc_timeout_ms_clamps_and_defaults`).
    #[test]
    fn rpc_tmout_clamp_default() {
        assert_eq!(
            parse_rpc_timeout_ms(Some("9000"), 10_000),
            Duration::from_millis(9000)
        );
        assert_eq!(
            parse_rpc_timeout_ms(Some("2500"), 3000),
            Duration::from_millis(2500)
        );
        assert_eq!(
            parse_rpc_timeout_ms(Some("0"), 3000),
            Duration::from_millis(3000)
        );
        assert_eq!(
            parse_rpc_timeout_ms(Some("999999999"), 3000),
            Duration::from_millis(3000)
        );
        assert_eq!(
            parse_rpc_timeout_ms(Some("bad"), 3000),
            Duration::from_millis(3000)
        );
    }
}
