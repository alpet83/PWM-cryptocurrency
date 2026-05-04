//! PoA chain: seal blocks, rotate producer, link `prev_hash`.

use crate::block::{hdr_hash, txs_root, Block, BlockHdr};
use crate::genesis::GenCfg;
use crate::state::{digest, State};
use crate::tx::SignedTx;
use ed25519_dalek::SigningKey;
use std::collections::VecDeque;

/// `Chain::seal` failed; carries txs back for mempool re-injection.
pub type SealAbort = (String, Vec<SignedTx>);

/// Cap on retained in-memory block bodies (canonical height stays in [`Chain::canonical_h`]).
pub const TAIL_BLOCK_CAP: usize = 1000;

/// Synthetic `prev_hash` for the first real block.
pub fn prev_gen() -> [u8; 32] {
    *blake3::hash(b"PWMv0/GENESIS").as_bytes()
}

/// In-memory devnet chain.
pub struct Chain {
    pub cfg: GenCfg,
    /// Same order as `cfg.vals.set`.
    pub val_sks: Vec<SigningKey>,
    /// At most [`TAIL_BLOCK_CAP`] most recent blocks; height [`Self::canonical_h`] is authoritative.
    pub blocks: VecDeque<Block>,
    pub canonical_h: u64,
    pub st: State,
}

impl Chain {
    pub fn boot(cfg: GenCfg, val_sks: Vec<SigningKey>) -> Self {
        assert_eq!(cfg.vals.set.len(), val_sks.len(), "keys vs validators set");
        assert!(!cfg.vals.set.is_empty(), "validators set must not be empty");
        assert_eq!(
            cfg.accounts, cfg.funding.accounts,
            "genesis invariant: accounts must mirror funding.accounts"
        );
        let st = cfg.state0();
        for (i, v) in cfg.vals.set.iter().enumerate() {
            assert!(
                st.accounts.contains_key(&v.acct),
                "genesis invariant: validators.set[{i}].acct must exist in funding.accounts"
            );
        }
        Self {
            cfg,
            val_sks,
            blocks: VecDeque::new(),
            canonical_h: 0,
            st,
        }
    }

    pub fn tip_h(&self) -> u64 {
        self.canonical_h
    }

    pub fn tip_hash(&self) -> [u8; 32] {
        self.blocks
            .back()
            .map(|b| hdr_hash(&b.hdr))
            .unwrap_or_else(prev_gen)
    }

    pub fn set_canon_h(&mut self, height: u64) {
        self.canonical_h = height;
    }

    pub fn sync_canon_h(&mut self) {
        self.canonical_h = self.blocks.back().map(|b| b.hdr.height).unwrap_or(0);
    }

    /// Seals one block: apply txs atomically, accrue marks, pay producer, verify PoA sig.
    pub fn seal(&mut self, txs: Vec<SignedTx>) -> Result<(), SealAbort> {
        let height = self.tip_h() + 1;
        let prev = self.tip_hash();
        let n = self.cfg.vals.set.len();
        let prod_idx = ((height - 1) as usize % n) as u32;

        let mut st = self.st.clone();
        for tx in &txs {
            if let Err(e) = st.apply_tx(tx) {
                return Err((format!("tx: {e}"), txs));
            }
        }
        st.accrue_marks(self.cfg.marks_coeff);
        let prod_acct = self.cfg.prod_acct(prod_idx);
        if !st.accounts.contains_key(&prod_acct) {
            return Err((
                format!(
                    "reward invariant violated: producer account {:02x?} missing from funding.accounts",
                    prod_acct
                ),
                txs,
            ));
        }
        st.reward_producer(&prod_acct, self.cfg.block_reward);

        let state_root = digest(&st);
        let tx_root = txs_root(&txs);
        let ts = match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
            Ok(d) => d.as_secs(),
            Err(e) => return Err((e.to_string(), txs)),
        };

        let hdr = BlockHdr {
            height,
            prev_hash: prev,
            ts,
            prod_idx,
            tx_root,
            state_root,
            sig: [0u8; 64],
        };
        let sk = &self.val_sks[prod_idx as usize];
        let hdr = BlockHdr::sign(sk, hdr);
        if !hdr.verify_sig(&self.cfg.prod_pk(prod_idx)) {
            return Err(("bad block sig".into(), txs));
        }

        let blk = Block { hdr, txs };
        self.st = st;
        self.blocks.push_back(blk);
        while self.blocks.len() > TAIL_BLOCK_CAP {
            self.blocks.pop_front();
        }
        self.canonical_h = height;
        Ok(())
    }
}

/// Truncates a full block list to the in-memory tail cap (used after snapshot load).
pub fn absorb_blocks_tail(blocks: Vec<Block>) -> VecDeque<Block> {
    let mut dq: VecDeque<_> = blocks.into();
    while dq.len() > TAIL_BLOCK_CAP {
        dq.pop_front();
    }
    dq
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genesis::{FundingCfg, RewPol, VRow, ValCfg};
    use crate::hd::account_id_from_parts;
    use slip10_ed25519::derive_ed25519_private_key;

    fn mk_val(seed: [u8; 32]) -> (SigningKey, VRow) {
        let sk = SigningKey::from_bytes(&derive_ed25519_private_key(&seed, &[1_000_000, 1]));
        let pk = sk.verifying_key().to_bytes();
        (
            sk,
            VRow {
                acct: account_id_from_parts(&pk, 1),
                pubkey: pk,
                der_idx: 1,
            },
        )
    }

    #[test]
    fn seal_empty_block() {
        let (g, sks) = crate::genesis::dev_net();
        let mut c = Chain::boot(g, sks);
        c.seal(vec![]).expect("empty seal");
        assert_eq!(c.tip_h(), 1);
    }

    /// Failed seal returns txs and chain height stays 0 (formerly `seal_returns_txs_on_apply_error`).
    #[test]
    fn seal_err_returns_txs_undo() {
        use crate::hd::domain_of_account_id;
        use crate::tx::{SignedTx, TxBody};

        let (g, sks) = crate::genesis::dev_net();
        let sk_v = sks[0].clone();
        let mut c = Chain::boot(g, sks);
        let aid_v = c.cfg.vals.set[0].acct;
        let dom_v = domain_of_account_id(&aid_v);
        let bad = SignedTx::sign_body(&sk_v, dom_v, 0, 99, TxBody::Stake { amount: 1 });
        let want = bad.clone();
        let r = c.seal(vec![bad]);
        assert!(r.is_err());
        let (msg, txs) = r.expect_err("seal must fail");
        assert!(msg.contains("bad nonce"), "msg={msg}");
        assert_eq!(txs, vec![want]);
        assert_eq!(c.tip_h(), 0);
    }

    /// One validator row can coexist with many funded accounts (formerly `seal_allows_one_val_many_funding`).
    #[test]
    fn seal_ok_val_many_fund() {
        let (sk, val) = mk_val([99u8; 32]);
        let mut cfg = crate::genesis::dev_net().0;
        cfg.funding.accounts = vec![
            crate::genesis::GRow {
                acct: val.acct,
                pubkey: val.pubkey,
                der_idx: val.der_idx,
                bal: 0,
            },
            crate::genesis::GRow {
                acct: [7u8; 32],
                pubkey: [8u8; 32],
                der_idx: 7,
                bal: 10,
            },
            crate::genesis::GRow {
                acct: [9u8; 32],
                pubkey: [10u8; 32],
                der_idx: 9,
                bal: 20,
            },
        ];
        cfg.accounts = cfg.funding.accounts.clone();
        cfg.vals = ValCfg {
            set: vec![val.clone()],
        };
        let mut c = Chain::boot(cfg, vec![sk]);
        c.seal(vec![]).expect("seal");
        assert_eq!(c.tip_h(), 1);
        assert_eq!(c.blocks[0].hdr.prod_idx, 0);
        assert!(c.blocks[0].hdr.verify_sig(&val.pubkey));
    }

    #[test]
    fn prod_rotation_uses_vals_len() {
        let (sk0, v0) = mk_val([51u8; 32]);
        let (sk1, v1) = mk_val([52u8; 32]);
        let mut cfg = crate::genesis::dev_net().0;
        cfg.funding = FundingCfg {
            accounts: vec![
                crate::genesis::GRow {
                    acct: v0.acct,
                    pubkey: v0.pubkey,
                    der_idx: v0.der_idx,
                    bal: 0,
                },
                crate::genesis::GRow {
                    acct: v1.acct,
                    pubkey: v1.pubkey,
                    der_idx: v1.der_idx,
                    bal: 0,
                },
                crate::genesis::GRow {
                    acct: [17u8; 32],
                    pubkey: [18u8; 32],
                    der_idx: 17,
                    bal: 1,
                },
            ],
        };
        cfg.accounts = cfg.funding.accounts.clone();
        cfg.vals = ValCfg { set: vec![v0, v1] };
        let mut c = Chain::boot(cfg, vec![sk0, sk1]);
        c.seal(vec![]).expect("h1");
        c.seal(vec![]).expect("h2");
        c.seal(vec![]).expect("h3");
        assert_eq!(c.blocks[0].hdr.prod_idx, 0);
        assert_eq!(c.blocks[1].hdr.prod_idx, 1);
        assert_eq!(c.blocks[2].hdr.prod_idx, 0);
    }

    #[test]
    fn canon_h_override_works() {
        let (g, sks) = crate::genesis::dev_net();
        let mut c = Chain::boot(g, sks);
        c.seal(vec![]).expect("seal");
        c.set_canon_h(77);
        assert_eq!(c.tip_h(), 77);
        c.sync_canon_h();
        assert_eq!(c.tip_h(), 1);
    }

    #[test]
    fn reward_default_is_deterministic() {
        let (sk0, v0) = mk_val([71u8; 32]);
        let mut cfg = crate::genesis::dev_net().0;
        cfg.rew = RewPol::ToProducerAccount;
        cfg.funding = FundingCfg {
            accounts: vec![crate::genesis::GRow {
                acct: v0.acct,
                pubkey: v0.pubkey,
                der_idx: v0.der_idx,
                bal: 0,
            }],
        };
        cfg.accounts = cfg.funding.accounts.clone();
        cfg.vals = ValCfg {
            set: vec![v0.clone()],
        };
        cfg.block_reward = 33;
        let mut c = Chain::boot(cfg, vec![sk0]);
        c.seal(vec![]).expect("h1");
        c.seal(vec![]).expect("h2");
        let bal =
            c.st.accounts
                .get(&v0.acct)
                .expect("producer acct")
                .balance_pwm;
        assert_eq!(bal, 66);
    }

    /// Boot panics if validator acct missing from funding rows (formerly `boot_rejects_missing_validator_funding_account`).
    #[test]
    #[should_panic(expected = "validators.set[0].acct must exist in funding.accounts")]
    fn boot_panic_val_not_funded() {
        let (sk0, v0) = mk_val([91u8; 32]);
        let mut cfg = crate::genesis::dev_net().0;
        cfg.funding.accounts.clear();
        cfg.accounts = cfg.funding.accounts.clone();
        cfg.vals = ValCfg { set: vec![v0] };
        let _ = Chain::boot(cfg, vec![sk0]);
    }

    #[test]
    fn tail_cap_evicts_old_keeps_tip() {
        let (g, sks) = crate::genesis::dev_net();
        let mut c = Chain::boot(g, sks);
        for _ in 0..1005 {
            c.seal(vec![]).expect("seal");
        }
        assert_eq!(c.tip_h(), 1005);
        assert!(c.blocks.len() <= TAIL_BLOCK_CAP);
        assert_eq!(c.blocks.len(), TAIL_BLOCK_CAP);
        assert_eq!(c.blocks.front().map(|b| b.hdr.height), Some(6));
        assert_eq!(c.blocks.back().map(|b| b.hdr.height), Some(1005));
        c.seal(vec![]).expect("seal after cap");
        assert_eq!(c.tip_h(), 1006);
        assert_eq!(c.blocks.back().map(|b| b.hdr.height), Some(1006));
    }
}
