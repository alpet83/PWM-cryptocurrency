//! Genesis rows + dev factory.

use crate::hd::account_id_from_parts;
use crate::state::State;
use crate::types::{Account, AccountId};
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use slip10_ed25519::derive_ed25519_private_key;

/// One funded validator row at height 0.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GRow {
    pub acct: AccountId,
    pub pubkey: [u8; 32],
    pub der_idx: u32,
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
    pub block_reward: u128,
    pub marks_coeff: u128,
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
            st.accounts
                .insert(r.acct, Account::genesis_funded(r.pubkey, r.der_idx, r.bal));
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
        block_reward: 100u128,
        marks_coeff: 10_000u128,
    };
    (g, vec![sk])
}
