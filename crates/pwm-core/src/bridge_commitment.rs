//! Level-2 bridge federation digest only (`imported_set` + `exported_registry`).
//!
//! Intentionally **not** a full [`crate::state::State`] encode: future `State` fields must not
//! change this commitment. See `docs/WHITE_SPEC_v0.md` §7.5 / RFC 9 Appendix A.6.

use crate::state::{ExportProvenance, State};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Minimal bridge payload hashed for federation trust.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BridgeFederationCommitment {
    pub imported_set: BTreeSet<[u8; 32]>,
    pub exported_registry: BTreeMap<[u8; 32], ExportProvenance>,
}

impl BridgeFederationCommitment {
    pub fn from_state(state: &State) -> Self {
        Self {
            imported_set: state.imported_set.clone(),
            exported_registry: state.exported_registry.clone(),
        }
    }

    /// Blake3 over bincode of **only** the two bridge maps (no accounts / fee pool / marks).
    pub fn digest_hex(&self) -> String {
        let bytes = bincode::serialize(self).expect("bridge commitment bincode");
        hex::encode(blake3::hash(&bytes).as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ExportProvenance;

    #[test]
    fn digest_is_deterministic_for_same_payload() {
        let mut a = BridgeFederationCommitment {
            imported_set: BTreeSet::new(),
            exported_registry: BTreeMap::new(),
        };
        a.imported_set.insert([1u8; 32]);
        a.exported_registry.insert(
            [2u8; 32],
            ExportProvenance {
                to: [3u8; 32],
                target_domain: 0x201,
                amount: 5,
            },
        );
        assert_eq!(a.digest_hex(), a.digest_hex());
    }

    #[test]
    fn digest_ignores_state_accounts_fee() {
        use crate::types::Account;

        let mut st = State::default();
        st.imported_set.insert([7u8; 32]);
        st.accounts.insert([99u8; 32], Account::default());
        st.fee_pool = 12345;
        let from_bridge = BridgeFederationCommitment::from_state(&st).digest_hex();
        let mut st2 = State::default();
        st2.imported_set = st.imported_set.clone();
        assert_eq!(
            from_bridge,
            BridgeFederationCommitment::from_state(&st2).digest_hex()
        );
    }
}
