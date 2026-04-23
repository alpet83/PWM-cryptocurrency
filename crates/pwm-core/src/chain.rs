//! PoA chain: seal blocks, rotate producer, link `prev_hash`.

use crate::block::{hdr_hash, txs_root, Block, BlockHdr};
use crate::genesis::GenCfg;
use crate::state::{digest, State};
use crate::tx::SignedTx;
use ed25519_dalek::SigningKey;

/// `Chain::seal` failed; carries txs back for mempool re-injection.
pub type SealAbort = (String, Vec<SignedTx>);

/// Synthetic `prev_hash` for the first real block.
pub fn prev_gen() -> [u8; 32] {
    *blake3::hash(b"PWMv0/GENESIS").as_bytes()
}

/// In-memory devnet chain.
pub struct Chain {
    pub cfg: GenCfg,
    /// Same order as `cfg.rows`.
    pub val_sks: Vec<SigningKey>,
    pub blocks: Vec<Block>,
    pub st: State,
}

impl Chain {
    pub fn boot(cfg: GenCfg, val_sks: Vec<SigningKey>) -> Self {
        assert_eq!(cfg.rows.len(), val_sks.len(), "keys vs genesis rows");
        let st = cfg.state0();
        Self {
            cfg,
            val_sks,
            blocks: vec![],
            st,
        }
    }

    pub fn tip_h(&self) -> u64 {
        self.blocks.len() as u64
    }

    pub fn tip_hash(&self) -> [u8; 32] {
        self.blocks
            .last()
            .map(|b| hdr_hash(&b.hdr))
            .unwrap_or_else(prev_gen)
    }

    /// Seals one block: apply txs atomically, accrue marks, pay producer, verify PoA sig.
    pub fn seal(&mut self, txs: Vec<SignedTx>) -> Result<(), SealAbort> {
        let height = self.tip_h() + 1;
        let prev = self.tip_hash();
        let n = self.cfg.rows.len();
        let prod_idx = ((height - 1) as usize % n) as u32;
        let row = &self.cfg.rows[prod_idx as usize];

        let mut st = self.st.clone();
        for tx in &txs {
            if let Err(e) = st.apply_tx(tx) {
                return Err((format!("tx: {e}"), txs));
            }
        }
        st.accrue_marks(self.cfg.marks_coeff);
        let prod_acct = self.cfg.prod_acct(prod_idx);
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
        if !hdr.verify_sig(&row.pubkey) {
            return Err(("bad block sig".into(), txs));
        }

        let blk = Block { hdr, txs };
        self.st = st;
        self.blocks.push(blk);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seal_empty_block() {
        let (g, sks) = crate::genesis::dev_net();
        let mut c = Chain::boot(g, sks);
        c.seal(vec![]).expect("empty seal");
        assert_eq!(c.tip_h(), 1);
    }

    #[test]
    fn seal_returns_txs_on_apply_error() {
        use crate::hd::domain_of_account_id;
        use crate::tx::{SignedTx, TxBody};

        let (g, sks) = crate::genesis::dev_net();
        let sk_v = sks[0].clone();
        let mut c = Chain::boot(g, sks);
        let aid_v = c.cfg.rows[0].acct;
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
}
