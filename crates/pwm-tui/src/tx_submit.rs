//! Submit init/transfer transactions via HTTP RPC.

use pwm_core::tx::{SignedTx, TxBody};
use pwm_core::AccountId;

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
    } else {
        Err(format_submit_transfer_error(status, &body, &rpc))
    }
}

fn is_xdom_xfer_reject_body(body: &str) -> bool {
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
    if is_xdom_xfer_reject_body(body) {
        return format!(
            "submit failed: {status} {body} | {}",
            shard_cli_hint(rpc_url)
        );
    }
    format!("submit failed: {status} {body}")
}
