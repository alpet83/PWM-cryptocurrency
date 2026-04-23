//! FIFO tx queue for block sealing.

use crate::tx::SignedTx;
use std::collections::VecDeque;

/// Bounded FIFO pool.
pub struct Mpool {
    cap: usize,
    q: VecDeque<SignedTx>,
}

impl Mpool {
    pub fn new(cap: usize) -> Self {
        Self {
            cap,
            q: VecDeque::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.q.len()
    }

    pub fn push(&mut self, tx: SignedTx) -> Result<(), &'static str> {
        if self.q.len() >= self.cap {
            return Err("pool full");
        }
        self.q.push_back(tx);
        Ok(())
    }

    /// Take up to `max` txs without re-adding (for one block).
    pub fn take(&mut self, max: usize) -> Vec<SignedTx> {
        let mut out = Vec::new();
        while out.len() < max {
            match self.q.pop_front() {
                Some(t) => out.push(t),
                None => break,
            }
        }
        out
    }

    /// Put txs back at the **front** in block order (`txs[0]` becomes the next `pop_front`).
    pub fn prepend_block(&mut self, txs: Vec<SignedTx>) {
        for tx in txs.into_iter().rev() {
            self.q.push_front(tx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genesis::dev_net;
    use crate::hd::{account_id_from_parts, domain_of_account_id};
    use crate::tx::{SignedTx, TxBody};
    use ed25519_dalek::SigningKey;
    use slip10_ed25519::derive_ed25519_private_key;

    fn user_sk(seed: &[u8; 32]) -> (SigningKey, u32, crate::types::AccountId) {
        let sk_bytes = derive_ed25519_private_key(seed, &[0, 0]);
        let sk = SigningKey::from_bytes(&sk_bytes);
        let i = 0u32;
        let pk = sk.verifying_key().to_bytes();
        let aid = account_id_from_parts(&pk, i);
        (sk, i, aid)
    }

    #[test]
    fn prepend_block_restores_fifo_order() {
        let mut p = Mpool::new(16);
        let (sk_a, i_a, aid_a) = user_sk(&[41u8; 32]);
        let da = domain_of_account_id(&aid_a);
        let t1 = SignedTx::sign_body(&sk_a, da, i_a, 0, TxBody::Init { index: 0, flags: 0 });
        let (sk_b, i_b, aid_b) = user_sk(&[42u8; 32]);
        let db = domain_of_account_id(&aid_b);
        let t2 = SignedTx::sign_body(&sk_b, db, i_b, 0, TxBody::Init { index: 0, flags: 0 });
        p.push(t1.clone()).unwrap();
        p.push(t2.clone()).unwrap();
        let taken = p.take(2);
        assert_eq!(taken.len(), 2);
        assert_eq!(p.len(), 0);
        p.prepend_block(taken);
        let again = p.take(1);
        assert_eq!(again, vec![t1]);
        let tail = p.take(1);
        assert_eq!(tail, vec![t2]);
    }

    /// Failed `Chain::seal` returns txs; `prepend_block` restores the pool (two `dev_net()` snapshots).
    #[test]
    fn seal_fail_then_prepend_keeps_len() {
        let (g1, sks1) = dev_net();
        let mut p = Mpool::new(4096);
        let sk_v = &sks1[0];
        let aid_v = g1.rows[0].acct;
        let dom_v = domain_of_account_id(&aid_v);
        let bad = SignedTx::sign_body(sk_v, dom_v, 0, 99, TxBody::Stake { amount: 1 });
        p.push(bad.clone()).unwrap();
        let txs = p.take(64);
        let (g2, sks2) = dev_net();
        let mut c = crate::Chain::boot(g2, sks2);
        let r = c.seal(txs);
        assert!(r.is_err());
        let (_msg, txs_back) = r.unwrap_err();
        p.prepend_block(txs_back);
        assert_eq!(p.len(), 1);
        assert_eq!(p.take(1), vec![bad]);
    }
}
