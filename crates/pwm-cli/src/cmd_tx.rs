//! Local tx submission paths (`tx-init`, `tx-policy-*`, `tx-send`, `tx-stake`, `tx-unstake`, `tx-burn-mark`).

use crate::cli_parse::{hex32, parse_address_arg, parse_address_input, resolve_tx_send_amount};
use crate::cmd_roaming;
use crate::purpose_expand::expand_purpose;
use crate::rpc_helpers::{fetch_marks, fetch_nonce, post_signed_tx, preflight_recipient_init};
use crate::signer::{load_tx_signer_source, load_wallet_account_signer};
use crate::wallet::assert_tx_recipient_allowed;
use crate::wallet::load_wallet_yaml_upgrade;
use crate::{exit_user_error, http_client_for_rpc};
use pwm_core::crypto::sign;
use pwm_core::tx::{
    ActivationMode, ClaimMode, CosignRole, Cosignature, InitPolicyEntry, InitV4Extension,
    PolicyAction, PolicyKind, SignedTx, TxBody,
};
use pwm_core::validate_recipient_domain_policy;
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
) {
    let source = load_tx_signer_source(wallet, master, domain, wallet_passphrase, upgrade_wallet)
        .unwrap_or_else(|e| exit_user_error(&e));
    let mut tx = SignedTx::sign_body(
        &source.sk,
        source.dom,
        source.idx,
        0,
        TxBody::Init { index, flags },
    );
    let init_v4 = parse_init_v4_args(init_v4).unwrap_or_else(|e| exit_user_error(&e));
    if init_v4.is_some() {
        tx.set_init_v4_signed(&source.sk, init_v4);
    }
    let c = http_client_for_rpc();
    post_signed_tx(&c, rpc_base, &tx).unwrap_or_else(|e| exit_user_error(&e));
}

pub(crate) fn run_tx_policy_set(
    rpc_base: &str,
    wallet: Option<PathBuf>,
    master: Option<String>,
    domain: Option<String>,
    wallet_passphrase: Option<&str>,
    upgrade_wallet: bool,
    policy: String,
    activation: String,
    fee: u128,
) {
    let source = load_tx_signer_source(wallet, master, domain, wallet_passphrase, upgrade_wallet)
        .unwrap_or_else(|e| exit_user_error(&e));
    let policy_kind = parse_policy_kind(&policy).unwrap_or_else(|e| exit_user_error(&e));
    let activation_mode =
        parse_activation_mode(&activation).unwrap_or_else(|e| exit_user_error(&e));
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
    policy: Option<String>,
    policy_id: Option<u8>,
    fee: u128,
    rescue: RescueCosignArgs,
) {
    let source = load_tx_signer_source(
        wallet.clone(),
        master.clone(),
        domain.clone(),
        wallet_passphrase,
        upgrade_wallet,
    )
    .unwrap_or_else(|e| exit_user_error(&e));
    let chosen_policy =
        parse_policy_selector(policy, policy_id).unwrap_or_else(|e| exit_user_error(&e));
    let c = http_client_for_rpc();
    let nonce = fetch_nonce(&c, rpc_base, source.from).unwrap_or_else(|e| exit_user_error(&e));
    let mut tx = SignedTx::sign_body(
        &source.sk,
        source.dom,
        source.idx,
        nonce,
        TxBody::Policy {
            target_account: source.from,
            action: PolicyAction::ActivatePolicy {
                policy_id: chosen_policy,
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
    policy: Option<String>,
    policy_id: Option<u8>,
    fee: u128,
) {
    let source = load_tx_signer_source(wallet, master, domain, wallet_passphrase, upgrade_wallet)
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
    to: String,
    amount: Option<u128>,
    fee: u128,
) {
    let source = load_tx_signer_source(
        wallet.clone(),
        master.clone(),
        domain,
        wallet_passphrase,
        upgrade_wallet,
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
    amount: u128,
) {
    let source = load_tx_signer_source(wallet, master, domain, wallet_passphrase, upgrade_wallet)
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
    amount: u128,
) {
    let source = load_tx_signer_source(wallet, master, domain, wallet_passphrase, upgrade_wallet)
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

fn parse_activation_mode(raw: &str) -> Result<ActivationMode, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "dormant" => Ok(ActivationMode::Dormant),
        "immediately" | "immediate" | "active" => Ok(ActivationMode::Immediately),
        other => Err(format!(
            "invalid activation mode '{other}'; expected `dormant` or `immediately`"
        )),
    }
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
        activation: parse_activation_mode(activation_raw)?,
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

pub(crate) fn parse_claim_mode_cli(raw: &str) -> Result<ClaimMode, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "free" => Ok(ClaimMode::Free),
        "paid" => Ok(ClaimMode::Paid),
        other => Err(format!(
            "invalid --claim-mode {other:?}: expected `free` or `paid`"
        )),
    }
}

pub(crate) fn run_tx_claim(
    rpc_base: &str,
    wallet: Option<PathBuf>,
    master: Option<String>,
    domain: Option<String>,
    wallet_passphrase: Option<&str>,
    upgrade_wallet: bool,
    mode: ClaimMode,
    claim_units: u32,
    anchor_ref: u64,
    fee: u128,
) {
    let source = load_tx_signer_source(wallet, master, domain, wallet_passphrase, upgrade_wallet)
        .unwrap_or_else(|e| exit_user_error(&e));
    let c = http_client_for_rpc();
    let nonce = fetch_nonce(&c, rpc_base, source.from).unwrap_or_else(|e| exit_user_error(&e));
    let tx = SignedTx::sign_body(
        &source.sk,
        source.dom,
        source.idx,
        nonce,
        TxBody::Claim {
            mode,
            claim_units,
            anchor_ref,
            fee,
        },
    );
    post_signed_tx(&c, rpc_base, &tx).unwrap_or_else(|e| exit_user_error(&e));
}

pub(crate) fn run_tx_burn_mark(
    rpc_base: &str,
    wallet: Option<PathBuf>,
    master: Option<String>,
    domain: Option<String>,
    wallet_passphrase: Option<&str>,
    upgrade_wallet: bool,
    mark_amount: u32,
    beneficiary: Option<String>,
    purpose: Option<String>,
) {
    let source = load_tx_signer_source(wallet, master, domain, wallet_passphrase, upgrade_wallet)
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
