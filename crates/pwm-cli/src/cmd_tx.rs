//! Local tx submission paths (`tx-init`, `tx-policy-*`, `tx-send`, `tx-stake`, `tx-unstake`, `tx-burn-mark`).

use crate::cli_parse::{hex32, parse_address_arg, parse_address_input, resolve_tx_send_amount};
use crate::cmd_roaming;
use crate::purpose_expand::expand_purpose;
use crate::rpc_helpers::{
    fetch_marks, fetch_nonce, fetch_nonce_init_opt, post_signed_tx, preflight_recipient_init,
};
use crate::signer::{load_tx_signer_source, load_wallet_account_signer};
use crate::wallet::assert_tx_recipient_allowed;
use crate::wallet::{load_wallet_yaml_upgrade, wallet_account_list};
use crate::{exit_user_error, http_client_for_rpc};
use pwm_core::crypto::sign;
use pwm_core::tx::{
    ActivationMode, CosignRole, Cosignature, InitPolicyEntry, InitV4Extension, PolicyAction,
    PolicyKind, SignedTx, TxBody,
};
use pwm_core::validate_recipient_domain_policy;
use pwm_core::{account_id_to_human, AccountId};
use std::fs;
use std::path::Path;
use std::path::PathBuf;

pub(crate) struct InitV4Args {
    pub owner_kind: Option<String>,
    pub owner_name: Option<String>,
    pub owner_country: Option<String>,
    pub metadata_commitment: Option<String>,
    pub verification_ref: Option<String>,
    pub requested_domain_lo: Option<u8>,
    pub rescue_address: Option<String>,
    pub initial_policies: Vec<String>,
}

pub(crate) struct RescueCosignArgs {
    pub rescue_account_index: Option<u32>,
    pub rescue_wallet: Option<PathBuf>,
    pub rescue_master: Option<String>,
    pub rescue_domain: Option<String>,
    pub rescue_passphrase: Option<String>,
}

pub(crate) fn run_tx_init(
    rpc_base: &str,
    wallet: Option<PathBuf>,
    master: Option<String>,
    domain: Option<String>,
    wallet_passphrase: Option<&str>,
    upgrade_wallet: bool,
    index: u32,
    flags: u32,
    init_v4: InitV4Args,
    save_activation_tx: Option<PathBuf>,
) {
    let source = load_tx_wallet_signer(
        wallet.clone(),
        master,
        domain,
        wallet_passphrase,
        upgrade_wallet,
        index,
    )
    .unwrap_or_else(|e| exit_user_error(&e));
    let c = http_client_for_rpc();
    let init_nonce = fetch_nonce_init_opt(&c, rpc_base, source.from)
        .unwrap_or_else(|e| exit_user_error(&e))
        .map(|(nonce, _)| nonce)
        .unwrap_or(0);
    let mut tx = SignedTx::sign_body(
        &source.sk,
        source.dom,
        source.idx,
        init_nonce,
        TxBody::Init { index, flags },
    );
    let init_v4 = parse_init_v4_args(init_v4).unwrap_or_else(|e| exit_user_error(&e));
    if let Some(path) = save_activation_tx.as_ref() {
        let prepared = build_init_activation(
            &source,
            init_v4.as_ref(),
            wallet.as_ref(),
            wallet_passphrase,
            upgrade_wallet,
            init_nonce,
        )
        .unwrap_or_else(|e| exit_user_error(&e))
        .unwrap_or_else(|| {
            exit_user_error(
                "--save-activation-tx requires --rescue-address and an emergency initial policy",
            )
        });
        save_signed_tx(path, &prepared).unwrap_or_else(|e| exit_user_error(&e));
    }
    if init_v4.is_some() {
        tx.set_init_v4_signed(&source.sk, init_v4);
    }
    post_signed_tx(&c, rpc_base, &tx).unwrap_or_else(|e| exit_user_error(&e));
}

fn load_tx_wallet_signer(
    wallet: Option<PathBuf>,
    master: Option<String>,
    domain: Option<String>,
    wallet_passphrase: Option<&str>,
    upgrade_wallet: bool,
    index: u32,
) -> Result<crate::signer::TxSignerSource, String> {
    if master.is_some() {
        return load_tx_signer_source(wallet, master, domain, wallet_passphrase, upgrade_wallet);
    }
    let wallet_path =
        wallet.ok_or_else(|| "either --wallet or --master must be provided".to_string())?;
    let sel_idx = resolve_tx_wallet_index(&wallet_path, upgrade_wallet, index)?;
    load_wallet_account_signer(&wallet_path, sel_idx, wallet_passphrase, upgrade_wallet)
}

fn resolve_tx_wallet_index(
    path: &PathBuf,
    upgrade_wallet: bool,
    index: u32,
) -> Result<u32, String> {
    if index != 0 {
        return Ok(index);
    }
    let wallet = load_wallet_yaml_upgrade(path, upgrade_wallet)
        .map_err(|e| format!("failed to read wallet '{}': {e}", path.display()))?;
    if wallet.schema_version == 3 || wallet.derivation_index == 0 {
        return Ok(0);
    }
    Ok(wallet.derivation_index)
}

pub(crate) fn run_tx_policy_set(
    rpc_base: &str,
    wallet: Option<PathBuf>,
    master: Option<String>,
    domain: Option<String>,
    wallet_passphrase: Option<&str>,
    upgrade_wallet: bool,
    index: u32,
    policy: String,
    activation: String,
    activate_at_height: Option<u64>,
    fee: u128,
) {
    let source = load_tx_wallet_signer(
        wallet,
        master,
        domain,
        wallet_passphrase,
        upgrade_wallet,
        index,
    )
    .unwrap_or_else(|e| exit_user_error(&e));
    let policy_kind = parse_policy_kind(&policy).unwrap_or_else(|e| exit_user_error(&e));
    let activation_mode = parse_activation_mode(&activation, activate_at_height)
        .unwrap_or_else(|e| exit_user_error(&e));
    let c = http_client_for_rpc();
    let nonce = fetch_nonce(&c, rpc_base, source.from).unwrap_or_else(|e| exit_user_error(&e));
    let tx = SignedTx::sign_body(
        &source.sk,
        source.dom,
        source.idx,
        nonce,
        TxBody::Policy {
            target_account: source.from,
            action: PolicyAction::SetPolicy {
                policy: policy_kind,
                activation: activation_mode,
            },
            fee,
        },
    );
    post_signed_tx(&c, rpc_base, &tx).unwrap_or_else(|e| exit_user_error(&e));
}

pub(crate) fn run_tx_policy_activate(
    rpc_base: &str,
    wallet: Option<PathBuf>,
    master: Option<String>,
    domain: Option<String>,
    wallet_passphrase: Option<&str>,
    upgrade_wallet: bool,
    index: u32,
    policy: Option<String>,
    policy_id: Option<u8>,
    fee: u128,
    activation_target: Option<String>,
    activation_tx: Option<PathBuf>,
    rescue: RescueCosignArgs,
) {
    if let Some(path) = activation_tx {
        let tx = load_signed_tx(&path).unwrap_or_else(|e| exit_user_error(&e));
        let c = http_client_for_rpc();
        post_signed_tx(&c, rpc_base, &tx).unwrap_or_else(|e| {
            let rich = enrich_act_nonce_err(&c, rpc_base, &tx, &e);
            exit_user_error(&rich);
        });
        return;
    }
    let source = load_tx_wallet_signer(
        wallet.clone(),
        master,
        domain,
        wallet_passphrase,
        upgrade_wallet,
        index,
    )
    .unwrap_or_else(|e| exit_user_error(&e));
    let chosen_policy =
        parse_policy_selector(policy, policy_id).unwrap_or_else(|e| exit_user_error(&e));
    let c = http_client_for_rpc();
    let nonce = fetch_nonce(&c, rpc_base, source.from).unwrap_or_else(|e| exit_user_error(&e));
    let activation_target = activation_target
        .as_deref()
        .map(|raw| parse_address_arg("--activation-target", raw))
        .transpose()
        .unwrap_or_else(|e| exit_user_error(&e));
    let mut tx = SignedTx::sign_body(
        &source.sk,
        source.dom,
        source.idx,
        nonce,
        TxBody::Policy {
            target_account: source.from,
            action: PolicyAction::ActivatePolicy {
                policy_id: chosen_policy,
                activation_target,
            },
            fee,
        },
    );
    let emergency_id = PolicyKind::RoutingEmergencyRedirect.policy_id();
    if chosen_policy != emergency_id {
        if rescue_has_any(&rescue) {
            exit_user_error("rescue cosign flags are only valid for emergency routing activation");
        }
    } else if rescue_has_any(&rescue) {
        let rescue_source = load_rescue_source(wallet, wallet_passphrase, upgrade_wallet, rescue)
            .unwrap_or_else(|e| exit_user_error(&e));
        append_rescue_cosign(&mut tx, &rescue_source.sk);
    }
    post_signed_tx(&c, rpc_base, &tx).unwrap_or_else(|e| exit_user_error(&e));
}

pub(crate) fn run_tx_policy_deactivate(
    rpc_base: &str,
    wallet: Option<PathBuf>,
    master: Option<String>,
    domain: Option<String>,
    wallet_passphrase: Option<&str>,
    upgrade_wallet: bool,
    index: u32,
    policy: Option<String>,
    policy_id: Option<u8>,
    fee: u128,
) {
    let source = load_tx_wallet_signer(
        wallet,
        master,
        domain,
        wallet_passphrase,
        upgrade_wallet,
        index,
    )
    .unwrap_or_else(|e| exit_user_error(&e));
    let chosen_policy =
        parse_policy_selector(policy, policy_id).unwrap_or_else(|e| exit_user_error(&e));
    let c = http_client_for_rpc();
    let nonce = fetch_nonce(&c, rpc_base, source.from).unwrap_or_else(|e| exit_user_error(&e));
    let tx = SignedTx::sign_body(
        &source.sk,
        source.dom,
        source.idx,
        nonce,
        TxBody::Policy {
            target_account: source.from,
            action: PolicyAction::DeactivatePolicy {
                policy_id: chosen_policy,
            },
            fee,
        },
    );
    post_signed_tx(&c, rpc_base, &tx).unwrap_or_else(|e| exit_user_error(&e));
}

pub(crate) fn run_tx_send(
    rpc_base: &str,
    wallet: Option<PathBuf>,
    master: Option<String>,
    domain: Option<String>,
    wallet_passphrase: Option<&str>,
    upgrade_wallet: bool,
    index: u32,
    to: String,
    amount: Option<u128>,
    fee: u128,
) {
    let source = load_tx_wallet_signer(
        wallet.clone(),
        master.clone(),
        domain,
        wallet_passphrase,
        upgrade_wallet,
        index,
    )
    .unwrap_or_else(|e| exit_user_error(&e));
    let (to_id, uri_amount) =
        parse_address_input("--to", &to).unwrap_or_else(|e| exit_user_error(&e));
    let amount = resolve_tx_send_amount(amount, uri_amount).unwrap_or_else(|e| exit_user_error(&e));
    validate_recipient_domain_policy(&to_id, Some("--to")).unwrap_or_else(|e| exit_user_error(&e));
    if master.is_none() {
        if let Some(ref wp) = wallet {
            let doc = load_wallet_yaml_upgrade(wp, upgrade_wallet).unwrap_or_else(|e| {
                exit_user_error(&format!("failed to read wallet for address_book: {e}"))
            });
            assert_tx_recipient_allowed(&doc, &to_id).unwrap_or_else(|e| exit_user_error(&e));
        }
    }
    let c = http_client_for_rpc();
    let same_hi = pwm_core::tx::same_hi_domain(&source.from, &to_id);
    if same_hi {
        preflight_recipient_init(&c, rpc_base, to_id, "tx-send")
            .unwrap_or_else(|e| exit_user_error(&e));
    } else {
        eprintln!(
            "tx-send note: target recipient preflight is unavailable in this source-RPC flow; target tx-import will reject missing/uninitialized recipients"
        );
    }
    let nonce = fetch_nonce(&c, rpc_base, source.from).unwrap_or_else(|e| exit_user_error(&e));
    let tx = SignedTx::sign_body(
        &source.sk,
        source.dom,
        source.idx,
        nonce,
        TxBody::Transfer {
            to: to_id,
            amount,
            fee,
        },
    );
    if same_hi {
        post_signed_tx(&c, rpc_base, &tx).unwrap_or_else(|e| exit_user_error(&e));
    } else {
        cmd_roaming::run_roaming_tx(rpc_base, &source, to_id, amount, fee, nonce);
    }
}

pub(crate) fn run_tx_stake(
    rpc_base: &str,
    wallet: Option<PathBuf>,
    master: Option<String>,
    domain: Option<String>,
    wallet_passphrase: Option<&str>,
    upgrade_wallet: bool,
    index: u32,
    amount: u128,
) {
    let source = load_tx_wallet_signer(
        wallet,
        master,
        domain,
        wallet_passphrase,
        upgrade_wallet,
        index,
    )
    .unwrap_or_else(|e| exit_user_error(&e));
    let c = http_client_for_rpc();
    let nonce = fetch_nonce(&c, rpc_base, source.from).unwrap_or_else(|e| exit_user_error(&e));
    let tx = SignedTx::sign_body(
        &source.sk,
        source.dom,
        source.idx,
        nonce,
        TxBody::Stake { amount },
    );
    post_signed_tx(&c, rpc_base, &tx).unwrap_or_else(|e| exit_user_error(&e));
}

pub(crate) fn run_tx_unstake(
    rpc_base: &str,
    wallet: Option<PathBuf>,
    master: Option<String>,
    domain: Option<String>,
    wallet_passphrase: Option<&str>,
    upgrade_wallet: bool,
    index: u32,
    amount: u128,
) {
    let source = load_tx_wallet_signer(
        wallet,
        master,
        domain,
        wallet_passphrase,
        upgrade_wallet,
        index,
    )
    .unwrap_or_else(|e| exit_user_error(&e));
    let c = http_client_for_rpc();
    let nonce = fetch_nonce(&c, rpc_base, source.from).unwrap_or_else(|e| exit_user_error(&e));
    let tx = SignedTx::sign_body(
        &source.sk,
        source.dom,
        source.idx,
        nonce,
        TxBody::Unstake { amount },
    );
    post_signed_tx(&c, rpc_base, &tx).unwrap_or_else(|e| exit_user_error(&e));
}

fn parse_policy_kind(raw: &str) -> Result<PolicyKind, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "routing.same_domain_only" | "routing-same-domain-only" | "same_domain_only"
        | "same-domain-only" => Ok(PolicyKind::RoutingSameDomainOnly),
        "routing.emergency_redirect" | "routing-emergency-redirect" | "emergency_redirect"
        | "emergency-redirect" => Ok(PolicyKind::RoutingEmergencyRedirect),
        "sender_filter" | "sender-filter" => Ok(PolicyKind::SenderFilter),
        "default_behavior" | "default-behavior" => Ok(PolicyKind::DefaultBehavior),
        "cosign_required" | "cosign-required" => Ok(PolicyKind::CosignRequired),
        other => Err(format!(
            "invalid policy kind '{other}'; use one of: sender_filter, routing.emergency_redirect, routing.same_domain_only, default_behavior, cosign_required"
        )),
    }
}

fn parse_activation_mode(
    raw: &str,
    activate_at_height: Option<u64>,
) -> Result<ActivationMode, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "dormant" => Ok(ActivationMode::Dormant),
        "immediately" | "immediate" | "active" => Ok(ActivationMode::Immediately),
        "deferred" => {
            let at = activate_at_height.ok_or_else(|| {
                "activation=deferred requires --activate-at-height > 0".to_string()
            })?;
            if at == 0 {
                return Err("activation=deferred requires --activate-at-height > 0".to_string());
            }
            Ok(ActivationMode::Deferred {
                activate_at_height: at,
            })
        }
        other => Err(format!(
            "invalid activation mode '{other}'; expected `dormant`, `immediately`, or `deferred`"
        )),
    }
}

#[cfg(test)]
pub(crate) fn parse_claim_mode_cli(raw: &str) -> Result<(), String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "free" | "paid" => Ok(()),
        other => Err(format!(
            "invalid --claim-mode '{other}'; expected `free` or `paid`"
        )),
    }
}

#[cfg(test)]
pub(crate) fn parse_policy_act_cli(
    raw: &str,
    activate_at_height: Option<u64>,
) -> Result<ActivationMode, String> {
    parse_activation_mode(raw, activate_at_height)
}

fn parse_policy_selector(policy: Option<String>, policy_id: Option<u8>) -> Result<u8, String> {
    match (policy, policy_id) {
        (Some(kind), Some(id)) => {
            let parsed_id = parse_policy_kind(&kind)?.policy_id();
            if parsed_id != id {
                return Err(format!(
                    "policy selector mismatch: --policy '{kind}' maps to id={parsed_id}, but --policy-id={id}"
                ));
            }
            Ok(id)
        }
        (Some(kind), None) => Ok(parse_policy_kind(&kind)?.policy_id()),
        (None, Some(id)) => {
            if PolicyKind::from_policy_id(id).is_none() {
                return Err(format!("invalid --policy-id {id}; allowed range is 0..=4"));
            }
            Ok(id)
        }
        (None, None) => Err("set --policy or --policy-id".to_string()),
    }
}

fn parse_initial_policy(raw: &str) -> Result<InitPolicyEntry, String> {
    let (policy_raw, activation_raw) = if let Some((left, right)) = raw.split_once(':') {
        (left, right)
    } else {
        (raw, "dormant")
    };
    Ok(InitPolicyEntry {
        policy: parse_policy_kind(policy_raw)?,
        activation: parse_activation_mode(activation_raw, None)?,
    })
}

fn parse_init_v4_args(args: InitV4Args) -> Result<Option<InitV4Extension>, String> {
    let has_any = args.owner_kind.is_some()
        || args.owner_name.is_some()
        || args.owner_country.is_some()
        || args.metadata_commitment.is_some()
        || args.verification_ref.is_some()
        || args.requested_domain_lo.is_some()
        || args.rescue_address.is_some()
        || !args.initial_policies.is_empty();
    if !has_any {
        return Ok(None);
    }
    let owner_kind = require_nonempty_arg(args.owner_kind, "--owner-kind")?;
    let owner_display_name = require_nonempty_arg(args.owner_name, "--owner-name")?;
    let owner_country_hint = require_nonempty_arg(args.owner_country, "--owner-country")?;
    let metadata_hex = require_nonempty_arg(args.metadata_commitment, "--metadata-commitment")?;
    let company_metadata_commitment =
        hex32(&metadata_hex).map_err(|e| format!("invalid --metadata-commitment: {e}"))?;
    let external_verification_ref = args.verification_ref.unwrap_or_default();
    let rescue_address = args
        .rescue_address
        .as_deref()
        .map(|v| parse_address_arg("--rescue-address", v))
        .transpose()?;
    let mut initial_policies = Vec::new();
    for raw in &args.initial_policies {
        initial_policies.push(parse_initial_policy(raw)?);
    }
    Ok(Some(InitV4Extension {
        owner_kind,
        owner_display_name,
        owner_country_hint,
        company_metadata_commitment,
        external_verification_ref,
        requested_domain_lo: args.requested_domain_lo.unwrap_or(0),
        rescue_address,
        initial_policies,
        cosign_policy: None,
    }))
}

fn require_nonempty_arg(value: Option<String>, flag: &str) -> Result<String, String> {
    let raw = value.ok_or_else(|| format!("{flag} is required when using v4 init extension"))?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(format!("{flag} must not be empty"));
    }
    Ok(trimmed.to_string())
}

fn rescue_has_any(args: &RescueCosignArgs) -> bool {
    args.rescue_account_index.is_some()
        || args.rescue_wallet.is_some()
        || args.rescue_master.is_some()
        || args.rescue_domain.is_some()
}

fn load_rescue_source(
    owner_wallet: Option<PathBuf>,
    wallet_passphrase: Option<&str>,
    upgrade_wallet: bool,
    rescue: RescueCosignArgs,
) -> Result<crate::signer::TxSignerSource, String> {
    if let Some(account_index) = rescue.rescue_account_index {
        let wallet_path = rescue.rescue_wallet.or(owner_wallet).ok_or_else(|| {
            "rescue account selection requires --wallet or --rescue-wallet".to_string()
        })?;
        let passphrase = rescue.rescue_passphrase.as_deref().or(wallet_passphrase);
        return load_wallet_account_signer(&wallet_path, account_index, passphrase, upgrade_wallet);
    }
    if let Some(master_hex) = rescue.rescue_master {
        let domain = rescue
            .rescue_domain
            .ok_or_else(|| "--rescue-domain is required with --rescue-master".to_string())?;
        return load_tx_signer_source(
            None,
            Some(master_hex),
            Some(domain),
            rescue.rescue_passphrase.as_deref(),
            upgrade_wallet,
        );
    }
    if let Some(wallet_path) = rescue.rescue_wallet {
        let passphrase = rescue.rescue_passphrase.as_deref().or(wallet_passphrase);
        return load_tx_signer_source(Some(wallet_path), None, None, passphrase, upgrade_wallet);
    }
    Err(
        "missing rescue signer source: set --rescue-account-index, --rescue-wallet, or --rescue-master/--rescue-domain"
            .to_string(),
    )
}

fn append_rescue_cosign(tx: &mut SignedTx, rescue_sk: &ed25519_dalek::SigningKey) {
    let msg = tx.signing_message();
    tx.cosigns.push(Cosignature {
        signer_pk: rescue_sk.verifying_key().to_bytes(),
        role: CosignRole::Rescue,
        signature: sign(rescue_sk, &msg),
    });
}

fn save_signed_tx(path: &Path, tx: &SignedTx) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
    }
    let body = serde_json::to_string_pretty(tx).map_err(|e| e.to_string())?;
    fs::write(path, body).map_err(|e| e.to_string())
}

fn enrich_act_nonce_err(
    c: &reqwest::blocking::Client,
    rpc_base: &str,
    tx: &SignedTx,
    err: &str,
) -> String {
    if !is_act_nonce_err(err) {
        return err.to_string();
    }
    let Some(acct) = tx_target_acct(tx) else {
        return err.to_string();
    };
    let acct_human = account_id_to_human(&acct);
    let file_nonce = tx.nonce;
    match fetch_nonce(c, rpc_base, acct) {
        Ok(chain_nonce) => format!(
            "{err}\n\
             tx-policy-activate --activation-tx nonce mismatch: file nonce={file_nonce}, on-chain nonce={chain_nonce} for target_account={acct_human}\n\
             hint: rebuild and submit a live activation with --wallet <path> --index <victim_idx> (and --rescue-account-index for same-wallet rescue)"
        ),
        Err(fetch_err) => format!(
            "{err}\n\
             tx-policy-activate --activation-tx rejected with bad nonce (file nonce={file_nonce} for target_account={acct_human}); failed to fetch on-chain nonce: {fetch_err}\n\
             hint: retry live activation with --wallet <path> --index <victim_idx>; if needed refresh account state and rebuild activation tx"
        ),
    }
}

fn tx_target_acct(tx: &SignedTx) -> Option<AccountId> {
    match tx.body {
        TxBody::Policy { target_account, .. } => Some(target_account),
        _ => None,
    }
}

fn is_act_nonce_err(err: &str) -> bool {
    let msg = err.to_ascii_lowercase();
    msg.contains("http 409") && msg.contains("nonce")
}

fn load_signed_tx(path: &Path) -> Result<SignedTx, String> {
    let body = fs::read_to_string(path)
        .map_err(|e| format!("failed to read activation tx '{}': {e}", path.display()))?;
    serde_json::from_str(&body)
        .map_err(|e| format!("failed to decode activation tx '{}': {e}", path.display()))
}

fn build_init_activation(
    source: &crate::signer::TxSignerSource,
    init_v4: Option<&InitV4Extension>,
    wallet_path: Option<&PathBuf>,
    wallet_passphrase: Option<&str>,
    upgrade_wallet: bool,
    init_nonce: u64,
) -> Result<Option<SignedTx>, String> {
    let Some(ext) = init_v4 else {
        return Ok(None);
    };
    let Some(target) = ext.rescue_address else {
        return Ok(None);
    };
    let has_emergency = ext.initial_policies.iter().any(|row| {
        row.policy == PolicyKind::RoutingEmergencyRedirect
            && !matches!(row.activation, ActivationMode::Immediately)
    });
    if !has_emergency {
        return Ok(None);
    }
    let mut tx = SignedTx::sign_body(
        &source.sk,
        source.dom,
        source.idx,
        calc_activation_nonce(init_nonce),
        TxBody::Policy {
            target_account: source.from,
            action: PolicyAction::ActivatePolicy {
                policy_id: PolicyKind::RoutingEmergencyRedirect.policy_id(),
                activation_target: Some(target),
            },
            fee: 0,
        },
    );
    if let Some(wp) = wallet_path {
        maybe_rescue_cosign(&mut tx, wp, &target, wallet_passphrase, upgrade_wallet)?;
    }
    eprintln!(
        "tx-init prepared activation: policy=routing.emergency_redirect target={}",
        account_id_to_human(&target)
    );
    Ok(Some(tx))
}

fn calc_activation_nonce(init_nonce: u64) -> u64 {
    init_nonce.saturating_add(1)
}

fn maybe_rescue_cosign(
    tx: &mut SignedTx,
    wallet_path: &PathBuf,
    target: &AccountId,
    wallet_passphrase: Option<&str>,
    upgrade_wallet: bool,
) -> Result<(), String> {
    let target_hex = hex::encode(target);
    let Ok(accounts) = wallet_account_list(wallet_path) else {
        return Ok(());
    };
    let Some(row) = accounts
        .into_iter()
        .find(|row| row.id_hex.eq_ignore_ascii_case(&target_hex))
    else {
        return Ok(());
    };
    let rescue = load_wallet_account_signer(
        wallet_path,
        row.derivation_index,
        wallet_passphrase,
        upgrade_wallet,
    )?;
    append_rescue_cosign(tx, &rescue.sk);
    Ok(())
}

#[cfg(test)]
#[allow(clippy::items_after_test_module)]
mod tests {
    use super::*;
    use ed25519_dalek::SigningKey;
    use pwm_core::hd::account_id_from_parts;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    fn spawn_mock_http_server(script: Vec<(&'static str, u16, &'static str)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        thread::spawn(move || {
            for (expected_line, status, body) in script {
                let (mut stream, _) = listener.accept().expect("accept");
                let mut buf = [0u8; 8192];
                let n = stream.read(&mut buf).expect("read");
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                assert!(req.starts_with(expected_line), "unexpected request: {req}");
                let reason = match status {
                    200 => "OK",
                    204 => "No Content",
                    409 => "Conflict",
                    _ => "OK",
                };
                let resp = format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                stream.write_all(resp.as_bytes()).expect("write");
            }
        });
        format!("http://{addr}")
    }

    #[test]
    fn prepared_activation_roundtrip() {
        let owner_sk = SigningKey::from_bytes(&[11u8; 32]);
        let owner_pk = owner_sk.verifying_key().to_bytes();
        let owner_id = account_id_from_parts(&owner_pk, 0);
        let source = crate::signer::TxSignerSource {
            sk: owner_sk,
            dom: 0x4359,
            idx: 0,
            from: owner_id,
        };
        let rescue_sk = SigningKey::from_bytes(&[12u8; 32]);
        let rescue_pk = rescue_sk.verifying_key().to_bytes();
        let rescue_id = account_id_from_parts(&rescue_pk, 1);
        let ext = InitV4Extension {
            owner_kind: "person".to_string(),
            owner_display_name: "Alice".to_string(),
            owner_country_hint: "CY".to_string(),
            company_metadata_commitment: [0u8; 32],
            external_verification_ref: "kyc:alice".to_string(),
            requested_domain_lo: 0,
            rescue_address: Some(rescue_id),
            initial_policies: vec![InitPolicyEntry {
                policy: PolicyKind::RoutingEmergencyRedirect,
                activation: ActivationMode::Dormant,
            }],
            cosign_policy: None,
        };
        let tx = build_init_activation(&source, Some(&ext), None, None, false, 0)
            .expect("must build")
            .expect("must prepare activation");
        assert_eq!(tx.nonce, 1);
        match &tx.body {
            TxBody::Policy {
                target_account,
                action:
                    PolicyAction::ActivatePolicy {
                        policy_id,
                        activation_target,
                    },
                fee,
            } => {
                assert_eq!(*target_account, owner_id);
                assert_eq!(*policy_id, PolicyKind::RoutingEmergencyRedirect.policy_id());
                assert_eq!(*activation_target, Some(rescue_id));
                assert_eq!(*fee, 0);
            }
            other => panic!("unexpected prepared tx body: {other:?}"),
        }
        let path = std::env::temp_dir().join(format!(
            "pwm-prepared-activation-{}-{}.json",
            std::process::id(),
            rescue_id[0]
        ));
        save_signed_tx(&path, &tx).expect("save prepared activation");
        let loaded = load_signed_tx(&path).expect("load prepared activation");
        let _ = fs::remove_file(&path);
        assert_eq!(loaded, tx);
    }

    #[test]
    fn tx_init_wallet_idx() {
        let path = std::env::temp_dir().join(format!(
            "pwm-cli-tx-init-wallet-idx-{}.yaml",
            rand::random::<u128>()
        ));
        let seed = [0x39u8; 32];
        let first_idx = 3u32;
        let sel_idx = 21u32;
        let first_key = slip10_ed25519::derive_ed25519_private_key(&seed, &[0, first_idx]);
        let first_sk = SigningKey::from_bytes(&first_key);
        let first_pk = first_sk.verifying_key().to_bytes();
        let first_id = account_id_from_parts(&first_pk, first_idx);
        let wallet = crate::wallet::build_wallet_yaml(
            seed,
            first_sk.to_bytes(),
            first_pk,
            first_idx,
            u16::from_be_bytes([first_id[0], first_id[1]]),
            0x03FF,
            0,
            u32::from_be_bytes([first_id[2], first_id[3], first_id[4], first_id[5]]),
            hex::encode(first_id),
            account_id_to_human(&first_id),
            Some("CY".to_string()),
            crate::wallet::WalletProtection::PlaintextDev,
        )
        .expect("wallet");
        crate::wallet::save_wallet_v3_new(&path, &wallet).expect("save wallet");
        crate::wallet::wallet_account_add_seed(&path, sel_idx, &seed).expect("add wallet account");

        let source = load_tx_wallet_signer(Some(path.clone()), None, None, None, false, sel_idx)
            .expect("load signer");
        assert_eq!(source.idx, sel_idx);
        assert_eq!(
            account_id_from_parts(&source.sk.verifying_key().to_bytes(), source.idx),
            source.from
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn tx_v2_idx_fallback() {
        let path = std::env::temp_dir().join(format!(
            "pwm-cli-tx-v2-def-idx-{}.yaml",
            rand::random::<u128>()
        ));
        let seed = [0x61u8; 32];
        let sel_idx = 19u32;
        let sk_bytes = slip10_ed25519::derive_ed25519_private_key(&seed, &[0, sel_idx]);
        let sk = SigningKey::from_bytes(&sk_bytes);
        let pk = sk.verifying_key().to_bytes();
        let id = account_id_from_parts(&pk, sel_idx);
        let wallet = crate::wallet::build_wallet_yaml(
            seed,
            sk.to_bytes(),
            pk,
            sel_idx,
            u16::from_be_bytes([id[0], id[1]]),
            0x03FF,
            0,
            u32::from_be_bytes([id[2], id[3], id[4], id[5]]),
            hex::encode(id),
            account_id_to_human(&id),
            Some("CY".to_string()),
            crate::wallet::WalletProtection::PlaintextDev,
        )
        .expect("wallet");
        crate::wallet::save_wallet_yaml(&path, &wallet).expect("save v2 wallet");

        let source = load_tx_wallet_signer(Some(path.clone()), None, None, None, false, 0)
            .expect("fallback signer");
        assert_eq!(source.idx, sel_idx);
        assert_eq!(source.from, id);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn tx_init_nonce_add1() {
        assert_eq!(calc_activation_nonce(0), 1);
        assert_eq!(calc_activation_nonce(7), 8);
    }

    #[test]
    fn tx_pol_nonce_detect() {
        assert!(is_act_nonce_err("tx submit: HTTP 409 (url): bad nonce"));
        assert!(!is_act_nonce_err("tx submit: HTTP 400 (url): bad nonce"));
    }

    #[test]
    fn tx_pol_nonce_409() {
        let owner_sk = SigningKey::from_bytes(&[21u8; 32]);
        let owner_id = account_id_from_parts(&owner_sk.verifying_key().to_bytes(), 0);
        let tx = SignedTx::sign_body(
            &owner_sk,
            0x4359,
            0,
            1,
            TxBody::Policy {
                target_account: owner_id,
                action: PolicyAction::ActivatePolicy {
                    policy_id: PolicyKind::RoutingEmergencyRedirect.policy_id(),
                    activation_target: Some(owner_id),
                },
                fee: 0,
            },
        );
        let from_hex = hex::encode(owner_id);
        let get_line = Box::leak(format!("GET /v1/account/{from_hex} HTTP/1.1").into_boxed_str());
        let rpc = spawn_mock_http_server(vec![(get_line, 200, r#"{"nonce":9}"#)]);
        let client = reqwest::blocking::Client::new();
        let rich = enrich_act_nonce_err(
            &client,
            &rpc,
            &tx,
            "tx submit: HTTP 409 (http://127.0.0.1:3030/v1/tx): bad nonce",
        );
        assert!(rich.contains("nonce mismatch"), "{rich}");
        assert!(rich.contains("file nonce=1"), "{rich}");
        assert!(rich.contains("on-chain nonce=9"), "{rich}");
        assert!(rich.contains("--index <victim_idx>"), "{rich}");
        assert!(rich.contains("--rescue-account-index"), "{rich}");
    }
}

pub(crate) fn run_tx_burn_mark(
    rpc_base: &str,
    wallet: Option<PathBuf>,
    master: Option<String>,
    domain: Option<String>,
    wallet_passphrase: Option<&str>,
    upgrade_wallet: bool,
    index: u32,
    mark_amount: u32,
    beneficiary: Option<String>,
    purpose: Option<String>,
) {
    let source = load_tx_wallet_signer(
        wallet,
        master,
        domain,
        wallet_passphrase,
        upgrade_wallet,
        index,
    )
    .unwrap_or_else(|e| exit_user_error(&e));
    let c = http_client_for_rpc();
    let marks_before = fetch_marks(&c, rpc_base, source.from).unwrap_or_else(|e| {
        eprintln!("pwm: warn: could not fetch marks: {e}");
        0
    });
    eprintln!("pwm: current marks: {marks_before}");
    let nonce = fetch_nonce(&c, rpc_base, source.from).unwrap_or_else(|e| exit_user_error(&e));
    let beneficiary = beneficiary
        .as_deref()
        .map(|v| {
            let (parsed, uri_amount) = parse_address_input("--beneficiary", v)?;
            if uri_amount.is_some() {
                return Err(
                    "URI amount is not allowed for --beneficiary in tx-burn-mark".to_string(),
                );
            }
            Ok(parsed)
        })
        .transpose()
        .unwrap_or_else(|e| {
            exit_user_error(&format!(
                "{e}. Hint: verify --rpc/PWM_RPC points to the intended source-shard node for this signer"
            ))
        });
    beneficiary
        .as_ref()
        .map(|b| validate_recipient_domain_policy(b, Some("--beneficiary")))
        .transpose()
        .unwrap_or_else(|e| {
            exit_user_error(&format!(
                "{e}. Hint: verify beneficiary domain policy and --rpc/PWM_RPC target shard"
            ))
        });
    let mut tx = SignedTx::sign_body(
        &source.sk,
        source.dom,
        source.idx,
        nonce,
        TxBody::BurnMark {
            mark_amount,
            beneficiary,
        },
    );
    if let Some(p) = purpose {
        let expanded = expand_purpose(&p);
        tx.set_burn_purpose_signed(&source.sk, expanded);
    } else {
        eprintln!(
            "pwm: note: burn uses a built-in default purpose; pass --purpose for an explicit v2 dedication string (RFC 0011)."
        );
    }
    // Node rejects insufficient marks with error code "E_BURN_OVER_BALANCE" (STATE_CONFLICT),
    // Display message: "insufficient marks" (TxError::InsufficientMarks -> #[error]).
    post_signed_tx(&c, rpc_base, &tx).unwrap_or_else(|e| exit_user_error(&e));
    eprintln!("pwm: burn submitted; marks before: {marks_before}");
}
