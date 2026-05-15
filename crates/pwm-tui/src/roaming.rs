//! Cross-shard roaming intent flow (export-readiness, polling, import relay).

use pwm_core::hd::domain_of_account_id;
use pwm_core::state::ExportProvenance;
use pwm_core::tx::{SignedTx, TxBody, MIN_IMPORT_FEE_UNITS};
use pwm_core::AccountId;
use serde_json::Value;

use crate::config::{base_url, http_client, rpc_timeout_hint, xshard_rpc_base};
use crate::rpc_account::{fetch_nonce, truncate_rpc_err_hint};
use crate::signing::signing_material_for_sender;
use crate::wallet::IdentitySource;

#[derive(serde::Serialize)]
struct CreateRoamingIntentReq<'a> {
    tx: &'a SignedTx,
}

#[derive(serde::Serialize)]
struct ExportReadyReq<'a> {
    tx: &'a SignedTx,
}

#[derive(serde::Deserialize)]
struct CreateRoamingIntentResp {
    intent_id: String,
    export_id: String,
    status: String,
    duplicate: bool,
}

#[derive(serde::Deserialize)]
struct IntentStatusResp {
    status: String,
    #[serde(default)]
    last_error: Option<String>,
}

#[derive(serde::Deserialize)]
struct ReadinessRejectResp {
    code: String,
    hint: String,
    message: String,
}

pub fn format_roaming_error(status: reqwest::StatusCode, body: &str) -> String {
    let body_lc = body.to_ascii_lowercase();
    if status == reqwest::StatusCode::CONFLICT
        && (body_lc.contains("duplicate") || body_lc.contains("already"))
    {
        return "Cross-domain send already exists: existing roaming intent will be reused.".into();
    }
    if status == reqwest::StatusCode::BAD_REQUEST || body_lc.contains("invalid") {
        return format!(
            "Cross-domain send rejected: invalid request for roaming intent. details: {}",
            body.trim()
        );
    }
    if body_lc.contains("expired") {
        return "Cross-domain send expired before completion. Retry from home shard.".into();
    }
    if body.trim().is_empty() {
        format!("Cross-domain send failed: HTTP {status}")
    } else {
        format!(
            "Cross-domain send failed: HTTP {status}. details: {}",
            body.trim()
        )
    }
}

/// Hint appended when relay/handoff path needs manual follow-up (cross-shard roaming flow).
const XFLOW_HANDOFF_HELP: &str = "Если auto-flow не доводит до импорта: на target с trusted seed context выполните `pwm tx-handoff-register`, затем `pwm tx-import`.";

fn xflow_preflight_fail(status: reqwest::StatusCode, body: &str) -> String {
    let mut hint = format!("HTTP {status}");
    if let Ok(parsed) = serde_json::from_str::<ReadinessRejectResp>(body) {
        hint = format!("{} ({})", parsed.message, parsed.hint);
    } else if !body.trim().is_empty() {
        hint = format!("HTTP {status}: {}", body.trim());
    }
    format!(
        "Cross-shard flow diagnostics:\n{}",
        [
            format!("1) preflight (export-readiness): FAIL - {hint}"),
            "2) export submit: SKIP - preflight не пройден.".to_string(),
            format!("3) handoff/provenance register: INFO - {XFLOW_HANDOFF_HELP}"),
            "4) import submit: SKIP - дождитесь успешного export submit.".to_string(),
            "5) balance verify (target): SKIP — нет импорта.".to_string(),
        ]
        .join("\n")
    )
}

fn xflow_export_fail(status: reqwest::StatusCode, body: &str) -> String {
    let mut detail = if body.trim().is_empty() {
        format!("HTTP {status}")
    } else {
        format!("HTTP {status}: {}", body.trim())
    };
    if let Ok(parsed) = serde_json::from_str::<ReadinessRejectResp>(body) {
        detail = format!(
            "{} (code={}, hint={})",
            parsed.message, parsed.code, parsed.hint
        );
    }
    format!(
        "Cross-shard flow diagnostics:\n{}",
        [
            "1) preflight (export-readiness): OK - readiness подтверждён.".to_string(),
            format!("2) export submit (roaming/export intent): FAIL - {detail}"),
            format!("3) handoff/provenance register: INFO - {XFLOW_HANDOFF_HELP}"),
            "4) import submit: SKIP - экспорт не зафиксирован.".to_string(),
            "5) balance verify (target): SKIP — нет импорта.".to_string(),
        ]
        .join("\n")
    )
}

fn xflow_terminal_report(
    intent_id: &str,
    export_id: &str,
    duplicate: bool,
    seen_relayed: bool,
    final_status: &str,
    last_error: Option<&str>,
    step5: Option<String>,
) -> Result<String, String> {
    let submit_note = if duplicate {
        format!(
            "2) export submit (roaming/export intent): OK - intent={} export={} (duplicate reused).",
            intent_id, export_id
        )
    } else {
        format!(
            "2) export submit (roaming/export intent): OK - intent={} export={}.",
            intent_id, export_id
        )
    };
    let handoff_line = if seen_relayed {
        "3) handoff/provenance register: OK - provenance доставлен (relayed).".to_string()
    } else if final_status == "exported" {
        if let Some(err) = last_error {
            format!(
                "3) handoff/provenance register: FAIL - relay: {err} (status остаётся exported; исправьте transport/peer и при необходимости вызовите finalize)."
            )
        } else {
            format!(
                "3) handoff/provenance register: INFO - relay не подтверждён, проверьте вручную. {XFLOW_HANDOFF_HELP}"
            )
        }
    } else {
        format!(
            "3) handoff/provenance register: INFO - relay не подтверждён, проверьте вручную. {XFLOW_HANDOFF_HELP}"
        )
    };
    let step5_line = step5.unwrap_or_else(|| "5) balance verify (target): SKIP.".to_string());
    match final_status {
        "imported" => Ok(format!(
            "Cross-shard flow diagnostics:\n{}",
            [
                "1) preflight (export-readiness): OK - readiness подтверждён.".to_string(),
                submit_note.clone(),
                handoff_line.clone(),
                "4) import submit: OK - импорт подтверждён (status=imported).".to_string(),
                step5_line.clone(),
            ]
            .join("\n")
        )),
        "expired" => Err(format!(
            "Cross-shard flow diagnostics:\n{}",
            [
                "1) preflight (export-readiness): OK - readiness подтверждён.".to_string(),
                submit_note.clone(),
                handoff_line.clone(),
                "4) import submit: FAIL - lifecycle истёк (expired). Повторите отправку с source shard.".to_string(),
                step5_line.clone(),
            ]
            .join("\n")
        )),
        "failed" => Err(format!(
            "Cross-shard flow diagnostics:\n{}",
            [
                "1) preflight (export-readiness): OK - readiness подтверждён.".to_string(),
                submit_note.clone(),
                handoff_line.clone(),
                format!(
                    "4) import submit: FAIL - {}",
                    last_error.unwrap_or("неизвестная ошибка import-этапа")
                ),
                step5_line.clone(),
            ]
            .join("\n")
        )),
        _ => Ok(format!(
            "Cross-shard flow diagnostics:\n{}",
            [
                "1) preflight (export-readiness): OK - readiness подтверждён.".to_string(),
                submit_note,
                handoff_line,
                format!(
                    "4) import submit: INFO - текущий статус `{final_status}`. Проверьте GET /v1/roaming-intents/{intent_id}."
                ),
                step5_line,
            ]
            .join("\n")
        )),
    }
}

pub fn submit_roaming_intent(
    from: &AccountId,
    to: &AccountId,
    amount: u128,
    fee: u128,
    identity: &IdentitySource,
) -> Result<String, String> {
    let (sk, dom, idx) = signing_material_for_sender(from, identity)?;
    let client = http_client();
    let rpc = base_url();
    let target_rpc = xshard_rpc_base();
    let nonce = fetch_nonce(&client, &rpc, *from)?;
    let export_tx = SignedTx::sign_body(
        &sk,
        dom,
        idx,
        nonce,
        TxBody::Export {
            to: *to,
            target_domain: domain_of_account_id(to),
            amount,
            fee,
        },
    );
    let preflight = client
        .post(format!("{}/v1/export-readiness", rpc))
        .json(&ExportReadyReq { tx: &export_tx })
        .send()
        .map_err(|e| {
            if e.is_timeout() {
                rpc_timeout_hint()
            } else {
                format!("rpc error: {e}")
            }
        })?;
    let preflight_status = preflight.status();
    let preflight_body = preflight.text().unwrap_or_default();
    if !preflight_status.is_success() {
        return Err(xflow_preflight_fail(preflight_status, &preflight_body));
    }
    let create = client
        .post(format!("{}/v1/roaming-intents", rpc))
        .json(&CreateRoamingIntentReq { tx: &export_tx })
        .send()
        .map_err(|e| {
            if e.is_timeout() {
                rpc_timeout_hint()
            } else {
                format!("rpc error: {e}")
            }
        })?;
    let create_status = create.status();
    let create_body = create.text().unwrap_or_default();
    if !create_status.is_success() {
        return Err(xflow_export_fail(create_status, &create_body));
    }
    let created: CreateRoamingIntentResp = serde_json::from_str(&create_body)
        .map_err(|e| format!("invalid roaming create response: {e}"))?;
    let mut seen_relayed = created.status == "relayed";
    let mut last_seen = created.status.clone();
    let mut last_poll_error: Option<String> = None;
    let mut import_submitted = false;
    let mut import_fee_used: Option<u128> = None;
    let mut pre_recv_bal: Option<u128> = None;

    if created.status == "relayed" {
        seen_relayed = true;
        if !import_submitted {
            if pre_recv_bal.is_none() {
                pre_recv_bal = fetch_account_balance_raw(&client, &target_rpc, *to).ok();
            }
            let imp_fee = submit_import_after_relay(to, amount, &created.export_id, identity)?;
            import_fee_used = Some(imp_fee);
            import_submitted = true;
        }
    }

    for _ in 0..12 {
        let status_resp = client
            .get(format!("{}/v1/roaming-intents/{}", rpc, created.intent_id))
            .send()
            .map_err(|e| {
                if e.is_timeout() {
                    rpc_timeout_hint()
                } else {
                    format!("rpc error: {e}")
                }
            })?;
        let status = status_resp.status();
        let body = status_resp.text().unwrap_or_default();
        if !status.is_success() {
            return Err(format_roaming_error(status, &body));
        }
        let st: IntentStatusResp = serde_json::from_str(&body)
            .map_err(|e| format!("invalid roaming status response: {e}"))?;
        if st.status == "relayed" {
            seen_relayed = true;
            if !import_submitted {
                if pre_recv_bal.is_none() {
                    pre_recv_bal = fetch_account_balance_raw(&client, &target_rpc, *to).ok();
                }
                match submit_import_after_relay(to, amount, &created.export_id, identity) {
                    Ok(imp_fee) => import_fee_used = Some(imp_fee),
                    Err(e) => {
                        return Err(format!(
                            "Cross-shard flow diagnostics:\n{}",
                            [
                                "1) preflight (export-readiness): OK - readiness подтверждён.".to_string(),
                                if created.duplicate {
                                    format!(
                                        "2) export submit (roaming/export intent): OK - intent={} export={} (duplicate reused).",
                                        created.intent_id, created.export_id
                                    )
                                } else {
                                    format!(
                                        "2) export submit (roaming/export intent): OK - intent={} export={}.",
                                        created.intent_id, created.export_id
                                    )
                                },
                                "3) handoff/provenance register: OK - provenance доставлен (relayed)."
                                    .to_string(),
                                format!("4) import submit: FAIL — {e}"),
                                "5) balance verify (target): SKIP — import not applied.".to_string(),
                            ]
                            .join("\n")
                        ));
                    }
                }
                import_submitted = true;
            }
        }
        last_seen = st.status.clone();
        last_poll_error = st.last_error.clone();
        if matches!(st.status.as_str(), "imported" | "expired" | "failed") {
            let post_bal = if st.status == "imported" {
                fetch_account_balance_raw(&client, &target_rpc, *to).ok()
            } else {
                None
            };
            let step5 = if st.status == "imported" {
                let import_fee = import_fee_used.unwrap_or(MIN_IMPORT_FEE_UNITS);
                let expected_delta = amount.saturating_sub(import_fee);
                format_balance_verify_step5(
                    &target_rpc,
                    pre_recv_bal,
                    post_bal,
                    amount,
                    expected_delta,
                    fee,
                    import_fee,
                )
            } else {
                "5) balance verify (target): SKIP — lifecycle not imported.".into()
            };
            return xflow_terminal_report(
                &created.intent_id,
                &created.export_id,
                created.duplicate,
                seen_relayed,
                &st.status,
                st.last_error.as_deref(),
                Some(step5),
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(600));
    }
    xflow_terminal_report(
        &created.intent_id,
        &created.export_id,
        created.duplicate,
        seen_relayed,
        &last_seen,
        last_poll_error.as_deref(),
        Some("5) balance verify (target): SKIP — polling stopped before imported.".into()),
    )
}

/// Fetches matching import provenance from target facts in one attempt.
fn import_prov_once(
    c: &reqwest::blocking::Client,
    rpc_base: &str,
    export_id: &[u8; 32],
    to: &AccountId,
    amount_raw: u128,
) -> Result<ExportProvenance, String> {
    let target_domain = domain_of_account_id(to);
    let hi = target_domain.to_be_bytes()[0];
    let url = format!(
        "{}/v1/cross-shard/facts?target_domain_hi={}&from_height=0&limit=512",
        rpc_base.trim_end_matches('/'),
        hi
    );
    let resp = c.get(&url).send().map_err(|e| {
        if e.is_timeout() {
            format!("cross-shard facts: {}", rpc_timeout_hint())
        } else {
            format!("cross-shard facts: {e}")
        }
    })?;
    if !resp.status().is_success() {
        return Err(format!(
            "cross-shard facts HTTP {} (target must list facts after relay/handoff)",
            resp.status()
        ));
    }
    let body = resp.text().unwrap_or_default();
    let v: Value = serde_json::from_str(&body).map_err(|e| format!("facts json: {e}"))?;
    let facts = v["facts"].as_array().ok_or("facts: missing array")?;
    let want_eid = hex::encode(export_id);
    let want_to = hex::encode(to);
    for fact in facts {
        if fact["export_id"].as_str() != Some(want_eid.as_str()) {
            continue;
        }
        let to_str = fact["to"].as_str().ok_or("fact.to missing")?;
        if to_str != want_to {
            continue;
        }
        let amt: u128 = match &fact["amount"] {
            Value::String(s) => s.parse().map_err(|_| "fact.amount parse".to_string())?,
            Value::Number(n) => n.as_u64().ok_or_else(|| "fact.amount number".to_string())? as u128,
            _ => continue,
        };
        if amt != amount_raw {
            continue;
        }
        return Ok(ExportProvenance {
            to: *to,
            target_domain,
            amount: amount_raw,
        });
    }
    Err(format!(
        "no matching cross-shard fact for export_id={want_eid} (wait for relay or run handoff on target)"
    ))
}

/// Retries target fact polling until provenance is found or attempts are exhausted.
fn import_prov_retry(
    c: &reqwest::blocking::Client,
    rpc_base: &str,
    export_id: &[u8; 32],
    to: &AccountId,
    amount_raw: u128,
) -> Result<ExportProvenance, String> {
    let mut last = String::new();
    for attempt in 0..24usize {
        match import_prov_once(c, rpc_base, export_id, to, amount_raw) {
            Ok(p) => return Ok(p),
            Err(e) => {
                last = e;
                if attempt + 1 < 24 {
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    continue;
                }
            }
        }
    }
    Err(last)
}

fn parse_export_id_hex(s: &str) -> Result<[u8; 32], String> {
    let v = hex::decode(s.trim()).map_err(|e| format!("export_id hex: {e}"))?;
    if v.len() != 32 {
        return Err(format!("export_id must be 32 bytes, got {} bytes", v.len()));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&v);
    Ok(out)
}

fn fetch_account_balance_raw(
    c: &reqwest::blocking::Client,
    rpc_base: &str,
    acct: AccountId,
) -> Result<u128, String> {
    let url = format!("{}/v1/account/{}", rpc_base, hex::encode(acct));
    let r = c.get(&url).send().map_err(|e| {
        if e.is_timeout() {
            format!("balance fetch: {}", rpc_timeout_hint())
        } else {
            format!("balance fetch: {e}")
        }
    })?;
    if !r.status().is_success() {
        return Err(format!(
            "balance fetch: HTTP {} from {url}",
            r.status().as_u16()
        ));
    }
    let body = r.text().unwrap_or_default();
    let v: Value = serde_json::from_str(&body)
        .map_err(|_| "balance fetch: invalid account json".to_string())?;
    let s = v
        .get("local_state_balance")
        .and_then(|x| x.as_str())
        .or_else(|| v.get("balance_pwm").and_then(|x| x.as_str()))
        .ok_or_else(|| "balance fetch: missing local_state_balance / balance_pwm".to_string())?;
    s.parse()
        .map_err(|e| format!("balance fetch: parse balance: {e}"))
}

/// Relays the import tx to the source-shard relay endpoint.
fn relay_imp_tx(
    c: &reqwest::blocking::Client,
    source_rpc: &str,
    tx: &SignedTx,
) -> Result<(), String> {
    let url = format!("{}/v1/tx", source_rpc);
    let mut last_err = String::new();
    for attempt in 0..20usize {
        let r = c.post(&url).json(tx).send().map_err(|e| {
            if e.is_timeout() {
                rpc_timeout_hint()
            } else {
                format!("import submit: {e}")
            }
        })?;
        if r.status().is_success() {
            return Ok(());
        }
        let status = r.status();
        let body = r.text().unwrap_or_default();
        let hint = truncate_rpc_err_hint(&body, 400);
        last_err = if hint.is_empty() {
            format!("import submit: HTTP {status} ({url})")
        } else {
            format!("import submit: HTTP {status} ({url}): {hint}")
        };
        let body_lc = body.to_ascii_lowercase();
        if status == reqwest::StatusCode::BAD_REQUEST
            && (body_lc.contains("export_id is not known")
                || body_lc.contains("embedded provenance is missing")
                || body_lc.contains("embedded provenance mismatch"))
            && attempt + 1 < 20
        {
            std::thread::sleep(std::time::Duration::from_millis(500));
            continue;
        }
        return Err(last_err);
    }
    Err(last_err)
}

fn submit_import_after_relay(
    to: &AccountId,
    amount_raw: u128,
    export_id_hex: &str,
    identity: &IdentitySource,
) -> Result<u128, String> {
    let export_id = parse_export_id_hex(export_id_hex)?;
    let (sk, dom, idx) = signing_material_for_sender(to, identity)?;
    let client = http_client();
    let target_rpc = xshard_rpc_base();
    let nonce = fetch_nonce(&client, &target_rpc, *to)?;
    let prov = import_prov_retry(&client, &target_rpc, &export_id, to, amount_raw)?;
    let mut tx = SignedTx::sign_body(
        &sk,
        dom,
        idx,
        nonce,
        TxBody::Import {
            to: *to,
            amount: amount_raw,
            export_id,
        },
    );
    tx.set_import_provenance_signed(&sk, Some(prov));
    let import_fee = tx.import_fee.unwrap_or(MIN_IMPORT_FEE_UNITS);
    relay_imp_tx(&client, &base_url(), &tx)?;
    Ok(import_fee)
}

fn format_balance_verify_step5(
    target_rpc: &str,
    pre: Option<u128>,
    post: Option<u128>,
    amount_raw: u128,
    expected_delta: u128,
    export_fee_raw: u128,
    import_fee_raw: u128,
) -> String {
    let fee_note = format!(
        " Source: export debits amount+export_fee={export_fee_raw} raw. Target: import fee={import_fee_raw} raw. Expected net delta = amount({amount_raw}) - import_fee({import_fee_raw})."
    );
    match (pre, post) {
        (Some(b0), Some(b1)) => {
            let d = b1.saturating_sub(b0);
            if d == expected_delta {
                format!(
                    "5) balance verify (target): OK — delta={d} raw, expected net delta {expected_delta} (= amount {amount_raw} - import_fee {import_fee_raw}).{fee_note} rpc={target_rpc}"
                )
            } else {
                format!(
                    "5) balance verify (target): FAIL — delta={d} raw, expected net delta {expected_delta} (= amount {amount_raw} - import_fee {import_fee_raw}).{fee_note} pre={b0} post={b1} rpc={target_rpc}"
                )
            }
        }
        (_, Some(b1)) => {
            format!(
                "5) balance verify (target): INFO — post_balance={b1} raw, expected net delta {expected_delta} (= amount {amount_raw} - import_fee {import_fee_raw}).{fee_note} (pre-balance unavailable) rpc={target_rpc}"
            )
        }
        _ => {
            format!(
                "5) balance verify (target): INFO — could not read account on target (set PWM_TUI_TARGET_RPC if port flip heuristic is wrong). expected net delta {expected_delta} (= amount {amount_raw} - import_fee {import_fee_raw}).{fee_note} rpc={target_rpc}"
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::format_balance_verify_step5;

    #[test]
    fn step5_target_uses_net_delta() {
        let msg = format_balance_verify_step5(
            "http://127.0.0.1:3031",
            Some(1_000_000_000),
            Some(1_000_990_000),
            1_000_000,
            990_000,
            5_000,
            10_000,
        );
        assert!(msg.contains("OK"), "{msg}");
        assert!(msg.contains("delta=990000 raw"), "{msg}");
        assert!(
            msg.contains("expected net delta 990000 (= amount 1000000 - import_fee 10000)"),
            "{msg}"
        );
    }

    #[test]
    fn step5_text_mentions_import_fee() {
        let msg = format_balance_verify_step5(
            "http://127.0.0.1:3031",
            Some(2_000_000_000),
            Some(2_001_000_000),
            1_000_000,
            990_000,
            20_000,
            10_000,
        );
        assert!(msg.contains("FAIL"), "{msg}");
        assert!(
            msg.contains("Source: export debits amount+export_fee=20000 raw."),
            "{msg}"
        );
        assert!(msg.contains("Target: import fee=10000 raw."), "{msg}");
    }
}
