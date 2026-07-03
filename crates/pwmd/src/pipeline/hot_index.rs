//! Lock-free account admission data for plain transfer prechecks.

use arc_swap::ArcSwap;
use pwm_core::state::State;
use pwm_core::types::{address_flags, Account, CONSERVATION, COSIGN_NON_DISABLEABLE};
use pwm_core::AccountId;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccountHot {
    pub balance: u128,
    pub nonce: u64,
    pub flags: u32,
    pub active_policies: u8,
    pub initialized: bool,
}

pub struct HotIndex {
    inner: ArcSwap<HashMap<AccountId, AccountHot>>,
}

impl HotIndex {
    pub fn new(state: &State) -> Self {
        Self {
            inner: ArcSwap::from_pointee(build_map(state)),
        }
    }

    pub fn load(&self) -> Arc<HashMap<AccountId, AccountHot>> {
        self.inner.load_full()
    }

    pub fn refresh(&self, state: &State) {
        self.inner.store(Arc::new(build_map(state)));
    }
}

fn build_map(state: &State) -> HashMap<AccountId, AccountHot> {
    state
        .accounts
        .iter()
        .map(|(id, account)| (*id, account_hot(id, account)))
        .collect()
}

fn account_hot(id: &AccountId, account: &Account) -> AccountHot {
    let address_policy = address_flags(id) & (COSIGN_NON_DISABLEABLE | CONSERVATION);
    let policy_sensitive = account.active_policies != 0
        || account.dormant_policies != 0
        || !account.deferred_policies.is_empty()
        || account.finalized
        || account.rescue_address.is_some();
    AccountHot {
        balance: account.balance_pwm,
        nonce: account.nonce,
        flags: account.flags | address_policy,
        active_policies: u8::from(policy_sensitive),
        initialized: account.initialized,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pwm_core::tx::PolicyKind;
    use pwm_core::types::DeferredPolicyEntry;
    use pwm_core::{dev_net, Chain};

    #[test]
    fn hot_index_refreshes_atomically() {
        let (cfg, keys) = dev_net();
        let mut chain = Chain::boot(cfg, keys);
        let id = chain.cfg.accounts[0].acct;
        let index = HotIndex::new(&chain.st);
        let before = index.load();
        let old = before.get(&id).copied().expect("funded account");

        let account = chain.st.accounts.get_mut(&id).expect("funded account");
        account.balance_pwm += 7;
        account.nonce += 1;
        index.refresh(&chain.st);

        let after = index.load();
        let new = after.get(&id).copied().expect("refreshed account");
        assert_eq!(old.balance + 7, new.balance);
        assert_eq!(old.nonce + 1, new.nonce);
        assert_eq!(before.get(&id), Some(&old));
    }

    #[test]
    fn hot_index_marks_policy_sensitive() {
        let (cfg, keys) = dev_net();
        let mut chain = Chain::boot(cfg, keys);
        let id = chain.cfg.accounts[0].acct;

        chain
            .st
            .accounts
            .get_mut(&id)
            .expect("account")
            .active_policies = 0x8000;
        let accounts = HotIndex::new(&chain.st).load();
        assert_eq!(accounts.get(&id).expect("hot account").active_policies, 1);

        let account = chain.st.accounts.get_mut(&id).expect("account");
        account.active_policies = 0;
        account.dormant_policies = 1;
        let accounts = HotIndex::new(&chain.st).load();
        assert_eq!(accounts.get(&id).expect("hot account").active_policies, 1);

        let account = chain.st.accounts.get_mut(&id).expect("account");
        account.dormant_policies = 0;
        account.deferred_policies.push(DeferredPolicyEntry {
            policy: PolicyKind::SenderFilter,
            activate_at_height: 10,
        });
        let accounts = HotIndex::new(&chain.st).load();
        assert_eq!(accounts.get(&id).expect("hot account").active_policies, 1);

        let account = chain.st.accounts.get_mut(&id).expect("account");
        account.deferred_policies.clear();
        account.finalized = true;
        let accounts = HotIndex::new(&chain.st).load();
        assert_eq!(accounts.get(&id).expect("hot account").active_policies, 1);
    }
}
