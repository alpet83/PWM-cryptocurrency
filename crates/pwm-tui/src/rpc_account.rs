//! RPC helpers for account nonce and recipient preflight.

use pwm_core::AccountId;
use serde_json::Value;

use crate::config::rpc_timeout_hint;

pub fn truncate_rpc_err_hint(body: &str, max: usize) -> String {
    let t = body.trim();
    if t.is_empty() {
        return String::new();
    }
    let one_line: String = t.chars().filter(|c| !c.is_control()).take(max).collect();
    if t.chars().count() > max {
        format!("{one_line}...")
    } else {
        one_line
    }
}

pub fn nonce_404_account_hint(status_code: u16, body: &str) -> Option<&'static str> {
    let body_lc = body.to_ascii_lowercase();
    if status_code == 404 && body_lc.contains("account not found") {
        Some(
            "sender is not initialized on current RPC; run `tx-init` for this sender on the source node and verify RPC points to source domain/shard",
        )
    } else {
        None
    }
}

/// Parses `GET /v1/account/{id}` body: no silent `nonce=0` on HTTP or JSON failure.
pub fn nonce_from_account_body(
    is_success: bool,
    status_code: u16,
    url: &str,
    body: &str,
) -> Result<u64, String> {
    if !is_success {
        let hint = truncate_rpc_err_hint(body, 240);
        let ux_hint = nonce_404_account_hint(status_code, body);
        return Err(if hint.is_empty() {
            if let Some(ux_hint) = ux_hint {
                format!("nonce: HTTP {status_code} from {url}. hint: {ux_hint}")
            } else {
                format!("nonce: HTTP {status_code} from {url}")
            }
        } else {
            if let Some(ux_hint) = ux_hint {
                format!("nonce: HTTP {status_code} from {url}: {hint}. hint: {ux_hint}")
            } else {
                format!("nonce: HTTP {status_code} from {url}: {hint}")
            }
        });
    }
    parse_nonce_json(body).ok_or_else(|| {
        let hint = truncate_rpc_err_hint(body, 240);
        format!(
            "nonce: response missing/invalid `nonce` field. {}",
            if hint.is_empty() {
                "(empty body)".into()
            } else {
                hint
            }
        )
    })
}

pub fn fetch_nonce(
    c: &reqwest::blocking::Client,
    rpc_base: &str,
    from: AccountId,
) -> Result<u64, String> {
    let from_hex = hex::encode(from);
    let url = format!("{rpc_base}/v1/account/{from_hex}");
    let r = c.get(&url).send().map_err(|e| {
        if e.is_timeout() {
            format!("nonce: {}", rpc_timeout_hint())
        } else if e.is_connect() {
            format!("nonce: cannot connect to RPC: {e}")
        } else {
            format!("nonce: rpc request failed: {e}")
        }
    })?;
    let status = r.status();
    let is_success = status.is_success();
    let code = status.as_u16();
    let body = r.text().unwrap_or_default();
    nonce_from_account_body(is_success, code, &url, &body)
}

pub fn parse_nonce_json(body: &str) -> Option<u64> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    v.get("nonce").and_then(parse_u64_value)
}

fn parse_initialized_json(body: &str) -> Option<bool> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    v.get("initialized")?.as_bool()
}

fn parse_u64_value(v: &Value) -> Option<u64> {
    match v {
        Value::String(s) => s.parse().ok(),
        Value::Number(n) => n.as_u64(),
        _ => None,
    }
}

pub fn preflight_recipient_rpc(
    c: &reqwest::blocking::Client,
    rpc_base: &str,
    to: AccountId,
) -> Result<(), String> {
    let to_hex = hex::encode(to);
    let url = format!("{rpc_base}/v1/account/{to_hex}");
    let r = c.get(&url).send().map_err(|e| {
        if e.is_timeout() {
            format!("recipient preflight: {}", rpc_timeout_hint())
        } else if e.is_connect() {
            format!("recipient preflight: cannot connect to RPC: {e}")
        } else {
            format!("recipient preflight: rpc request failed: {e}")
        }
    })?;
    let status = r.status();
    let body = r.text().unwrap_or_default();
    if status == reqwest::StatusCode::NOT_FOUND
        && body.to_ascii_lowercase().contains("account not found")
    {
        return Err(
            "recipient account not found; recipient must initialize on target shard first"
                .to_string(),
        );
    }
    if !status.is_success() {
        let hint = truncate_rpc_err_hint(&body, 240);
        return Err(if hint.is_empty() {
            format!("recipient preflight: HTTP {} from {url}", status.as_u16())
        } else {
            format!(
                "recipient preflight: HTTP {} from {url}: {hint}",
                status.as_u16()
            )
        });
    }
    let initialized = parse_initialized_json(&body)
        .ok_or_else(|| "recipient preflight: missing/invalid `initialized` field".to_string())?;
    if !initialized {
        return Err(
            "recipient account is not initialized; recipient must initialize on target shard first"
                .to_string(),
        );
    }
    Ok(())
}
