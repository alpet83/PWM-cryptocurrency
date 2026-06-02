//! PoA chain: seal blocks, rotate producer, link `prev_hash`.

use crate::block::{hdr_hash, txs_root, Block, BlockHdr};
use crate::genesis::GenCfg;
use crate::marks::compute_block_reward;
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

/// Seal-time source for block headers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SealTimeMode {
    /// Default production behavior: use wall clock from `SystemTime::now()`.
    WallClock,
    /// Test/dev behavior: derive timestamp from deterministic height context.
    DeterministicHeight,
}

const DET_SEAL_TS_BASE: u64 = 1_700_000_000;

/// In-memory devnet chain.
pub struct Chain {
    pub cfg: GenCfg,
    /// Same order as `cfg.vals.set`.
    pub val_sks: Vec<SigningKey>,
    /// At most [`TAIL_BLOCK_CAP`] most recent blocks; height [`Self::canonical_h`] is authoritative.
    pub blocks: VecDeque<Block>,
    pub canonical_h: u64,
    pub st: State,
    pub seal_time_mode: SealTimeMode,
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
            seal_time_mode: SealTimeMode::WallClock,
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

    pub fn set_seal_time_mode(&mut self, mode: SealTimeMode) {
        self.seal_time_mode = mode;
    }

    /// Returns `(next_height, now_unix_secs)` for tx application outside/inside sealing.
    pub fn next_apply_ctx(&self) -> Result<(u64, u64), String> {
        let height = self.tip_h() + 1;
        let ts = match self.seal_time_mode {
            SealTimeMode::WallClock => std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_secs(),
            SealTimeMode::DeterministicHeight => DET_SEAL_TS_BASE.saturating_add(height),
        };
        Ok((height, ts))
    }

    /// Seals one block: apply txs atomically, pay producer, verify PoA sig (marks are not accrued here; genesis seeds marks).
    pub fn seal(&mut self, txs: Vec<SignedTx>) -> Result<(), SealAbort> {
        let (height, ts) = self.next_apply_ctx().map_err(|e| (e, txs.clone()))?;
        let prev = self.tip_hash();
        let n = self.cfg.vals.set.len();
        let prod_idx = ((height - 1) as usize % n) as u32;

        let mut st = self.st.clone();
        for tx in &txs {
            if let Err(e) = st.apply_tx_with_ctx(tx, height, ts, &self.cfg) {
                return Err((format!("tx: {e}"), txs));
            }
        }
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
        if self.cfg.is_legacy_policy() {
            st.reward_producer(&prod_acct, self.cfg.block_reward);
        } else {
            let rew = compute_block_reward(&self.cfg, height);
            st.reward_producer_v2(&prod_acct, rew, self.cfg.pwm_stake_min, 1_000_000);
        }

        let state_root = digest(&st);
        let tx_root = txs_root(&txs);
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
    use crate::block::hdr_hash;
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

    #[test]
    fn legacy_keeps_reward_path() {
        let (g, sks) = crate::genesis::dev_net();
        let aid = g.accounts[0].acct;
        let mut c = Chain::boot(g, sks);
        {
            let acc = c.st.accounts.get_mut(&aid).expect("validator");
            acc.staked_pwm_raw = 250_000;
        }
        c.cfg.policy_ver = 1;
        c.cfg.pwm_stake_min = 500_000;
        c.cfg.marks_stake_min = 500_000;
        c.cfg.season_enabled = true;
        c.cfg.season_coeff_ppm = 500_000;
        c.seal(vec![]).expect("seal");
        let acc = c.st.accounts.get(&aid).expect("validator");
        assert_eq!(acc.balance_pwm, 1_000_100);
        // Genesis marks from 1 PWM balance; seal does not call accrue_marks.
        assert_eq!(acc.stored_marks, 1);
    }

    #[test]
    fn policy_v2_gates_with_season() {
        let (g, sks) = crate::genesis::dev_net();
        let aid = g.accounts[0].acct;
        let mut c = Chain::boot(g, sks);
        {
            let acc = c.st.accounts.get_mut(&aid).expect("validator");
            acc.staked_pwm_raw = 250_000;
        }
        c.cfg.policy_ver = 2;
        c.cfg.pwm_stake_min = 200_000;
        c.cfg.marks_stake_min = 200_000;
        c.cfg.season_enabled = true;
        c.cfg.season_coeff_ppm = 500_000;
        c.seal(vec![]).expect("seal");
        let acc = c.st.accounts.get(&aid).expect("validator");
        assert_eq!(acc.balance_pwm, 1_000_050);
        assert_eq!(acc.stored_marks, 1);
    }

    #[test]
    fn policy_v2_uses_float_reward() {
        let (g, sks) = crate::genesis::dev_net();
        let aid = g.accounts[0].acct;
        let mut c = Chain::boot(g, sks);
        {
            let acc = c.st.accounts.get_mut(&aid).expect("validator");
            acc.staked_pwm_raw = 300_000;
        }
        c.cfg.policy_ver = 2;
        c.cfg.pwm_stake_min = 200_000;
        c.cfg.base_emission_per_block = 240;
        c.cfg.season_coeff_ppm = 250_000;
        c.cfg.block_reward = 999;

        c.seal(vec![]).expect("seal");
        let acc = c.st.accounts.get(&aid).expect("validator");
        assert_eq!(acc.balance_pwm, 1_000_060);
    }

    /// Empty seal does not accrue marks; accounts keep genesis-seeded marks only.
    #[test]
    fn seal_no_accrue_marks() {
        let (g, sks) = crate::genesis::dev_net();
        let aid = g.accounts[0].acct;
        let mut c = Chain::boot(g, sks);
        let want = c.st.accounts.get(&aid).expect("acct").stored_marks;
        c.st.accounts.get_mut(&aid).expect("acct").staked_pwm_raw = 250_000;
        c.seal(vec![]).expect("seal");
        let got = c.st.accounts.get(&aid).expect("acct").stored_marks;
        assert_eq!(got, want, "seal must not accrue marks from stake");
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
    fn tail_cap_keeps_tip() {
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

    #[test]
    fn det_mode_stable_hdr_hash() {
        let (g1, sks1) = crate::genesis::dev_net();
        let (g2, sks2) = crate::genesis::dev_net();
        let mut c1 = Chain::boot(g1, sks1);
        let mut c2 = Chain::boot(g2, sks2);
        c1.set_seal_time_mode(SealTimeMode::DeterministicHeight);
        c2.set_seal_time_mode(SealTimeMode::DeterministicHeight);

        c1.seal(vec![]).expect("seal c1");
        c2.seal(vec![]).expect("seal c2");

        let h1 = c1.blocks.back().expect("block c1");
        let h2 = c2.blocks.back().expect("block c2");
        assert_eq!(h1.hdr.ts, h2.hdr.ts);
        assert_eq!(hdr_hash(&h1.hdr), hdr_hash(&h2.hdr));
    }
}
