//! Submit init/transfer/stake transactions via HTTP RPC.

use pwm_core::tx::{ClaimMode, SignedTx, TxBody};
use pwm_core::{summarize_tx_reject_json, AccountId};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::{base_url, http_client, shard_cli_hint};
use crate::rpc_account::{fetch_nonce, preflight_recipient_rpc};
use crate::signing::signing_material_for_sender;
use crate::wallet::IdentitySource;

pub fn is_cross_domain_route(from: &AccountId, to: &AccountId) -> bool {
    !pwm_core::tx::same_hi_domain(from, to)
}

pub fn submit_init(
    from: &AccountId,
    nonce: u64,
    identity: &IdentitySource,
) -> Result<String, String> {
    let (sk, dom, idx) = signing_material_for_sender(from, identity)?;
    let client = http_client();
    let rpc = base_url();
    let tx = SignedTx::sign_body(
        &sk,
        dom,
        idx,
        nonce,
        TxBody::Init {
            index: idx,
            flags: 0,
        },
    );
    let response = client
        .post(format!("{rpc}/v1/tx"))
        .json(&tx)
        .send()
        .map_err(|e| {
            if e.is_timeout() {
                crate::config::rpc_timeout_hint()
            } else {
                format!("rpc error: {e}")
            }
        })?;
    let status = response.status();
    let body = response.text().unwrap_or_default();
    if status.is_success() {
        Ok(format!("auto-init sent: {status}"))
    } else if body.is_empty() {
        Err(format!("auto-init failed: {status}"))
    } else {
        Err(format!("auto-init failed: {status} {body}"))
    }
}

/// Submit `BurnMark` with optional v2 purpose (re-sign when purpose != built-in default).
pub fn submit_burn_mark(
    from: &AccountId,
    mark_amount: u32,
    beneficiary: Option<AccountId>,
    purpose: String,
    identity: &IdentitySource,
) -> Result<String, String> {
    if matches!(
        identity,
        IdentitySource::Wallet(w) if w.wallet_is_encrypted && w.signing_key.is_none()
    ) {
        return Err("Wallet is locked — press F3 to unlock".to_string());
    }
    let (sk, dom, idx) = signing_material_for_sender(from, identity)?;
    let client = http_client();
    let rpc = base_url();
    let nonce = fetch_nonce(&client, &rpc, *from)?;
    let mut tx = SignedTx::sign_body(
        &sk,
        dom,
        idx,
        nonce,
        TxBody::BurnMark {
            mark_amount,
            beneficiary,
        },
    );
    let purpose = expand_purpose(&purpose);
    if purpose != "default" {
        tx.set_burn_purpose_signed(&sk, purpose);
    }
    let response = client
        .post(format!("{rpc}/v1/tx"))
        .json(&tx)
        .send()
        .map_err(|e| {
            if e.is_timeout() {
                crate::config::rpc_timeout_hint()
            } else {
                format!("rpc error: {e}")
            }
        })?;
    let status = response.status();
    let body = response.text().unwrap_or_default();
    if status.is_success() {
        Ok(format!("burn sent: {status}"))
    } else if let Some(hint) = summarize_tx_reject_json(&body) {
        Err(format!("burn failed: {status} {hint}"))
    } else {
        Err(format!(
            "burn failed: {status} {}",
            body.chars()
                .filter(|c| !c.is_control())
                .take(400)
                .collect::<String>()
        ))
    }
}

/// Submits a ClaimTx with CLAIM_ALL sentinel - node materialises all matured marks.
pub fn submit_claim(
    from: &AccountId,
    claim_units: u32,
    anchor_ref: u64,
    identity: &IdentitySource,
) -> Result<(), String> {
    let (sk, dom, idx) = signing_material_for_sender(from, identity)?;
    let client = http_client();
    let rpc = base_url();
    let nonce = fetch_nonce(&client, &rpc, *from)?;
    let tx = SignedTx::sign_body(
        &sk,
        dom,
        idx,
        nonce,
        TxBody::Claim {
            mode: ClaimMode::Free,
            claim_units,
            anchor_ref,
            fee: 0,
        },
    );
    let response = client
        .post(format!("{rpc}/v1/tx"))
        .json(&tx)
        .send()
        .map_err(|e| {
            if e.is_timeout() {
                crate::config::rpc_timeout_hint()
            } else {
                format!("rpc error: {e}")
            }
        })?;
    let status = response.status();
    let body = response.text().unwrap_or_default();
    if status.is_success() {
        Ok(())
    } else if let Some(hint) = summarize_tx_reject_json(&body) {
        Err(format!("claim failed: {status} {hint}"))
    } else {
        Err(format!("claim failed: {status} {body}"))
    }
}

fn expand_purpose(raw: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let ts = now.as_secs();
    let utc_time = fmt_utc_time(ts);
    raw.replace("{utc_timestamp}", &ts.to_string())
        .replace("{utc_time}", &utc_time)
}

fn fmt_utc_time(ts: u64) -> String {
    let days = (ts / 86_400) as i64;
    let sec_day = ts % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hh = sec_day / 3_600;
    let mm = (sec_day % 3_600) / 60;
    let ss = sec_day % 60;
    format!(
        "{:02}-{:02}-{:02} {:02}:{:02}:{:02}Z",
        day,
        month,
        (year % 100).rem_euclid(100),
        hh,
        mm,
        ss
    )
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

pub fn submit_transfer(
    from: &AccountId,
    to: &AccountId,
    amount: u128,
    fee: u128,
    identity: &IdentitySource,
) -> Result<String, String> {
    let (sk, dom, idx) = signing_material_for_sender(from, identity)?;
    let client = http_client();
    let rpc = base_url();
    if !is_cross_domain_route(from, to) {
        preflight_recipient_rpc(&client, &rpc, *to)?;
    }
    let nonce = fetch_nonce(&client, &rpc, *from)?;
    let tx = SignedTx::sign_body(
        &sk,
        dom,
        idx,
        nonce,
        TxBody::Transfer {
            to: *to,
            amount,
            fee,
        },
    );
    let response = client
        .post(format!("{rpc}/v1/tx"))
        .json(&tx)
        .send()
        .map_err(|e| {
            if e.is_timeout() {
                crate::config::rpc_timeout_hint()
            } else {
                format!("rpc error: {e}")
            }
        })?;
    let status = response.status();
    let body = response.text().unwrap_or_default();
    if status.is_success() {
        Ok(format!("sent: {status}"))
    } else if let Some(hint) = summarize_tx_reject_json(&body) {
        Err(format!("submit failed: {status} {hint}"))
    } else {
        Err(format_submit_transfer_error(status, &body, &rpc))
    }
}

pub fn submit_stake(
    from: &AccountId,
    amount: u128,
    _anchor_ref: u64,
    identity: &IdentitySource,
) -> Result<(), String> {
    submit_stake_like(from, amount, identity, true)
}

pub fn submit_unstake(
    from: &AccountId,
    amount: u128,
    _anchor_ref: u64,
    identity: &IdentitySource,
) -> Result<(), String> {
    submit_stake_like(from, amount, identity, false)
}

fn submit_stake_like(
    from: &AccountId,
    amount: u128,
    identity: &IdentitySource,
    is_stake: bool,
) -> Result<(), String> {
    let (sk, dom, idx) = signing_material_for_sender(from, identity)?;
    let client = http_client();
    let rpc = base_url();
    let nonce = fetch_nonce(&client, &rpc, *from)?;
    let body = if is_stake {
        TxBody::Stake { amount }
    } else {
        TxBody::Unstake { amount }
    };
    let tx = SignedTx::sign_body(&sk, dom, idx, nonce, body);
    let response = client
        .post(format!("{rpc}/v1/tx"))
        .json(&tx)
        .send()
        .map_err(|e| {
            if e.is_timeout() {
                crate::config::rpc_timeout_hint()
            } else {
                format!("rpc error: {e}")
            }
        })?;
    let status = response.status();
    let body = response.text().unwrap_or_default();
    if status.is_success() {
        Ok(())
    } else if let Some(hint) = summarize_tx_reject_json(&body) {
        let kind = if is_stake { "stake" } else { "unstake" };
        Err(format!("{kind} failed: {status} {hint}"))
    } else {
        let kind = if is_stake { "stake" } else { "unstake" };
        Err(format!("{kind} failed: {status} {body}"))
    }
}

fn is_xdom_xfer_reject(body: &str) -> bool {
    let body_lc = body.to_ascii_lowercase();
    body_lc.contains("cross-domain transfer is disabled")
        || (body_lc.contains("export") && body_lc.contains("import"))
}

pub fn format_submit_transfer_error(
    status: reqwest::StatusCode,
    body: &str,
    rpc_url: &str,
) -> String {
    if body.trim().is_empty() {
        return format!("submit failed: {status}");
    }
    if is_xdom_xfer_reject(body) {
        return format!(
            "submit failed: {status} {body} | {}",
            shard_cli_hint(rpc_url)
        );
    }
    format!("submit failed: {status} {body}")
}
