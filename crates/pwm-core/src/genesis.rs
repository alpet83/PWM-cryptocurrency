//! Genesis rows + dev factory.
//! Genesis marks are initialized from whole PWM balance at `state0()`.

use crate::hd::account_id_from_parts;
use crate::ser_json_u128;
use crate::state::State;
use crate::types::{Account, AccountId};
use crate::MARKS_CAP;
use crate::PWM_RAW_SCALE;
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use slip10_ed25519::derive_ed25519_private_key;

pub const LEGACY_POLICY_VER: u32 = 1;
pub const DEF_BLOCKS_PER_HOUR: u64 = 3600;
pub const DEF_MARKS_HOUR: u64 = 1;
pub const DEF_PWM_STAKE_MIN: u128 = 100_000;
pub const DEF_MARKS_STAKE_MIN: u128 = PWM_RAW_SCALE;
pub const DEF_SEASON_COEFF_PPM: u64 = 1_000_000;
pub const DEF_BASE_EMIT: u128 = 100;

fn default_blocks_per_hour() -> u64 {
    DEF_BLOCKS_PER_HOUR
}

fn default_marks_hour() -> u64 {
    DEF_MARKS_HOUR
}

fn default_pol_ver() -> u32 {
    LEGACY_POLICY_VER
}

fn default_pwm_stake_min() -> u128 {
    DEF_PWM_STAKE_MIN
}

fn default_marks_stake_min() -> u128 {
    DEF_MARKS_STAKE_MIN
}

fn default_season_coeff_ppm() -> u64 {
    DEF_SEASON_COEFF_PPM
}

fn default_base_emit() -> u128 {
    DEF_BASE_EMIT
}

/// IPv4 claim phase allocation config carried in genesis.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaimPhaseConfig {
    pub phase: u8,
    pub registry_address: AccountId,
    #[serde(with = "ser_json_u128")]
    pub allocation: u128,
}

/// One funded validator row at height 0.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GRow {
    pub acct: AccountId,
    pub pubkey: [u8; 32],
    pub der_idx: u32,
    #[serde(with = "ser_json_u128")]
    pub bal: u128,
}

/// Chain params at genesis.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenCfg {
    pub funding: FundingCfg,
    pub vals: ValCfg,
    #[serde(default)]
    pub rew: RewPol,
    pub accounts: Vec<GRow>,
    #[serde(default = "default_blocks_per_hour")]
    pub blocks_per_hour: u64,
    #[serde(default = "default_marks_hour", rename = "marks_per_coin_per_hour")]
    pub marks_per_hour: u64,
    #[serde(default)]
    pub ipv4_claim_phases: Vec<ClaimPhaseConfig>,
    #[serde(with = "ser_json_u128")]
    pub block_reward: u128,
    #[serde(with = "ser_json_u128")]
    pub marks_coeff: u128,
    #[serde(default = "default_pol_ver")]
    pub policy_ver: u32,
    #[serde(default = "default_base_emit", with = "ser_json_u128")]
    pub base_emission_per_block: u128,
    #[serde(default = "default_pwm_stake_min", with = "ser_json_u128")]
    pub pwm_stake_min: u128,
    #[serde(default = "default_marks_stake_min", with = "ser_json_u128")]
    pub marks_stake_min: u128,
    #[serde(default)]
    pub season_enabled: bool,
    #[serde(default = "default_season_coeff_ppm")]
    pub season_coeff_ppm: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FundingCfg {
    pub accounts: Vec<GRow>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct VRow {
    pub acct: AccountId,
    pub pubkey: [u8; 32],
    pub der_idx: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValCfg {
    pub set: Vec<VRow>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum RewPol {
    #[default]
    ToProducerAccount,
}

impl GenCfg {
    /// Builds initial `State` (all rows pre-initialized).
    pub fn state0(&self) -> State {
        let mut st = State::default();
        for r in &self.funding.accounts {
            let mut acc = Account::genesis_funded(r.pubkey, r.der_idx, r.bal);
            acc.stored_marks =
                (r.bal / crate::display::PWM_RAW_SCALE).min(MARKS_CAP as u128) as u32;
            st.accounts.insert(r.acct, acc);
        }
        st
    }

    pub fn prod_acct(&self, prod_idx: u32) -> AccountId {
        match self.rew {
            RewPol::ToProducerAccount => {
                let n = self.vals.set.len();
                self.vals.set[(prod_idx as usize) % n].acct
            }
        }
    }

    pub fn prod_pk(&self, prod_idx: u32) -> [u8; 32] {
        let n = self.vals.set.len();
        self.vals.set[(prod_idx as usize) % n].pubkey
    }

    pub fn is_legacy_policy(&self) -> bool {
        self.policy_ver == LEGACY_POLICY_VER
    }

    /// Seasonal multiplier in ppm (`1_000_000` == 1.0), derived from block context only.
    pub fn season_ppm(&self, block_ts: u64) -> u128 {
        let _ = block_ts;
        if self.season_enabled {
            u128::from(self.season_coeff_ppm)
        } else {
            u128::from(DEF_SEASON_COEFF_PPM)
        }
    }
}

/// Single-validator devnet: seed `[99;32]` (`m/0'/0'`) so wallets can use other seeds without colliding.
pub fn dev_net() -> (GenCfg, Vec<SigningKey>) {
    let seed = [99u8; 32];
    let sk_bytes = derive_ed25519_private_key(&seed, &[0, 0]);
    let sk = SigningKey::from_bytes(&sk_bytes);
    let pk = sk.verifying_key().to_bytes();
    let aid = account_id_from_parts(&pk, 0u32);
    let row = GRow {
        acct: aid,
        pubkey: pk,
        der_idx: 0,
        bal: 1_000_000u128,
    };
    let g = GenCfg {
        funding: FundingCfg {
            accounts: vec![row.clone()],
        },
        vals: ValCfg {
            set: vec![VRow {
                acct: row.acct,
                pubkey: row.pubkey,
                der_idx: row.der_idx,
            }],
        },
        rew: RewPol::ToProducerAccount,
        accounts: vec![row],
        blocks_per_hour: DEF_BLOCKS_PER_HOUR,
        marks_per_hour: DEF_MARKS_HOUR,
        ipv4_claim_phases: Vec::new(),
        block_reward: 100u128,
        marks_coeff: 10_000u128,
        policy_ver: LEGACY_POLICY_VER,
        base_emission_per_block: DEF_BASE_EMIT,
        pwm_stake_min: DEF_PWM_STAKE_MIN,
        marks_stake_min: DEF_MARKS_STAKE_MIN,
        season_enabled: false,
        season_coeff_ppm: DEF_SEASON_COEFF_PPM,
    };
    (g, vec![sk])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hd::account_id_from_parts;
    use ed25519_dalek::SigningKey;
    use serde_json::json;
    use slip10_ed25519::derive_ed25519_private_key;

    fn mk_cfg_one_row(bal: u128) -> GenCfg {
        let seed = [13u8; 32];
        let sk_bytes = derive_ed25519_private_key(&seed, &[0, 0]);
        let sk = SigningKey::from_bytes(&sk_bytes);
        let pk = sk.verifying_key().to_bytes();
        let aid = account_id_from_parts(&pk, 0u32);
        let row = GRow {
            acct: aid,
            pubkey: pk,
            der_idx: 0,
            bal,
        };
        let mut cfg = dev_net().0;
        cfg.funding.accounts = vec![row.clone()];
        cfg.accounts = vec![row.clone()];
        cfg.vals.set = vec![VRow {
            acct: row.acct,
            pubkey: row.pubkey,
            der_idx: row.der_idx,
        }];
        cfg
    }

    #[test]
    fn genesis_marks_from_bal() {
        let cfg = mk_cfg_one_row(2_000_000);
        let aid = cfg.accounts[0].acct;
        let st = cfg.state0();
        assert_eq!(st.accounts.get(&aid).expect("acct").stored_marks, 2);
    }

    #[test]
    fn genesis_marks_zero_bal() {
        let cfg = mk_cfg_one_row(0);
        let aid = cfg.accounts[0].acct;
        let st = cfg.state0();
        assert_eq!(st.accounts.get(&aid).expect("acct").stored_marks, 0);
    }

    #[test]
    fn genesis_marks_saturation() {
        let cfg = mk_cfg_one_row(u128::MAX);
        let aid = cfg.accounts[0].acct;
        let st = cfg.state0();
        assert_eq!(st.accounts.get(&aid).expect("acct").stored_marks, MARKS_CAP);
    }

    #[test]
    fn gen_cfg_json_round_trip() {
        let mut cfg = dev_net().0;
        cfg.blocks_per_hour = 7200;
        cfg.marks_per_hour = 3;
        cfg.base_emission_per_block = 42;
        cfg.season_coeff_ppm = 750_000;
        cfg.ipv4_claim_phases = vec![ClaimPhaseConfig {
            phase: 1,
            registry_address: cfg.accounts[0].acct,
            allocation: 123_456_789,
        }];

        let raw = serde_json::to_value(&cfg).expect("ser");
        assert_eq!(raw["base_emission_per_block"], json!("42"));
        assert_eq!(raw["season_coeff_ppm"], json!(750000));
        assert_eq!(raw["pwm_stake_min"], json!(DEF_PWM_STAKE_MIN.to_string()));
        assert_eq!(
            raw["ipv4_claim_phases"][0]["allocation"],
            json!("123456789")
        );

        let back: GenCfg = serde_json::from_value(raw).expect("de");
        assert_eq!(back, cfg);
    }

    #[test]
    fn gen_cfg_defaults_sparse_json() {
        let cfg: GenCfg = serde_json::from_value(json!({
            "funding": { "accounts": [] },
            "vals": { "set": [] },
            "accounts": [],
            "block_reward": "100",
            "marks_coeff": "10000"
        }))
        .expect("de sparse");

        assert_eq!(cfg.blocks_per_hour, DEF_BLOCKS_PER_HOUR);
        assert_eq!(cfg.marks_per_hour, DEF_MARKS_HOUR);
        assert_eq!(cfg.base_emission_per_block, DEF_BASE_EMIT);
        assert_eq!(cfg.pwm_stake_min, DEF_PWM_STAKE_MIN);
        assert_eq!(cfg.marks_stake_min, DEF_MARKS_STAKE_MIN);
        assert_eq!(cfg.season_coeff_ppm, DEF_SEASON_COEFF_PPM);
        assert!(cfg.ipv4_claim_phases.is_empty());
    }
}
