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
    pub rows: Vec<GRow>,
    pub block_reward: u128,
    pub marks_coeff: u128,
}

impl GenCfg {
    /// Builds initial `State` (all rows pre-initialized).
    pub fn state0(&self) -> State {
        let mut st = State::default();
        for r in &self.rows {
            st.accounts
                .insert(r.acct, Account::genesis_funded(r.pubkey, r.der_idx, r.bal));
        }
        st
    }

    pub fn prod_acct(&self, prod_idx: u32) -> AccountId {
        let n = self.rows.len();
        self.rows[(prod_idx as usize) % n].acct
    }
}

/// Single-validator devnet: seed `[99;32]` (`m/0'/0'`) so wallets can use other seeds without colliding.
pub fn dev_net() -> (GenCfg, Vec<SigningKey>) {
    let seed = [99u8; 32];
    let sk_bytes = derive_ed25519_private_key(&seed, &[0, 0]);
    let sk = SigningKey::from_bytes(&sk_bytes);
    let pk = sk.verifying_key().to_bytes();
    let aid = account_id_from_parts(&pk, 0u32);
    let g = GenCfg {
        rows: vec![GRow {
            acct: aid,
            pubkey: pk,
            der_idx: 0,
            bal: 1_000_000u128,
        }],
        block_reward: 100u128,
        marks_coeff: 10_000u128,
    };
    (g, vec![sk])
}
