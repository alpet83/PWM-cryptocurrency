//! Shared RPC / JSON helpers for CLI HTTP plumbing.

use crate::cli_config::rpc_http_timeout;
use crate::wallet::WalletAccountEntry;
use pwm_core::summarize_tx_reject_json;
use pwm_core::tx::SignedTx;
use pwm_core::AccountId;
use serde_json::Value;
use std::io::IsTerminal;
use std::path::Path;

pub(crate) fn fmt_wallet_acct_line(account: &WalletAccountEntry) -> String {
    let marker = if account.is_active { "*" } else { " " };
    format!(
        "{marker} id_hex={} id_pretty={} derivation_index={} derivation_path={}",
        account.id_hex, account.id_pretty, account.derivation_index, account.derivation_path
    )
}

pub(crate) fn tx_import_contract_note() -> &'static str {
    "tx-import note: use the target RPC after tx-handoff-register succeeds; target must already trust the source peer via configured seed context, and target --to must already be initialized with tx-init on the target shard"
}

pub(crate) fn truncate_rpc_body_hint(body: &str, max: usize) -> String {
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

pub(crate) fn map_reqwest_err(e: &reqwest::Error, ctx: &str) -> String {
    if e.is_timeout() {
        format!(
            "{ctx}: RPC timeout after {:?} (set PWM_CLI_RPC_TIMEOUT_MS or check --rpc/PWM_RPC)",
            rpc_http_timeout()
        )
    } else if e.is_connect() {
        format!("{ctx}: cannot connect (is pwmd running? check --rpc/PWM_RPC): {e}")
    } else {
        format!("{ctx}: {e}")
    }
}

pub(crate) fn resolve_genesis_passphrase(raw: Option<&str>) -> Result<String, String> {
    if let Some(pass) = raw {
        if pass.trim().is_empty() {
            return Err("genesis passphrase must not be empty".to_string());
        }
        return Ok(pass.to_string());
    }
    if std::io::stdin().is_terminal() {
        let pass = rpassword::prompt_password("Enter genesis passphrase: ")
            .map_err(|e| format!("failed to read genesis passphrase: {e}"))?;
        if pass.trim().is_empty() {
            return Err("genesis passphrase must not be empty".to_string());
        }
        return Ok(pass);
    }
    Err(
        "missing genesis passphrase: pass --genesis-passphrase or set PWM_GENESIS_PASSPHRASE"
            .to_string(),
    )
}

fn parse_u64_json_field(v: &Value, field: &str) -> Option<u64> {
    v.get(field).and_then(|n| match n {
        Value::String(s) => s.parse().ok(),
        Value::Number(num) => num.as_u64(),
        _ => None,
    })
}

fn parse_u32_json_field(v: &Value, field: &str) -> Option<u32> {
    v.get(field).and_then(|n| match n {
        Value::String(s) => s.parse().ok(),
        Value::Number(num) => num.as_u64().and_then(|x| u32::try_from(x).ok()),
        _ => None,
    })
}

pub(crate) fn parse_nonce_acct_json(body: &str) -> Option<u64> {
    let v: Value = serde_json::from_str(body).ok()?;
    parse_u64_json_field(&v, "nonce")
}

pub(crate) fn parse_init_flag(body: &str) -> Option<bool> {
    let v: Value = serde_json::from_str(body).ok()?;
    v.get("initialized")?.as_bool()
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AccountLookupMeta {
    pub(crate) local_view_only: bool,
    pub(crate) home_lookup_status: Option<String>,
    pub(crate) authoritative_home_initialized: Option<bool>,
}

pub(crate) fn parse_account_lookup_meta(body: &str) -> Option<AccountLookupMeta> {
    let v: Value = serde_json::from_str(body).ok()?;
    let local_view_only = v
        .get("local_view_only")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let home_lookup_status = v
        .get("home_lookup_status")
        .and_then(Value::as_str)
        .map(|s| s.to_ascii_lowercase());
    let authoritative_home_initialized = v
        .get("authoritative_home_initialized")
        .and_then(Value::as_bool);
    Some(AccountLookupMeta {
        local_view_only,
        home_lookup_status,
        authoritative_home_initialized,
    })
}

pub(crate) fn nonce_404_account_hint(status_code: u16, body: &str) -> Option<&'static str> {
    let body_lc = body.to_ascii_lowercase();
    if status_code == 404 && body_lc.contains("account not found") {
        Some(
            "sender is not initialized on current RPC; run `tx-init` for this sender on the source node and verify --rpc/PWM_RPC points to source domain/shard",
        )
    } else {
        None
    }
}

pub(crate) fn parse_nonce_init(body: &str) -> Result<(u64, bool), String> {
    let nonce = parse_nonce_acct_json(body).ok_or_else(|| {
        format!(
            "missing/invalid `nonce` in /v1/account JSON (body prefix={})",
            {
                let hint = truncate_rpc_body_hint(body, 120);
                if hint.is_empty() {
                    "<empty>".into()
                } else {
                    hint
                }
            }
        )
    })?;
    let initialized = parse_init_flag(body).ok_or_else(|| {
        format!(
            "missing/invalid `initialized` in /v1/account JSON (body prefix={})",
            truncate_rpc_body_hint(body, 120)
        )
    })?;
    Ok((nonce, initialized))
}

pub(crate) fn fetch_nonce_init_opt(
    c: &reqwest::blocking::Client,
    rpc_base: &str,
    from: AccountId,
) -> Result<Option<(u64, bool)>, String> {
    let from_hex = hex::encode(from);
    let url = format!("{}/v1/account/{}", rpc_base, from_hex);
    let r = c
        .get(&url)
        .send()
        .map_err(|e| map_reqwest_err(&e, "nonce fetch"))?;
    let status = r.status();
    let body = r.text().unwrap_or_default();

    parse_nonce_init_response(status, &body, &url)
}

pub(crate) fn parse_nonce_init_response(
    status: reqwest::StatusCode,
    body: &str,
    url: &str,
) -> Result<Option<(u64, bool)>, String> {
    if status == reqwest::StatusCode::NOT_FOUND
        && body.to_ascii_lowercase().contains("account not found")
    {
        return Ok(None);
    }

    if !status.is_success() {
        let hint = truncate_rpc_body_hint(body, 240);
        let ux_hint = nonce_404_account_hint(status.as_u16(), body);
        return Err(if hint.is_empty() {
            if let Some(ux_hint) = ux_hint {
                format!("nonce fetch: HTTP {status} from {url}. hint: {ux_hint}")
            } else {
                format!(
                    "nonce fetch: HTTP {status} from {url} (wrong shard, unknown account, or bad --rpc/PWM_RPC?)"
                )
            }
        } else if let Some(ux_hint) = ux_hint {
            format!("nonce fetch: HTTP {status} from {url}: {hint}. hint: {ux_hint}")
        } else {
            format!("nonce fetch: HTTP {status} from {url}: {hint}")
        });
    }

    parse_nonce_init(body).map(Some)
}

pub(crate) fn preflight_recipient_init(
    c: &reqwest::blocking::Client,
    rpc_base: &str,
    to: AccountId,
    flow: &str,
) -> Result<(), String> {
    let to_hex = hex::encode(to);
    let url = format!("{}/v1/account/{}", rpc_base, to_hex);
    let r = c
        .get(&url)
        .send()
        .map_err(|e| map_reqwest_err(&e, "recipient preflight"))?;
    let status = r.status();
    let body = r.text().unwrap_or_default();
    if status == reqwest::StatusCode::NOT_FOUND
        && body.to_ascii_lowercase().contains("account not found")
    {
        return Err(format!(
            "{flow}: recipient account not found on current RPC; recipient must run `tx-init` on the target shard first"
        ));
    }
    if !status.is_success() {
        let hint = truncate_rpc_body_hint(&body, 240);
        return Err(if hint.is_empty() {
            format!("{flow}: recipient preflight HTTP {status} from {url}")
        } else {
            format!("{flow}: recipient preflight HTTP {status} from {url}: {hint}")
        });
    }
    let meta = parse_account_lookup_meta(&body);
    if let Some(meta) = meta.as_ref() {
        if meta.local_view_only {
            match meta.home_lookup_status.as_deref() {
                Some("ok") => {
                    if let Some(initialized) = meta.authoritative_home_initialized {
                        if !initialized {
                            return Err(format!(
                                "{flow}: recipient home-shard account is not initialized (authoritative peer data); run `tx-init` on the recipient home shard"
                            ));
                        }
                        return Ok(());
                    }
                    return Err(format!(
                        "{flow}: recipient home-shard init state is unknown (authoritative peer path returned partial data); verify protocol peer link and retry"
                    ));
                }
                Some("unavailable") | None => {
                    return Err(format!(
                        "{flow}: recipient home-shard init state is unavailable via protocol peer path; verify trusted peer connectivity before submit"
                    ));
                }
                Some(other) => {
                    return Err(format!(
                        "{flow}: recipient home-shard init state is not authoritative (home_lookup_status={other}); verify trusted peer connectivity before submit"
                    ));
                }
            }
        }
    }
    let initialized = parse_init_flag(&body).ok_or_else(|| {
        format!("{flow}: recipient preflight response missing/invalid `initialized` field")
    })?;
    if !initialized {
        return Err(format!(
            "{flow}: recipient account is not initialized on current RPC; recipient must run `tx-init` on the target shard first"
        ));
    }
    Ok(())
}

pub(crate) fn fetch_nonce(
    c: &reqwest::blocking::Client,
    rpc_base: &str,
    from: AccountId,
) -> Result<u64, String> {
    let from_hex = hex::encode(from);
    let url = format!("{}/v1/account/{}", rpc_base, from_hex);
    let r = c
        .get(&url)
        .send()
        .map_err(|e| map_reqwest_err(&e, "nonce fetch"))?;
    let status = r.status();
    let body = r.text().unwrap_or_default();
    if !status.is_success() {
        let hint = truncate_rpc_body_hint(&body, 240);
        let ux_hint = nonce_404_account_hint(status.as_u16(), &body);
        return Err(if hint.is_empty() {
            if let Some(ux_hint) = ux_hint {
                format!("nonce fetch: HTTP {status} from {url}. hint: {ux_hint}")
            } else {
                format!(
                "nonce fetch: HTTP {status} from {url} (wrong shard, unknown account, or bad --rpc/PWM_RPC?)"
            )
            }
        } else {
            if let Some(ux_hint) = ux_hint {
                format!("nonce fetch: HTTP {status} from {url}: {hint}. hint: {ux_hint}")
            } else {
                format!("nonce fetch: HTTP {status} from {url}: {hint}")
            }
        });
    }
    parse_nonce_acct_json(&body).ok_or_else(|| {
        let hint = truncate_rpc_body_hint(&body, 240);
        format!(
            "nonce fetch: HTTP 200 but missing/invalid `nonce` (expected JSON number or decimal string). {}",
            if hint.is_empty() {
                "(empty body)".into()
            } else {
                hint
            }
        )
    })
}

pub(crate) fn fetch_marks(
    c: &reqwest::blocking::Client,
    rpc_base: &str,
    from: AccountId,
) -> Result<u32, String> {
    let from_hex = hex::encode(from);
    let url = format!("{}/v1/account/{}", rpc_base, from_hex);
    let r = c
        .get(&url)
        .send()
        .map_err(|e| map_reqwest_err(&e, "marks fetch"))?;
    let status = r.status();
    let body = r.text().unwrap_or_default();
    if !status.is_success() {
        let hint = truncate_rpc_body_hint(&body, 240);
        return Err(if hint.is_empty() {
            format!("marks fetch: HTTP {status} from {url}")
        } else {
            format!("marks fetch: HTTP {status} from {url}: {hint}")
        });
    }
    let v: Value = serde_json::from_str(&body).map_err(|e| {
        format!(
            "marks fetch: failed to parse /v1/account JSON for {}: {e}",
            hex::encode(from)
        )
    })?;
    parse_u32_json_field(&v, "marks").ok_or_else(|| {
        let hint = truncate_rpc_body_hint(&body, 240);
        format!(
            "marks fetch: HTTP 200 but missing/invalid `marks` (expected JSON number or decimal string). {}",
            if hint.is_empty() {
                "(empty body)".into()
            } else {
                hint
            }
        )
    })
}

pub(crate) fn format_tx_submit_error(status: reqwest::StatusCode, body: &str, url: &str) -> String {
    if let Some(hint) = summarize_tx_reject_json(body) {
        return format!("tx submit: HTTP {status} ({url}): {hint}");
    }
    let hint = truncate_rpc_body_hint(body, 400);
    if hint.is_empty() {
        format!("tx submit: HTTP {status} ({url})")
    } else {
        format!("tx submit: HTTP {status} ({url}): {hint}")
    }
}

pub(crate) fn post_signed_tx(
    c: &reqwest::blocking::Client,
    rpc_base: &str,
    tx: &SignedTx,
) -> Result<(), String> {
    let url = format!("{}/v1/tx", rpc_base);
    let r = c
        .post(&url)
        .json(tx)
        .send()
        .map_err(|e| map_reqwest_err(&e, "tx submit"))?;
    let status = r.status();
    let body = r.text().unwrap_or_default();
    if status.is_success() {
        println!("{status}");
        return Ok(());
    }
    Err(format_tx_submit_error(status, &body, &url))
}

pub(crate) fn load_handoff_json(path: &Path) -> Result<Value, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read handoff JSON {}: {e}", path.display()))?;
    let value: Value = serde_json::from_str(&raw)
        .map_err(|e| format!("failed to parse handoff JSON {}: {e}", path.display()))?;
    Ok(value.get("handoff").cloned().unwrap_or(value))
}
