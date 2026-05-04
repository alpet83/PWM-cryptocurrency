//! Wallet CLI shell helpers (protection mode, derivation, display).

use crate::bruteforce::{flags_from_account_id, BruteforceMatch};
use crate::wallet::{WalletProtection, WalletSecrets, WalletYaml};
use ed25519_dalek::SigningKey;
use pwm_core::domain_index::DomainEntry;
use pwm_core::hd::{account_id_from_parts, domain_of_account_id};
use pwm_core::{account_id_to_human, parse_account_id, validate_recipient_address_policy};
use std::path::PathBuf;

pub(crate) fn parse_domain_label_only(s: &str) -> Result<&'static DomainEntry, String> {
    let input = s.trim();
    if input.is_empty() {
        return Err("domain label is required".into());
    }
    if input.starts_with("0x")
        || input.starts_with("0X")
        || input.chars().all(|c| c.is_ascii_digit())
    {
        return Err(format!(
            "numeric domain input is not allowed for addr-bruteforce: '{input}'. Use a domain label from pwm_core::domain_index (e.g. CY, FIN)"
        ));
    }
    pwm_core::domain_index::lookup_by_label(input).ok_or_else(|| {
        format!(
            "unknown domain label '{input}'. Use a label from pwm_core::domain_index (e.g. CY, FIN)"
        )
    })
}

pub(crate) fn validate_user_profile_flags(
    flags_mask: u32,
    expected_flags: u32,
) -> Result<(), String> {
    const USER_FLAGS_MASK: u32 = 0x03FF;
    if (flags_mask & !USER_FLAGS_MASK) != 0 {
        return Err(format!(
            "Phase1 user profile allows only low 10 bits in --flags-mask (max 0x03FF), got 0x{flags_mask:08X}"
        ));
    }
    if (expected_flags & !USER_FLAGS_MASK) != 0 {
        return Err(format!(
            "Phase1 user profile allows only low 10 bits in --expected-flags (max 0x03FF), got 0x{expected_flags:08X}"
        ));
    }
    if (expected_flags & !flags_mask) != 0 {
        return Err("expected_flags must not set bits outside flags_mask".to_string());
    }
    Ok(())
}

fn parse_derivation_path_index(path: &str) -> Result<u32, String> {
    let trimmed = path.trim();
    let parts: Vec<&str> = trimmed.split('/').collect();
    if parts.len() != 3 || parts[0] != "m" || parts[1] != "0" {
        return Err(format!(
            "invalid --derivation-path '{trimmed}': expected canonical form 'm/0/<index>'"
        ));
    }
    let idx = parts[2].parse::<u32>().map_err(|_| {
        format!("invalid --derivation-path '{trimmed}': index must be a non-negative u32")
    })?;
    Ok(idx)
}

pub(crate) fn resolve_explicit_derivation_index(
    derivation_index: Option<u32>,
    derivation_path: Option<&str>,
) -> Result<Option<u32>, String> {
    let path_idx = derivation_path
        .map(parse_derivation_path_index)
        .transpose()?;
    match (derivation_index, path_idx) {
        (Some(idx), Some(path_idx)) if idx != path_idx => Err(format!(
            "conflicting derivation selectors: --derivation-index={idx} but --derivation-path resolves to index {path_idx}"
        )),
        (Some(idx), _) => Ok(Some(idx)),
        (None, Some(path_idx)) => Ok(Some(path_idx)),
        (None, None) => Ok(None),
    }
}

pub(crate) fn derive_user_profile_hit(
    master_seed: &[u8; 32],
    derivation_index: u32,
) -> BruteforceMatch {
    let sk_bytes = slip10_ed25519::derive_ed25519_private_key(master_seed, &[0, derivation_index]);
    let sk = SigningKey::from_bytes(&sk_bytes);
    let pk = sk.verifying_key().to_bytes();
    let account_id = account_id_from_parts(&pk, derivation_index);
    let domain = domain_of_account_id(&account_id);
    let derived_flags = flags_from_account_id(&account_id);
    BruteforceMatch {
        signing_key: sk.to_bytes(),
        verifying_key: pk,
        derivation_index,
        account_id,
        domain,
        derived_flags,
    }
}

/// Explicit `m/0/N` derivation: enforce recipient domain policy only (no `--country` high-byte match).
pub(crate) fn validate_explicit_derivation_account(hit: &BruteforceMatch) -> Result<(), String> {
    validate_recipient_address_policy(&hit.account_id)
}

pub(crate) fn wallet_regulatory_label_for_hit(hit: &BruteforceMatch) -> Option<String> {
    pwm_core::domain_index::lookup_for_display(hit.domain as u32).map(|e| e.label.to_string())
}

pub(crate) fn resolve_wallet_protection(
    wallet_passphrase: Option<&str>,
    plaintext_dev: bool,
) -> Result<WalletProtection, String> {
    if plaintext_dev {
        return Ok(WalletProtection::PlaintextDev);
    }
    let passphrase = wallet_passphrase.ok_or_else(|| {
        "encrypted wallet mode is default. Provide --wallet-passphrase (or PWM_WALLET_PASSPHRASE), or use --plaintext-dev only for explicit local dev mode.".to_string()
    })?;
    if passphrase.trim().is_empty() {
        return Err("wallet passphrase must not be empty".to_string());
    }
    Ok(WalletProtection::Encrypted {
        passphrase: passphrase.to_string(),
    })
}

pub(crate) fn resolve_bruteforce_wallet_protection(
    wallet_passphrase: Option<&str>,
) -> Result<(WalletProtection, bool), String> {
    match wallet_passphrase {
        Some(passphrase) => {
            if passphrase.trim().is_empty() {
                return Err("wallet passphrase must not be empty".to_string());
            }
            Ok((
                WalletProtection::Encrypted {
                    passphrase: passphrase.to_string(),
                },
                false,
            ))
        }
        None => Ok((WalletProtection::PlaintextDev, true)),
    }
}

pub(crate) fn wallet_show_lines(
    doc: &WalletYaml,
    wallet_path: &PathBuf,
    secrets: Option<&WalletSecrets>,
) -> Vec<String> {
    let mut lines = vec![
        format!("wallet_path {}", wallet_path.display()),
        format!("schema_version {}", doc.schema_version),
        format!("wallet_mode {}", doc.mode),
        format!("created_at_unix_sec {}", doc.created_at_unix_sec),
        format!(
            "country_label {}",
            doc.country_code_label.as_deref().unwrap_or("-")
        ),
        format!("derivation_index {}", doc.derivation_index),
        format!(
            "derivation_path {}",
            doc.derivation_path.as_deref().unwrap_or("-")
        ),
        format!("domain_u16 {}", doc.domain_u16),
        format!("flags_mask_u32 {}", doc.flags_mask_u32),
        format!("expected_flags_u32 {}", doc.expected_flags_u32),
        format!("flags_derived_u32 {}", doc.flags_derived_u32),
        format!("account_id_hex {}", doc.account_id_hex),
        format!("id_pretty {}", doc.account_id_human),
    ];
    if doc.address_book.is_empty() {
        lines.push("address_book (empty — tx-send allows any policy-valid recipient)".into());
    } else {
        lines.push(format!("address_book_count {}", doc.address_book.len()));
        for (i, e) in doc.address_book.iter().enumerate() {
            let id = parse_account_id(e.address_str())
                .unwrap_or_else(|_| panic!("normalized address_book contains invalid address"));
            let addr = account_id_to_human(&id);
            if let Some(l) = e.label() {
                lines.push(format!("address_book[{i}] {addr}  label={l}"));
            } else {
                lines.push(format!("address_book[{i}] {addr}"));
            }
        }
    }
    if doc.ignored_legacy_pretty_entries > 0 {
        lines.push(format!(
            "address_book_ignored_legacy_pretty {}",
            doc.ignored_legacy_pretty_entries
        ));
    }
    if let Some(secrets) = secrets {
        lines.push(format!("master_seed_hex {}", secrets.master_seed_hex));
        lines.push(format!("signing_key_hex {}", secrets.signing_key_hex));
        lines.push(format!("verifying_key_hex {}", secrets.verifying_key_hex));
    }
    lines
}
