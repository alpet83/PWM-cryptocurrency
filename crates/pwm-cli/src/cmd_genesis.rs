//! Genesis build (`genesis-build`) helpers and JSON envelope.

use crate::rpc_helpers::resolve_genesis_passphrase;
use crate::wallet::{load_wallet_yaml_upgrade, wallet_account_list, wallet_secrets};
use ed25519_dalek::SigningKey;
use pwm_core::genesis::{
    GRow, GenCfg, DEF_MARKS_STAKE_MIN, DEF_PWM_STAKE_MIN, DEF_SEASON_COEFF_PPM, LEGACY_POLICY_VER,
};
use pwm_core::hd::account_id_from_parts;
use pwm_core::{parse_acct_id_ui, seal_wallet_secret_plaintext, WALLET_KDF};
use serde::Serialize;
use std::path::{Path, PathBuf};

use super::exit_user_error;
use crate::cli_parse::master_seed;

pub(crate) const GENESIS_SCHEMA_VERSION: u32 = 5;
pub(crate) const GENESIS_VALIDATOR_DER_PATH: &str = "m/1000000'/1'";
const GENESIS_DER_PATH_IDX: [u32; 2] = [1_000_000, 1];
const GENESIS_AEAD_NAME: &str = "chacha20poly1305";

#[derive(Serialize)]
pub(crate) struct GenesisAccountOut {
    pub(crate) acct_hex: String,
    pub(crate) pubkey_hex: String,
    pub(crate) der_idx: u32,
    pub(crate) bal: String,
}

#[derive(Serialize)]
pub(crate) struct GenesisFundingOut {
    pub(crate) accounts: Vec<GenesisAccountOut>,
}

#[derive(Serialize)]
pub(crate) struct GenesisValRowOut {
    pub(crate) acct_hex: String,
    pub(crate) pubkey_hex: String,
    pub(crate) der_idx: u32,
}

#[derive(Serialize)]
pub(crate) struct GenesisValsOut {
    pub(crate) set: Vec<GenesisValRowOut>,
}

#[derive(Serialize)]
pub(crate) struct GenesisRewOut {
    pub(crate) mode: String,
}

#[derive(Serialize)]
pub(crate) struct GenesisV4CfgOut {
    pub(crate) funding: GenesisFundingOut,
    pub(crate) validators: GenesisValsOut,
    pub(crate) reward_policy: GenesisRewOut,
    pub(crate) block_reward: String,
    pub(crate) marks_coeff: String,
    pub(crate) policy_ver: u32,
    pub(crate) pwm_stake_min: String,
    pub(crate) marks_stake_min: String,
    pub(crate) season_enabled: bool,
    pub(crate) season_coeff_ppm: String,
}

#[derive(Serialize)]
pub(crate) struct GenesisKdfOut {
    pub(crate) name: String,
    pub(crate) iters: u32,
    pub(crate) salt_b64: String,
}

#[derive(Serialize)]
pub(crate) struct GenesisAeadOut {
    pub(crate) name: String,
    pub(crate) nonce_b64: String,
    pub(crate) ciphertext_b64: String,
}

#[derive(Serialize)]
pub(crate) struct GenesisEncSeedOut {
    pub(crate) kdf: GenesisKdfOut,
    pub(crate) aead: GenesisAeadOut,
}

#[derive(Serialize)]
pub(crate) struct GenesisValidatorKeyOut {
    pub(crate) derivation_path: String,
    pub(crate) enc_seed: GenesisEncSeedOut,
}

#[derive(Serialize)]
pub(crate) struct GenesisV4Out {
    pub(crate) schema_version: u32,
    pub(crate) gen_cfg: GenesisV4CfgOut,
    pub(crate) validator_keys: Vec<GenesisValidatorKeyOut>,
}

fn pick_val_idx(
    accounts: &[crate::wallet::WalletAccountEntry],
    val_id: Option<&str>,
) -> Result<usize, String> {
    if let Some(id) = val_id {
        let want =
            parse_acct_id_ui(id).map_err(|e| format!("validator account id parse failed: {e}"))?;
        let want_hex = hex::encode(want);
        return accounts
            .iter()
            .position(|a| a.id_hex.eq_ignore_ascii_case(&want_hex))
            .ok_or_else(|| format!("validator account id not found in wallet: {want_hex}"));
    }
    accounts
        .iter()
        .position(|a| a.is_active)
        .ok_or_else(|| "wallet default account not found".to_string())
}

pub(crate) fn build_genesis_v4_wallet(
    wallet_path: &Path,
    wallet_passphrase: Option<&str>,
    genesis_passphrase: &str,
    upgrade_wallet: bool,
    premine_bal: u128,
    block_reward: u128,
    marks_coeff: u128,
    val_id: Option<&str>,
) -> Result<(GenesisV4Out, GenCfg), String> {
    let wallet = load_wallet_yaml_upgrade(wallet_path, upgrade_wallet)
        .map_err(|e| format!("load wallet failed: {e}"))?;
    let secrets = wallet_secrets(&wallet, wallet_passphrase)
        .map_err(|e| format!("wallet decrypt failed: {e}"))?;
    let seed = master_seed(&secrets.master_seed_hex).map_err(|e| format!("master seed: {e}"))?;
    let accounts =
        wallet_account_list(wallet_path).map_err(|e| format!("wallet account list failed: {e}"))?;
    if accounts.is_empty() {
        return Err("wallet has no accounts for genesis accounts".to_string());
    }
    let val_idx = pick_val_idx(&accounts, val_id)?;
    let mut funding_accounts = Vec::with_capacity(accounts.len());
    let mut funding_cfg_accounts = Vec::with_capacity(accounts.len());
    for account in &accounts {
        let signer_seed =
            slip10_ed25519::derive_ed25519_private_key(&seed, &[0, account.derivation_index]);
        let sk = SigningKey::from_bytes(&signer_seed);
        let pubkey = sk.verifying_key().to_bytes();
        let acct = account_id_from_parts(&pubkey, account.derivation_index);
        funding_accounts.push(GenesisAccountOut {
            acct_hex: hex::encode(acct),
            pubkey_hex: hex::encode(pubkey),
            der_idx: account.derivation_index,
            bal: premine_bal.to_string(),
        });
        funding_cfg_accounts.push(GRow {
            acct,
            pubkey,
            der_idx: account.derivation_index,
            bal: premine_bal,
        });
    }
    let val_account = &accounts[val_idx];
    let mut validator_keys = Vec::with_capacity(1);
    let mut val_set = Vec::with_capacity(1);
    let mut val_cfg = Vec::with_capacity(1);
    {
        let account = val_account;
        let validator_seed =
            slip10_ed25519::derive_ed25519_private_key(&seed, &[0, account.derivation_index]);
        let sk_bytes =
            slip10_ed25519::derive_ed25519_private_key(&validator_seed, &GENESIS_DER_PATH_IDX);
        let sk = SigningKey::from_bytes(&sk_bytes);
        let pubkey = sk.verifying_key().to_bytes();
        let acct = account_id_from_parts(&pubkey, GENESIS_DER_PATH_IDX[1]);
        let sealed = seal_wallet_secret_plaintext(&validator_seed, genesis_passphrase)
            .map_err(|e| format!("encrypt validator seed failed: {e}"))?;
        if sealed.kdf != WALLET_KDF {
            return Err(format!("unexpected wallet kdf output: {}", sealed.kdf));
        }
        validator_keys.push(GenesisValidatorKeyOut {
            derivation_path: GENESIS_VALIDATOR_DER_PATH.to_string(),
            enc_seed: GenesisEncSeedOut {
                kdf: GenesisKdfOut {
                    name: sealed.kdf,
                    iters: sealed.kdf_iters,
                    salt_b64: sealed.kdf_salt_b64,
                },
                aead: GenesisAeadOut {
                    name: GENESIS_AEAD_NAME.to_string(),
                    nonce_b64: sealed.aead_nonce_b64,
                    ciphertext_b64: sealed.encrypted_payload_b64,
                },
            },
        });
        val_set.push(GenesisValRowOut {
            acct_hex: hex::encode(acct),
            pubkey_hex: hex::encode(pubkey),
            der_idx: GENESIS_DER_PATH_IDX[1],
        });
        val_cfg.push(pwm_core::VRow {
            acct,
            pubkey,
            der_idx: GENESIS_DER_PATH_IDX[1],
        });
    }
    for val in &val_set {
        let exists = funding_accounts
            .iter()
            .any(|r| r.acct_hex.eq_ignore_ascii_case(&val.acct_hex));
        if exists {
            continue;
        }
        funding_accounts.push(GenesisAccountOut {
            acct_hex: val.acct_hex.clone(),
            pubkey_hex: val.pubkey_hex.clone(),
            der_idx: val.der_idx,
            bal: "0".to_string(),
        });
    }
    for val in &val_cfg {
        let exists = funding_cfg_accounts.iter().any(|r| r.acct == val.acct);
        if exists {
            continue;
        }
        funding_cfg_accounts.push(GRow {
            acct: val.acct,
            pubkey: val.pubkey,
            der_idx: val.der_idx,
            bal: 0,
        });
    }
    Ok((
        GenesisV4Out {
            schema_version: GENESIS_SCHEMA_VERSION,
            gen_cfg: GenesisV4CfgOut {
                funding: GenesisFundingOut {
                    accounts: funding_accounts,
                },
                validators: GenesisValsOut { set: val_set },
                reward_policy: GenesisRewOut {
                    mode: "to_producer_account".to_string(),
                },
                block_reward: block_reward.to_string(),
                marks_coeff: marks_coeff.to_string(),
                policy_ver: LEGACY_POLICY_VER,
                pwm_stake_min: DEF_PWM_STAKE_MIN.to_string(),
                marks_stake_min: DEF_MARKS_STAKE_MIN.to_string(),
                season_enabled: false,
                season_coeff_ppm: DEF_SEASON_COEFF_PPM.to_string(),
            },
            validator_keys,
        },
        GenCfg {
            funding: pwm_core::FundingCfg {
                accounts: funding_cfg_accounts.clone(),
            },
            vals: pwm_core::ValCfg { set: val_cfg },
            rew: pwm_core::RewPol::ToProducerAccount,
            accounts: funding_cfg_accounts,
            blocks_per_hour: pwm_core::genesis::DEF_BLOCKS_PER_HOUR,
            marks_per_hour: pwm_core::genesis::DEF_MARKS_HOUR,
            ipv4_claim_phases: Vec::new(),
            block_reward,
            marks_coeff,
            policy_ver: LEGACY_POLICY_VER,
            base_emission_per_block: pwm_core::genesis::DEF_BASE_EMIT,
            min_validator_stake: DEF_PWM_STAKE_MIN,
            epoch_length_blocks: pwm_core::genesis::DEF_EPOCH_LEN_BLOCKS,
            conservation_delay_blocks: pwm_core::genesis::DEF_CONSERV_DELAY_BLOCKS,
            xshard_lock_to_blocks: pwm_core::genesis::DEF_XSHARD_LOCK_TO,
            pwm_stake_min: DEF_PWM_STAKE_MIN,
            marks_stake_min: DEF_MARKS_STAKE_MIN,
            season_enabled: false,
            season_coeff_ppm: DEF_SEASON_COEFF_PPM,
        },
    ))
}

pub(crate) fn run_genesis_build(
    genesis_passphrase_cli: Option<String>,
    wallet_passphrase: Option<String>,
    upgrade_wallet: bool,
    wallet: PathBuf,
    out: PathBuf,
    val_id: Option<String>,
    premine_bal: u128,
    block_reward: u128,
    marks_coeff: u128,
) {
    let genesis_passphrase = resolve_genesis_passphrase(genesis_passphrase_cli.as_deref())
        .unwrap_or_else(|e| exit_user_error(&e));
    let (bundle, cfg) = build_genesis_v4_wallet(
        &wallet,
        wallet_passphrase.as_deref(),
        genesis_passphrase.as_str(),
        upgrade_wallet,
        premine_bal,
        block_reward,
        marks_coeff,
        val_id.as_deref(),
    )
    .unwrap_or_else(|e| exit_user_error(&e));
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|e| exit_user_error(&format!("failed to create output dir: {e}")));
    }
    let raw = serde_json::to_string_pretty(&bundle)
        .unwrap_or_else(|e| exit_user_error(&format!("serialize genesis failed: {e}")));
    std::fs::write(&out, raw)
        .unwrap_or_else(|e| exit_user_error(&format!("write genesis failed: {e}")));
    println!("genesis_path {}", out.display());
    println!("genesis_accounts {}", cfg.funding.accounts.len());
    println!("genesis_schema {}", GENESIS_SCHEMA_VERSION);
}
