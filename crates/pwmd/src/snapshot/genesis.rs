//! Genesis JSON ingestion (`schema_version` 4/5) into snapshot genesis rows.

use super::types::SnapshotGenesisRow;
use ed25519_dalek::SigningKey;
use pwm_core::genesis::{
    ClaimPhaseConfig, FundingCfg, GRow, GenCfg, RewPol, VRow, ValCfg, DEF_CONSERV_DELAY_BLOCKS,
    DEF_MARKS_STAKE_MIN, DEF_PWM_STAKE_MIN, DEF_SEASON_COEFF_PPM, DEF_XSHARD_LOCK_TO,
    LEGACY_POLICY_VER,
};
use pwm_core::hd::account_id_from_parts;
use pwm_core::{open_wallet_secret_ciphertext, WALLET_KDF};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeSet;

#[derive(Deserialize)]
struct GenesisFileV4 {
    schema_version: u32,
    gen_cfg: GenesisCfgV4,
    validator_keys: Vec<GenesisValidatorKeyV3>,
}

#[derive(Deserialize)]
struct GenesisCfgV4 {
    funding: GenesisFundingV4,
    validators: GenesisValsV4,
    #[serde(default)]
    reward_policy: GenesisRewardV4,
    block_reward: String,
    marks_coeff: String,
    #[serde(default = "default_pol_ver")]
    policy_ver: u32,
    #[serde(default = "def_pwm_min_s")]
    pwm_stake_min: String,
    #[serde(default = "def_min_val_stake_s")]
    min_validator_stake: String,
    #[serde(default = "def_marks_min_s")]
    marks_stake_min: String,
    #[serde(default)]
    season_enabled: bool,
    #[serde(default = "def_season_ppm_s")]
    season_coeff_ppm: String,
    #[serde(default)]
    ipv4_claim_phases: Vec<GenesisClaimPhaseV4>,
    #[serde(
        default = "def_xshard_lock_to",
        rename = "cross_shard_lock_timeout_blocks"
    )]
    xshard_lock_to_blocks: u64,
    #[serde(default = "def_conserv_delay_blocks")]
    conservation_delay_blocks: u64,
}

#[derive(Deserialize)]
struct GenesisFundingV4 {
    accounts: Vec<GenesisRowV3>,
}

#[derive(Deserialize)]
struct GenesisValsV4 {
    set: Vec<GenesisValRowV4>,
}

#[derive(Deserialize)]
struct GenesisValRowV4 {
    acct_hex: String,
    pubkey_hex: String,
    der_idx: u32,
}

#[derive(Clone, Default, Deserialize)]
struct GenesisRewardV4 {
    #[serde(default)]
    mode: GenesisRewardModeV4,
}

#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum GenesisRewardModeV4 {
    #[default]
    ToProducerAccount,
}

#[derive(Deserialize)]
struct GenesisRowV3 {
    acct_hex: String,
    pubkey_hex: String,
    der_idx: u32,
    bal: String,
}

#[derive(Deserialize)]
struct GenesisClaimPhaseV4 {
    phase: u8,
    registry_address: String,
    allocation: Value,
}

#[derive(Clone, Deserialize)]
struct GenesisValidatorKeyV3 {
    derivation_path: String,
    enc_seed: GenesisEncSeedV3,
}

#[derive(Clone, Deserialize)]
struct GenesisEncSeedV3 {
    kdf: GenesisKdfV3,
    aead: GenesisAeadV3,
}

#[derive(Clone, Deserialize)]
struct GenesisKdfV3 {
    name: String,
    iters: u32,
    salt_b64: String,
}

#[derive(Clone, Deserialize)]
struct GenesisAeadV3 {
    name: String,
    nonce_b64: String,
    ciphertext_b64: String,
}
fn hex32_from_hex(s: &str) -> Result<[u8; 32], String> {
    let v = hex::decode(s.trim()).map_err(|e| format!("hex: {e}"))?;
    if v.len() != 32 {
        return Err("need 32-byte hex".into());
    }
    let mut a = [0u8; 32];
    a.copy_from_slice(&v);
    Ok(a)
}

fn parse_u128_json(v: &str, field: &str) -> Result<u128, String> {
    v.trim()
        .parse::<u128>()
        .map_err(|e| format!("{field}: invalid u128 string: {e}"))
}

fn parse_u128_value(v: &Value, field: &str) -> Result<u128, String> {
    if let Some(s) = v.as_str() {
        return parse_u128_json(s, field);
    }
    if let Some(n) = v.as_u64() {
        return Ok(u128::from(n));
    }
    Err(format!("{field}: invalid u128 JSON value"))
}

fn parse_u64_json(v: &str, field: &str) -> Result<u64, String> {
    v.trim()
        .parse::<u64>()
        .map_err(|e| format!("{field}: invalid u64 string: {e}"))
}

fn default_pol_ver() -> u32 {
    LEGACY_POLICY_VER
}

fn def_pwm_min_s() -> String {
    DEF_PWM_STAKE_MIN.to_string()
}

fn def_min_val_stake_s() -> String {
    DEF_PWM_STAKE_MIN.to_string()
}

fn def_marks_min_s() -> String {
    DEF_MARKS_STAKE_MIN.to_string()
}

fn def_season_ppm_s() -> String {
    DEF_SEASON_COEFF_PPM.to_string()
}

fn def_xshard_lock_to() -> u64 {
    DEF_XSHARD_LOCK_TO
}

fn def_conserv_delay_blocks() -> u64 {
    DEF_CONSERV_DELAY_BLOCKS
}

const GENESIS_SCHEMA_VERSION: u32 = 5;
const GENESIS_SCHEMA_BACKWARD: u32 = 4;
const VALIDATOR_DERIVATION_PATH: &str = "m/1000000'/1'";
const VALIDATOR_DERIVATION_PATH_IDX: [u32; 2] = [1_000_000, 1];
const GENESIS_AEAD_NAME: &str = "chacha20poly1305";
const GENESIS_KDF_ITERS_MAX: u32 = 10_000_000;

fn parse_genesis_v4(raw: Value) -> Result<(GenCfg, Vec<GenesisValidatorKeyV3>), String> {
    let b: GenesisFileV4 = serde_json::from_value(raw)
        .map_err(|e| format!("parse genesis v4 JSON: invalid v4 payload: {e}"))?;
    if b.schema_version != GENESIS_SCHEMA_VERSION && b.schema_version != GENESIS_SCHEMA_BACKWARD {
        return Err(format!(
            "parse genesis JSON: unsupported schema_version {}; supported: 4, 5",
            b.schema_version
        ));
    }
    let mut rows = Vec::with_capacity(b.gen_cfg.funding.accounts.len());
    for (i, row) in b.gen_cfg.funding.accounts.into_iter().enumerate() {
        let acct = hex32_from_hex(&row.acct_hex)
            .map_err(|e| format!("gen_cfg.funding.accounts[{i}].acct_hex: {e}"))?;
        let pubkey = hex32_from_hex(&row.pubkey_hex)
            .map_err(|e| format!("gen_cfg.funding.accounts[{i}].pubkey_hex: {e}"))?;
        let bal = parse_u128_json(&row.bal, &format!("gen_cfg.funding.accounts[{i}].bal"))?;
        rows.push(GRow {
            acct,
            pubkey,
            der_idx: row.der_idx,
            bal,
        });
    }
    let mut set = Vec::with_capacity(b.gen_cfg.validators.set.len());
    for (i, row) in b.gen_cfg.validators.set.into_iter().enumerate() {
        let acct = hex32_from_hex(&row.acct_hex)
            .map_err(|e| format!("gen_cfg.validators.set[{i}].acct_hex: {e}"))?;
        let pubkey = hex32_from_hex(&row.pubkey_hex)
            .map_err(|e| format!("gen_cfg.validators.set[{i}].pubkey_hex: {e}"))?;
        set.push(VRow {
            acct,
            pubkey,
            der_idx: row.der_idx,
        });
    }
    let block_reward = parse_u128_json(&b.gen_cfg.block_reward, "gen_cfg.block_reward")?;
    let marks_coeff = parse_u128_json(&b.gen_cfg.marks_coeff, "gen_cfg.marks_coeff")?;
    let pwm_stake_min = parse_u128_json(&b.gen_cfg.pwm_stake_min, "gen_cfg.pwm_stake_min")?;
    let min_validator_stake = parse_u128_json(
        &b.gen_cfg.min_validator_stake,
        "gen_cfg.min_validator_stake",
    )?;
    let marks_stake_min = parse_u128_json(&b.gen_cfg.marks_stake_min, "gen_cfg.marks_stake_min")?;
    let season_coeff_ppm = parse_u64_json(&b.gen_cfg.season_coeff_ppm, "gen_cfg.season_coeff_ppm")?;
    let ipv4_claim_phases = parse_claim_phases(b.gen_cfg.ipv4_claim_phases)?;
    let rew = match b.gen_cfg.reward_policy.mode {
        GenesisRewardModeV4::ToProducerAccount => RewPol::ToProducerAccount,
    };
    Ok((
        GenCfg {
            funding: FundingCfg {
                accounts: rows.clone(),
            },
            vals: ValCfg { set },
            rew,
            accounts: rows,
            blocks_per_hour: pwm_core::genesis::DEF_BLOCKS_PER_HOUR,
            marks_per_hour: pwm_core::genesis::DEF_MARKS_HOUR,
            ipv4_claim_phases,
            block_reward,
            marks_coeff,
            policy_ver: b.gen_cfg.policy_ver,
            base_emission_per_block: pwm_core::genesis::DEF_BASE_EMIT,
            min_validator_stake,
            epoch_length_blocks: pwm_core::genesis::DEF_EPOCH_LEN_BLOCKS,
            conservation_delay_blocks: b.gen_cfg.conservation_delay_blocks,
            xshard_lock_to_blocks: b.gen_cfg.xshard_lock_to_blocks,
            pwm_stake_min,
            marks_stake_min,
            season_enabled: b.gen_cfg.season_enabled,
            season_coeff_ppm,
        },
        b.validator_keys,
    ))
}

fn parse_claim_phases(rows: Vec<GenesisClaimPhaseV4>) -> Result<Vec<ClaimPhaseConfig>, String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::with_capacity(rows.len());
    for (i, row) in rows.into_iter().enumerate() {
        if !seen.insert(row.phase) {
            return Err(format!(
                "gen_cfg.ipv4_claim_phases[{i}].phase: duplicate phase {}",
                row.phase
            ));
        }
        let registry_address = hex32_from_hex(&row.registry_address)
            .map_err(|e| format!("gen_cfg.ipv4_claim_phases[{i}].registry_address: {e}"))?;
        let allocation = parse_u128_value(
            &row.allocation,
            &format!("gen_cfg.ipv4_claim_phases[{i}].allocation"),
        )?;
        out.push(ClaimPhaseConfig {
            phase: row.phase,
            registry_address,
            allocation,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn gen_ipv4_phases_load() {
        let registry = "1111111111111111111111111111111111111111111111111111111111111111";
        let raw = json!({
            "schema_version": 5,
            "validator_keys": [],
            "gen_cfg": {
                "chain_id": "devnet",
                "epoch": 0,
                "validator_set": "test",
                "block_reward": "0",
                "reward_policy": {
                    "mode": "to_producer_account"
                },
                "marks_coeff": "0",
                "funding": {
                    "accounts": []
                },
                "validators": {
                    "set": []
                },
                "ipv4_claim_phases": [
                    {
                        "phase": 7,
                        "registry_address": registry,
                        "allocation": 1000000
                    },
                    {
                        "phase": 8,
                        "registry_address": registry,
                        "allocation": "42"
                    }
                ]
            }
        });

        let (parsed, _) = parse_genesis_v4(raw).expect("schema v5 genesis parses");

        assert_eq!(parsed.ipv4_claim_phases.len(), 2);
        assert_eq!(parsed.ipv4_claim_phases[0].phase, 7);
        assert_eq!(parsed.ipv4_claim_phases[0].allocation, 1_000_000);
        assert_eq!(parsed.ipv4_claim_phases[1].phase, 8);
        assert_eq!(parsed.ipv4_claim_phases[1].allocation, 42);
        assert_eq!(
            hex::encode(parsed.ipv4_claim_phases[0].registry_address),
            registry
        );
    }

    #[test]
    fn gen_ipv4_phases_reject_dup() {
        let registry_a = "1111111111111111111111111111111111111111111111111111111111111111";
        let registry_b = "2222222222222222222222222222222222222222222222222222222222222222";
        let raw = json!({
            "schema_version": 5,
            "validator_keys": [],
            "gen_cfg": {
                "chain_id": "devnet",
                "epoch": 0,
                "validator_set": "test",
                "block_reward": "0",
                "reward_policy": {
                    "mode": "to_producer_account"
                },
                "marks_coeff": "0",
                "funding": {
                    "accounts": []
                },
                "validators": {
                    "set": []
                },
                "ipv4_claim_phases": [
                    {
                        "phase": 7,
                        "registry_address": registry_a,
                        "allocation": "100"
                    },
                    {
                        "phase": 7,
                        "registry_address": registry_b,
                        "allocation": "200"
                    }
                ]
            }
        });

        let err = match parse_genesis_v4(raw) {
            Ok(_) => panic!("duplicate phase must reject"),
            Err(e) => e,
        };

        assert!(err.contains("gen_cfg.ipv4_claim_phases[1].phase"));
        assert!(err.contains("duplicate phase 7"));
    }

    #[test]
    fn gen_xshard_lock_to_load() {
        let raw = json!({
            "schema_version": 5,
            "validator_keys": [],
            "gen_cfg": {
                "block_reward": "0",
                "reward_policy": { "mode": "to_producer_account" },
                "marks_coeff": "0",
                "funding": { "accounts": [] },
                "validators": { "set": [] },
                "cross_shard_lock_timeout_blocks": 10
            }
        });
        let (parsed, _) = parse_genesis_v4(raw).expect("xshard timeout parses");
        assert_eq!(parsed.xshard_lock_to_blocks, 10);
    }

    #[test]
    fn gen_conservation_delay_load() {
        let raw = json!({
            "schema_version": 5,
            "validator_keys": [],
            "gen_cfg": {
                "block_reward": "0",
                "reward_policy": { "mode": "to_producer_account" },
                "marks_coeff": "0",
                "funding": { "accounts": [] },
                "validators": { "set": [] },
                "conservation_delay_blocks": 10
            }
        });
        let (parsed, _) = parse_genesis_v4(raw).expect("conservation delay parses");
        assert_eq!(parsed.conservation_delay_blocks, 10);
    }

    #[test]
    fn gen_min_val_stake_load() {
        let raw = json!({
            "schema_version": 5,
            "validator_keys": [],
            "gen_cfg": {
                "block_reward": "0",
                "reward_policy": { "mode": "to_producer_account" },
                "marks_coeff": "0",
                "funding": { "accounts": [] },
                "validators": { "set": [] },
                "min_validator_stake": "0"
            }
        });
        let (parsed, _) = parse_genesis_v4(raw).expect("min_validator_stake parses");
        assert_eq!(parsed.min_validator_stake, 0);
    }

    #[test]
    fn gen_xshard_lock_to_default() {
        let raw = json!({
            "schema_version": 5,
            "validator_keys": [],
            "gen_cfg": {
                "block_reward": "0",
                "reward_policy": { "mode": "to_producer_account" },
                "marks_coeff": "0",
                "funding": { "accounts": [] },
                "validators": { "set": [] }
            }
        });
        let (parsed, _) = parse_genesis_v4(raw).expect("default xshard timeout");
        assert_eq!(parsed.xshard_lock_to_blocks, DEF_XSHARD_LOCK_TO);
    }

    #[test]
    fn gen_conservation_delay_default() {
        let raw = json!({
            "schema_version": 5,
            "validator_keys": [],
            "gen_cfg": {
                "block_reward": "0",
                "reward_policy": { "mode": "to_producer_account" },
                "marks_coeff": "0",
                "funding": { "accounts": [] },
                "validators": { "set": [] }
            }
        });
        let (parsed, _) = parse_genesis_v4(raw).expect("default conservation delay");
        assert_eq!(parsed.conservation_delay_blocks, DEF_CONSERV_DELAY_BLOCKS);
    }
}

/// Load `gen_cfg` + encrypted validator keys (schema_version=4/5).
pub fn load_genesis_bundle(
    path: &std::path::Path,
    genesis_passphrase: Option<&str>,
) -> Result<(GenCfg, Vec<SigningKey>), String> {
    let txt = std::fs::read_to_string(path).map_err(|e| format!("read genesis: {e}"))?;
    let raw: Value = serde_json::from_str(&txt)
        .map_err(|e| format!("parse genesis JSON: invalid JSON payload: {e}"))?;
    let obj = raw
        .as_object()
        .ok_or_else(|| "parse genesis JSON: root must be an object".to_string())?;
    let ver = obj.get("schema_version").ok_or_else(|| {
        "parse genesis JSON: missing schema_version (required: 4 or 5)".to_string()
    })?;
    let Some(v) = ver.as_u64() else {
        return Err("parse genesis JSON: schema_version must be an unsigned integer".to_string());
    };
    if v != GENESIS_SCHEMA_VERSION as u64 && v != GENESIS_SCHEMA_BACKWARD as u64 {
        return Err(format!(
            "parse genesis JSON: unsupported schema_version {v}; supported: 4, 5"
        ));
    }
    let (cfg, validator_keys) = parse_genesis_v4(raw)?;
    if cfg.vals.set.is_empty() {
        return Err("gen_cfg.validators.set must not be empty".into());
    }
    if validator_keys.len() != cfg.vals.set.len() {
        return Err("validator_keys length must match gen_cfg.validators.set".into());
    }
    let passphrase = genesis_passphrase.ok_or_else(|| {
        "genesis passphrase is required for schema_version=4/5 (use --genesis-passphrase or PWM_GENESIS_PASSPHRASE)"
            .to_string()
    })?;
    if passphrase.trim().is_empty() {
        return Err("genesis passphrase must not be empty".to_string());
    }
    let mut sks = Vec::new();
    for (i, key_row) in validator_keys.iter().enumerate() {
        if key_row.derivation_path != VALIDATOR_DERIVATION_PATH {
            return Err(format!(
                "validator_keys[{i}].derivation_path must be {VALIDATOR_DERIVATION_PATH}"
            ));
        }
        if key_row.enc_seed.kdf.name != WALLET_KDF {
            return Err(format!(
                "validator_keys[{i}].enc_seed.kdf.name: unsupported kdf '{}'",
                key_row.enc_seed.kdf.name
            ));
        }
        if key_row.enc_seed.kdf.iters > GENESIS_KDF_ITERS_MAX {
            return Err(format!(
                "validator_keys[{i}].enc_seed.kdf.iters exceeds safety cap ({GENESIS_KDF_ITERS_MAX})"
            ));
        }
        if key_row.enc_seed.aead.name != GENESIS_AEAD_NAME {
            return Err(format!(
                "validator_keys[{i}].enc_seed.aead.name: unsupported aead '{}'",
                key_row.enc_seed.aead.name
            ));
        }
        let seed_vec = open_wallet_secret_ciphertext(
            &key_row.enc_seed.aead.ciphertext_b64,
            &key_row.enc_seed.kdf.salt_b64,
            &key_row.enc_seed.aead.nonce_b64,
            &key_row.enc_seed.kdf.name,
            key_row.enc_seed.kdf.iters,
            passphrase,
        )
        .map_err(|e| format!("validator_keys[{i}].enc_seed: {e}"))?;
        if seed_vec.len() != 32 {
            return Err(format!(
                "validator_keys[{i}].enc_seed: seed must be 32 bytes"
            ));
        }
        let mut seed = [0u8; 32];
        seed.copy_from_slice(&seed_vec);
        let row = &cfg.vals.set[i];
        if row.der_idx != VALIDATOR_DERIVATION_PATH_IDX[1] {
            return Err(format!(
                "gen_cfg.validators.set[{i}].der_idx must be {} for derivation path {VALIDATOR_DERIVATION_PATH}",
                VALIDATOR_DERIVATION_PATH_IDX[1]
            ));
        }
        let sk_bytes =
            slip10_ed25519::derive_ed25519_private_key(&seed, &VALIDATOR_DERIVATION_PATH_IDX);
        let sk = SigningKey::from_bytes(&sk_bytes);
        let pk = sk.verifying_key().to_bytes();
        if pk != row.pubkey {
            return Err(format!(
                "validator key {i}: derived pubkey does not match gen_cfg.validators.set[{i}].pubkey"
            ));
        }
        let aid = account_id_from_parts(&pk, row.der_idx);
        if aid != row.acct {
            return Err(format!(
                "validator key {i}: derived account id does not match gen_cfg.validators.set[{i}].acct"
            ));
        }
        sks.push(sk);
    }
    Ok((cfg, sks))
}

pub(crate) fn snapshot_genesis_accounts(cfg: &GenCfg) -> Vec<SnapshotGenesisRow> {
    cfg.funding
        .accounts
        .iter()
        .map(|r| SnapshotGenesisRow {
            acct: r.acct,
            pubkey: r.pubkey,
            der_idx: r.der_idx,
        })
        .collect()
}
