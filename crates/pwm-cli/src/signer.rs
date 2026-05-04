//! Transaction signing sources (wallet unlock or master+domain override).

use crate::cli_parse::{hex32, master_seed, parse_domain};
use crate::wallet::{load_wallet_yaml_upgrade, wallet_secrets};
use ed25519_dalek::SigningKey;
use pwm_core::hd::{account_id_from_parts, brute_cluster_address};
use pwm_core::{parse_account_id, AccountId};
use std::path::PathBuf;

pub(crate) struct TxSignerSource {
    pub(crate) sk: SigningKey,
    pub(crate) dom: u16,
    pub(crate) idx: u32,
    pub(crate) from: AccountId,
}

fn derive_sender(master: &str, domain: &str) -> Result<(SigningKey, u16, u32, AccountId), String> {
    let seed = master_seed(master).map_err(|e| format!("invalid --master: {e}"))?;
    let dom = parse_domain(domain).map_err(|e| format!("invalid --domain: {e}"))?;
    let (sk, _pk, i, from) = brute_cluster_address(&seed, dom, 500_000)
        .ok_or_else(|| "no sender match found in derivation window".to_string())?;
    Ok((sk, dom, i, from))
}

fn load_sender_from_wallet(
    path: &PathBuf,
    wallet_passphrase: Option<&str>,
    upgrade_wallet: bool,
) -> Result<TxSignerSource, String> {
    let wallet = load_wallet_yaml_upgrade(path, upgrade_wallet)
        .map_err(|e| format!("failed to read wallet '{}': {e}", path.display()))?;
    let secrets = wallet_secrets(&wallet, wallet_passphrase)
        .map_err(|e| format!("failed to unlock wallet '{}': {e}", path.display()))?;
    let seed = hex32(&secrets.master_seed_hex).map_err(|e| {
        format!(
            "invalid master_seed_hex in wallet '{}': {e}",
            path.display()
        )
    })?;
    let from = parse_account_id(&wallet.account_id_hex).map_err(|e| {
        format!(
            "invalid account_id_hex in wallet '{}': {}",
            path.display(),
            e
        )
    })?;
    let key = slip10_ed25519::derive_ed25519_private_key(&seed, &[0, wallet.derivation_index]);
    let sk = SigningKey::from_bytes(&key);
    let derived = account_id_from_parts(&sk.verifying_key().to_bytes(), wallet.derivation_index);
    if derived != from {
        return Err(format!(
            "wallet signing material mismatch for m/0/{}: derivation metadata does not match selected account",
            wallet.derivation_index
        ));
    }
    Ok(TxSignerSource {
        sk,
        dom: wallet.domain_u16,
        idx: wallet.derivation_index,
        from,
    })
}

pub(crate) fn load_tx_signer_source(
    wallet: Option<PathBuf>,
    master: Option<String>,
    domain: Option<String>,
    wallet_passphrase: Option<&str>,
    upgrade_wallet: bool,
) -> Result<TxSignerSource, String> {
    if let Some(master_hex) = master {
        let domain_str = domain
            .ok_or_else(|| "--domain is required when --master override is set".to_string())?;
        let (sk, dom, idx, from) = derive_sender(&master_hex, &domain_str)?;
        return Ok(TxSignerSource { sk, dom, idx, from });
    }
    let wallet_path =
        wallet.ok_or_else(|| "either --wallet or --master must be provided".to_string())?;
    load_sender_from_wallet(&wallet_path, wallet_passphrase, upgrade_wallet)
}
