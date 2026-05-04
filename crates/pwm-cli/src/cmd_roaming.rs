//! tx-export / tx-import / tx-handoff-register and tx-send cross-shard roaming path.

use crate::cli_parse::{hex32, parse_address_input, parse_domain};
use crate::rpc_helpers::{
    fetch_nonce, fetch_nonce_init_opt, load_handoff_json, map_reqwest_err,
    parse_account_lookup_meta, parse_nonce_init_response, post_signed_tx, preflight_recipient_init,
    truncate_rpc_body_hint, tx_import_contract_note,
};
use crate::signer::{load_tx_signer_source, TxSignerSource};
use crate::{exit_user_error, http_client_for_rpc};
use pwm_core::hd::domain_of_account_id;
use pwm_core::state::ExportProvenance;
use pwm_core::tx::{SignedTx, TxBody};
use pwm_core::AccountId;
use serde_json::Value;
use std::path::PathBuf;
use std::time::Duration;

/// Cross-shard `tx-send`: roaming intent lifecycle (shared with roaming HTTP helpers in this module).
pub(crate) fn run_tx_send_cross_domain(
    rpc_base: &str,
    source: &TxSignerSource,
    to_id: AccountId,
    amount: u128,
    fee: u128,
    nonce: u64,
) {
    let c = http_client_for_rpc();
    let export_tx = SignedTx::sign_body(
        &source.sk,
        source.dom,
        source.idx,
        nonce,
        TxBody::Export {
            to: to_id,
            target_domain: domain_of_account_id(&to_id),
            amount,
            fee,
        },
    );
    post_export_readiness(&c, rpc_base, &export_tx).unwrap_or_else(|e| exit_user_error(&e));
    let created =
        post_roaming_intent(&c, rpc_base, &export_tx).unwrap_or_else(|e| exit_user_error(&e));
    println!(
        "cross-domain send: roaming intent {} created (export_id={}, status={}, duplicate={})",
        created.intent_id, created.export_id, created.status, created.duplicate
    );
    let mut reached_terminal = false;
    for _ in 0..8 {
        let status = get_roaming_intent_status(&c, rpc_base, &created.intent_id)
            .unwrap_or_else(|e| exit_user_error(&e));
        let suffix = status
            .last_error
            .as_deref()
            .map(|x| format!(" ({x})"))
            .unwrap_or_default();
        println!("roaming intent status: {}{}", status.status, suffix);
        if is_terminal_intent_status(&status.status) {
            reached_terminal = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    if !reached_terminal {
        println!(
            "roaming intent is still in progress; repeat status check: GET {}/v1/roaming-intents/{}",
            rpc_base, created.intent_id
        );
    }
}

pub(crate) fn run_tx_export(
    rpc_base: &str,
    wallet: Option<PathBuf>,
    master: Option<String>,
    domain: Option<String>,
    wallet_passphrase: Option<&str>,
    upgrade_wallet: bool,
    to: String,
    target_domain: String,
    amount: u128,
    fee: u128,
) {
    let source = load_tx_signer_source(wallet, master, domain, wallet_passphrase, upgrade_wallet)
        .unwrap_or_else(|e| exit_user_error(&e));
    let c = http_client_for_rpc();
    let nonce = fetch_nonce(&c, rpc_base, source.from).unwrap_or_else(|e| exit_user_error(&e));
    let (to, uri_amount) = parse_address_input("--to", &to).unwrap_or_else(|e| exit_user_error(&e));
    if uri_amount.is_some() {
        exit_user_error("URI amount is not allowed for --to in tx-export");
    }
    let target_domain = parse_domain(&target_domain)
        .unwrap_or_else(|e| exit_user_error(&format!("bad --target-domain: {e}")));
    let tx = SignedTx::sign_body(
        &source.sk,
        source.dom,
        source.idx,
        nonce,
        TxBody::Export {
            to,
            target_domain,
            amount,
            fee,
        },
    );
    post_signed_tx(&c, rpc_base, &tx).unwrap_or_else(|e| exit_user_error(&e));
}

pub(crate) fn run_tx_import(
    rpc_base: &str,
    wallet: Option<PathBuf>,
    master: Option<String>,
    domain: Option<String>,
    wallet_passphrase: Option<&str>,
    upgrade_wallet: bool,
    to: String,
    amount: u128,
    export_id: String,
) {
    let source = load_tx_signer_source(wallet, master, domain, wallet_passphrase, upgrade_wallet)
        .unwrap_or_else(|e| exit_user_error(&e));
    let c = http_client_for_rpc();
    let (to, uri_amount) = parse_address_input("--to", &to).unwrap_or_else(|e| exit_user_error(&e));
    if uri_amount.is_some() {
        exit_user_error("URI amount is not allowed for --to in tx-import");
    }
    let export_id = parse_export_id_hex_arg(&export_id).unwrap_or_else(|e| exit_user_error(&e));
    preflight_recipient_init(&c, rpc_base, to, "tx-import").unwrap_or_else(|e| exit_user_error(&e));
    let nonce = ensure_import_sender(&c, rpc_base, &source).unwrap_or_else(|e| exit_user_error(&e));
    let prov = import_provenance_from_target_facts(&c, rpc_base, &export_id, to, amount)
        .unwrap_or_else(|e| exit_user_error(&e));
    let mut tx = SignedTx::sign_body(
        &source.sk,
        source.dom,
        source.idx,
        nonce,
        TxBody::Import {
            to,
            amount,
            export_id,
        },
    );
    tx.set_import_provenance_signed(&source.sk, Some(prov));
    eprintln!("{}", tx_import_contract_note());
    post_import_retry_inner(&c, rpc_base, &tx, 20, Duration::from_millis(500))
        .unwrap_or_else(|e| exit_user_error(&e));
}

/// Loads [`ExportProvenance`] from target node's `/v1/cross-shard/facts` (required for deterministic import replay).
fn import_provenance_from_target_facts(
    c: &reqwest::blocking::Client,
    rpc_base: &str,
    export_id: &[u8; 32],
    to: AccountId,
    amount: u128,
) -> Result<ExportProvenance, String> {
    let target_domain = domain_of_account_id(&to);
    let hi = target_domain.to_be_bytes()[0];
    let url = format!(
        "{}/v1/cross-shard/facts?target_domain_hi={}&from_height=0&limit=512",
        rpc_base.trim_end_matches('/'),
        hi
    );
    let resp = c
        .get(&url)
        .send()
        .map_err(|e| map_reqwest_err(&e, "GET /v1/cross-shard/facts"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "cross-shard facts HTTP {} (target must list facts after export/handoff)",
            resp.status()
        ));
    }
    let v: Value = resp.json().map_err(|e| e.to_string())?;
    let facts = v["facts"].as_array().ok_or("facts: missing array")?;
    let want_eid = hex::encode(export_id);
    let want_to = hex::encode(to);
    for fact in facts {
        if fact["export_id"].as_str() != Some(want_eid.as_str()) {
            continue;
        }
        let to_str = fact["to"].as_str().ok_or("fact.to")?;
        if to_str != want_to {
            continue;
        }
        let amt: u128 = match &fact["amount"] {
            Value::String(s) => s.parse().map_err(|_| "fact.amount".to_string())?,
            Value::Number(n) => n.as_u64().ok_or_else(|| "fact.amount number".to_string())? as u128,
            _ => continue,
        };
        if amt != amount {
            continue;
        }
        return Ok(ExportProvenance {
            to,
            target_domain,
            amount,
        });
    }
    Err(format!(
        "no matching cross-shard fact for export_id={want_eid} (run tx-handoff-register / roaming finalize relay on target first)"
    ))
}

pub(crate) fn run_tx_handoff_register(rpc_base: &str, handoff_json: PathBuf) {
    let handoff = load_handoff_json(&handoff_json).unwrap_or_else(|e| exit_user_error(&e));
    let c = http_client_for_rpc();
    post_export_handoff(&c, rpc_base, &handoff).unwrap_or_else(|e| exit_user_error(&e));
    eprintln!(
        "tx-handoff-register note: target provenance is registered; run tx-import with the same export_id/to/amount from the handoff"
    );
}

// --- roaming HTTP + import sender (lifted from main) ---

pub(crate) fn ensure_import_sender(
    c: &reqwest::blocking::Client,
    rpc_base: &str,
    source: &TxSignerSource,
) -> Result<u64, String> {
    let from_hex = hex::encode(source.from);
    let sender_url = format!("{rpc_base}/v1/account/{from_hex}");
    let sender_view = c
        .get(&sender_url)
        .send()
        .map_err(|e| map_reqwest_err(&e, "sender preflight"))?;
    let sender_status = sender_view.status();
    let sender_body = sender_view.text().unwrap_or_default();
    if sender_status.is_success() {
        if let Some(meta) = parse_account_lookup_meta(&sender_body) {
            if meta.local_view_only {
                match meta.home_lookup_status.as_deref() {
                    Some("ok") => {
                        return Err(
                            "tx-import: sender is foreign on current RPC; use target-shard RPC where signer account is local"
                                .to_string(),
                        );
                    }
                    Some("unavailable") | None => {
                        return Err(
                            "tx-import: sender home-shard state is unavailable on current RPC (no trusted peer path); switch to target-shard RPC"
                                .to_string(),
                        );
                    }
                    Some(other) => {
                        return Err(format!(
                            "tx-import: sender account is not local and authoritative state is unavailable (home_lookup_status={other}); switch to target-shard RPC"
                        ));
                    }
                }
            }
        }
    }
    let st = parse_nonce_init_response(sender_status, &sender_body, &sender_url)?;
    let need_init = st.map(|(_, initialized)| !initialized).unwrap_or(true);
    if need_init {
        let init_tx = SignedTx::sign_body(
            &source.sk,
            source.dom,
            source.idx,
            0,
            TxBody::Init { index: 0, flags: 0 },
        );
        post_signed_tx(c, rpc_base, &init_tx)?;
        let mut last = st;
        for _ in 0..12 {
            let cur = fetch_nonce_init_opt(c, rpc_base, source.from)?;
            let ok = matches!(cur, Some((_, true)));
            if ok {
                return Ok(cur.unwrap().0);
            }
            last = cur;
            std::thread::sleep(Duration::from_millis(250));
        }

        return Err(if matches!(last, Some((_, false))) {
            "auto tx-init succeeded but sender account is still uninitialized".to_string()
        } else {
            "auto tx-init succeeded but sender account is still missing".to_string()
        });
    }

    st.map(|(nonce, _)| nonce)
        .ok_or_else(|| "sender account missing".to_string())
}

pub(crate) fn post_import_retry_inner(
    c: &reqwest::blocking::Client,
    rpc_base: &str,
    tx: &SignedTx,
    max_attempts: usize,
    retry_delay: Duration,
) -> Result<(), String> {
    let url = format!("{rpc_base}/v1/tx");
    let mut last_err: Option<String> = None;
    for attempt_idx in 0..max_attempts {
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
        let hint = truncate_rpc_body_hint(&body, 400);
        let body_lc = body.to_ascii_lowercase();
        let err = if hint.is_empty() {
            format!("tx import: HTTP {status} ({url})")
        } else {
            format!("tx import: HTTP {status} ({url}): {hint}")
        };
        last_err = Some(err.clone());

        if status == reqwest::StatusCode::BAD_REQUEST
            && body_lc.contains("invalid import")
            && body_lc.contains("export_id is not known")
        {
            let last_attempt = attempt_idx + 1 == max_attempts;
            if !last_attempt {
                if !retry_delay.is_zero() {
                    std::thread::sleep(retry_delay);
                }
                continue;
            }
        }

        return Err(err);
    }
    Err(last_err.unwrap_or_else(|| "tx import retry: exhausted attempts".to_string()))
}

#[derive(serde::Serialize)]
struct CreateRoamingIntentReq<'a> {
    tx: &'a SignedTx,
}

#[derive(serde::Serialize)]
struct ExportReadinessReq<'a> {
    tx: &'a SignedTx,
}

#[derive(serde::Deserialize)]
pub(crate) struct CreateRoamingIntentResp {
    pub(crate) intent_id: String,
    pub(crate) export_id: String,
    pub(crate) status: String,
    pub(crate) duplicate: bool,
}

#[derive(serde::Deserialize)]
pub(crate) struct IntentStatusResp {
    pub(crate) status: String,
    #[serde(default)]
    pub(crate) last_error: Option<String>,
}

pub(crate) fn user_msg_roaming_intent_error(status: reqwest::StatusCode, body: &str) -> String {
    let details = truncate_rpc_body_hint(body, 240);
    let body_lc = body.to_ascii_lowercase();
    if status == reqwest::StatusCode::CONFLICT
        && (body_lc.contains("duplicate") || body_lc.contains("already"))
    {
        return "cross-domain send already started earlier; reusing existing roaming intent".into();
    }
    if body_lc.contains("invalid") || status == reqwest::StatusCode::BAD_REQUEST {
        return if details.is_empty() {
            "cross-domain send request is invalid for this node".into()
        } else {
            format!("cross-domain send request is invalid for this node. details: {details}")
        };
    }
    if body_lc.contains("expired") {
        return "cross-domain send intent expired before completion; retry from home shard".into();
    }
    if details.is_empty() {
        format!("cross-domain send failed with HTTP {status}")
    } else {
        format!("cross-domain send failed with HTTP {status}. details: {details}")
    }
}

pub(crate) fn post_roaming_intent(
    c: &reqwest::blocking::Client,
    rpc_base: &str,
    export_tx: &SignedTx,
) -> Result<CreateRoamingIntentResp, String> {
    let url = format!("{rpc_base}/v1/roaming-intents");
    let req = CreateRoamingIntentReq { tx: export_tx };
    let r = c
        .post(&url)
        .json(&req)
        .send()
        .map_err(|e| map_reqwest_err(&e, "roaming intent create"))?;
    let status = r.status();
    let body = r.text().unwrap_or_default();
    if !status.is_success() {
        return Err(user_msg_roaming_intent_error(status, &body));
    }
    serde_json::from_str::<CreateRoamingIntentResp>(&body).map_err(|e| {
        format!(
            "roaming intent create returned unreadable response: {e}. body={}",
            truncate_rpc_body_hint(&body, 240)
        )
    })
}

pub(crate) fn post_export_readiness(
    c: &reqwest::blocking::Client,
    rpc_base: &str,
    export_tx: &SignedTx,
) -> Result<(), String> {
    let url = format!("{rpc_base}/v1/export-readiness");
    let req = ExportReadinessReq { tx: export_tx };
    let r = c
        .post(&url)
        .json(&req)
        .send()
        .map_err(|e| map_reqwest_err(&e, "export readiness"))?;
    let status = r.status();
    let body = r.text().unwrap_or_default();
    if status.is_success() {
        return Ok(());
    }
    let hint = truncate_rpc_body_hint(&body, 400);
    Err(if hint.is_empty() {
        format!("export readiness: HTTP {status} ({url})")
    } else {
        format!("export readiness: HTTP {status} ({url}): {hint}")
    })
}

pub(crate) fn get_roaming_intent_status(
    c: &reqwest::blocking::Client,
    rpc_base: &str,
    intent_id: &str,
) -> Result<IntentStatusResp, String> {
    let url = format!("{rpc_base}/v1/roaming-intents/{intent_id}");
    let r = c
        .get(&url)
        .send()
        .map_err(|e| map_reqwest_err(&e, "roaming intent status"))?;
    let status = r.status();
    let body = r.text().unwrap_or_default();
    if !status.is_success() {
        return Err(user_msg_roaming_intent_error(status, &body));
    }
    serde_json::from_str::<IntentStatusResp>(&body).map_err(|e| {
        format!(
            "roaming intent status returned unreadable response: {e}. body={}",
            truncate_rpc_body_hint(&body, 240)
        )
    })
}

pub(crate) fn post_export_handoff(
    c: &reqwest::blocking::Client,
    rpc_base: &str,
    handoff: &Value,
) -> Result<(), String> {
    let url = format!("{rpc_base}/v1/export-provenance");
    let r = c
        .post(&url)
        .json(handoff)
        .send()
        .map_err(|e| map_reqwest_err(&e, "handoff register"))?;
    let status = r.status();
    let body = r.text().unwrap_or_default();
    if status.is_success() {
        let hint = truncate_rpc_body_hint(&body, 400);
        println!("{status}");
        if !hint.is_empty() {
            println!("{hint}");
        }
        return Ok(());
    }
    let hint = truncate_rpc_body_hint(&body, 400);
    Err(if hint.is_empty() {
        format!("handoff register: HTTP {status} ({url})")
    } else {
        format!("handoff register: HTTP {status} ({url}): {hint}")
    })
}

pub(crate) fn is_terminal_intent_status(status: &str) -> bool {
    matches!(status, "imported" | "expired" | "failed")
}

pub(crate) fn parse_export_id_hex_arg(raw: &str) -> Result<[u8; 32], String> {
    hex32(raw).map_err(|_| {
        format!(
            "Invalid value for --export-id: expected 32-byte hex (64 hex chars), got `{}`",
            raw.trim()
        )
    })
}
