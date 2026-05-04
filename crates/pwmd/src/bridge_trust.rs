//! Bridge-only commitment digest helpers for federation trust gates.

use crate::App;
use pwm_core::BridgeFederationCommitment;

/// Canonical bridge commitment over [`BridgeFederationCommitment`] only.
///
/// Full `State` layout is excluded by design (WHITE_SPEC §7.5 / RFC A.6).
pub(crate) fn bridge_commitment_hex(state: &pwm_core::State) -> String {
    BridgeFederationCommitment::from_state(state).digest_hex()
}

pub(crate) async fn local_bridge_commitment(app: &App) -> String {
    let g = app.inner.read().await;
    bridge_commitment_hex(&g.chain.st)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pwm_core::state::ExportProvenance;

    #[test]
    fn bridge_commitment_is_deterministic() {
        let mut state = pwm_core::State::default();
        state.imported_set.insert([7u8; 32]);
        state.exported_registry.insert(
            [9u8; 32],
            ExportProvenance {
                to: [1u8; 32],
                target_domain: 0x2001,
                amount: 77,
            },
        );

        let a = bridge_commitment_hex(&state);
        let b = bridge_commitment_hex(&state);
        assert_eq!(a, b);
    }
}
