//! Cross-shard facts ledger backing exports, imports, and API summaries.

use pwm_core::hd::domain_of_account_id;
use pwm_core::state::ExportProvenance;
use pwm_core::tx::{SignedTx, TxBody};
use pwm_core::AccountId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(crate) const SUMMARY_BLOCK_INTERVAL: u64 = 500;
const FACT_CAP: usize = 4096;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CrossShardStatus {
    Exported,
    Registered,
    Imported,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CrossShardOrigin {
    Local,
    TrustedPeer,
}

impl Default for CrossShardOrigin {
    fn default() -> Self {
        Self::Local
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct CrossShardFact {
    pub(crate) export_id: [u8; 32],
    pub(crate) source_domain_hi: u8,
    pub(crate) target_domain_hi: u8,
    #[serde(
        serialize_with = "crate::wire_serde::ser_u128_hex",
        deserialize_with = "crate::wire_serde::de_u128_compat"
    )]
    pub(crate) amount: u128,
    pub(crate) status: CrossShardStatus,
    pub(crate) first_height: u64,
    pub(crate) last_height: u64,
    #[serde(default)]
    pub(crate) source: Option<AccountId>,
    pub(crate) to: AccountId,
    #[serde(default)]
    pub(crate) intent_id: Option<[u8; 32]>,
    #[serde(default)]
    pub(crate) origin: CrossShardOrigin,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct CrossShardLedger {
    #[serde(default)]
    facts: BTreeMap<[u8; 32], CrossShardFact>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub(crate) struct CrossShardSummary {
    pub(crate) scope: &'static str,
    pub(crate) total_exported_count: u64,
    pub(crate) total_exported_amount: u128,
    pub(crate) total_imported_count: u64,
    pub(crate) total_imported_amount: u128,
    pub(crate) pending_count: u64,
    pub(crate) trusted_peer_observed_count: u64,
    pub(crate) by_domain_hi: Vec<CrossShardDomainSummary>,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
pub(crate) struct CrossShardDomainSummary {
    pub(crate) domain_hi: u8,
    pub(crate) exported_count: u64,
    pub(crate) exported_amount: u128,
    pub(crate) imported_count: u64,
    pub(crate) imported_amount: u128,
    pub(crate) pending_count: u64,
    pub(crate) trusted_peer_observed_count: u64,
}

impl CrossShardStatus {
    fn rank(self) -> u8 {
        match self {
            Self::Exported => 1,
            Self::Registered => 2,
            Self::Imported => 3,
        }
    }
}

impl CrossShardLedger {
    pub(crate) fn fact(&self, export_id: &[u8; 32]) -> Option<&CrossShardFact> {
        self.facts.get(export_id)
    }

    pub(crate) fn facts(&self) -> Vec<CrossShardFact> {
        self.facts.values().cloned().collect()
    }

    pub(crate) fn facts_for_target(
        &self,
        target_domain_hi: u8,
        from_height: u64,
        limit: usize,
    ) -> Vec<CrossShardFact> {
        let capped = limit.clamp(1, FACT_CAP);
        let mut rows = self
            .facts
            .values()
            .filter(|f| f.target_domain_hi == target_domain_hi && f.last_height >= from_height)
            .cloned()
            .collect::<Vec<_>>();
        // Height window semantics: deterministic order by `last_height`,
        // then by `export_id` as a stable tie-break.
        rows.sort_by_key(|f| (f.last_height, f.export_id));
        rows.truncate(capped);
        rows
    }

    pub(crate) fn insert_peer_fact(&mut self, mut fact: CrossShardFact) -> bool {
        fact.origin = CrossShardOrigin::TrustedPeer;
        self.upsert(fact)
    }

    pub(crate) fn insert_fact(&mut self, fact: CrossShardFact) -> bool {
        self.upsert(fact)
    }

    pub(crate) fn record_export(
        &mut self,
        tx: &SignedTx,
        height: u64,
        intent_id: Option<[u8; 32]>,
    ) {
        let TxBody::Export {
            to,
            target_domain,
            amount,
            ..
        } = &tx.body
        else {
            return;
        };
        let Some(export_id) = tx.export_id() else {
            return;
        };
        let fact = CrossShardFact {
            export_id,
            source_domain_hi: tx.domain_code.to_be_bytes()[0],
            target_domain_hi: target_domain.to_be_bytes()[0],
            amount: *amount,
            status: CrossShardStatus::Exported,
            first_height: height,
            last_height: height,
            source: Some(tx.computed_account_id()),
            to: *to,
            intent_id,
            origin: CrossShardOrigin::Local,
        };
        self.upsert(fact);
    }

    pub(crate) fn record_handoff(
        &mut self,
        export_id: [u8; 32],
        source_domain_hi: u8,
        source: AccountId,
        to: AccountId,
        target_domain: u16,
        amount: u128,
        height: u64,
        intent_id: Option<[u8; 32]>,
    ) {
        let fact = CrossShardFact {
            export_id,
            source_domain_hi,
            target_domain_hi: target_domain.to_be_bytes()[0],
            amount,
            status: CrossShardStatus::Registered,
            first_height: height,
            last_height: height,
            source: Some(source),
            to,
            intent_id,
            origin: CrossShardOrigin::Local,
        };
        self.upsert(fact);
    }

    pub(crate) fn record_import(
        &mut self,
        tx: &SignedTx,
        height: u64,
        provenance: Option<&ExportProvenance>,
    ) {
        let TxBody::Import {
            to,
            amount,
            export_id,
        } = &tx.body
        else {
            return;
        };
        let target_domain_hi = tx.domain_code.to_be_bytes()[0];
        let (known_source_hi, known_amount, known_to) = self
            .facts
            .get(export_id)
            .map(|f| (f.source_domain_hi, f.amount, f.to))
            .or_else(|| {
                provenance.map(|p| {
                    (
                        domain_of_account_id(&tx.computed_account_id()).to_be_bytes()[0],
                        p.amount,
                        p.to,
                    )
                })
            })
            .unwrap_or((0, *amount, *to));
        let fact = CrossShardFact {
            export_id: *export_id,
            source_domain_hi: known_source_hi,
            target_domain_hi,
            amount: known_amount,
            status: CrossShardStatus::Imported,
            first_height: height,
            last_height: height,
            source: self.facts.get(export_id).and_then(|f| f.source),
            to: known_to,
            intent_id: self.facts.get(export_id).and_then(|f| f.intent_id),
            origin: CrossShardOrigin::Local,
        };
        self.upsert(fact);
    }

    pub(crate) fn summary(&self) -> CrossShardSummary {
        let mut by_domain = BTreeMap::<u8, CrossShardDomainSummary>::new();
        let mut out = CrossShardSummary {
            scope: "local_plus_trusted_peer_observations",
            total_exported_count: 0,
            total_exported_amount: 0,
            total_imported_count: 0,
            total_imported_amount: 0,
            pending_count: 0,
            trusted_peer_observed_count: 0,
            by_domain_hi: Vec::new(),
        };
        for fact in self.facts.values() {
            let export_row = by_domain.entry(fact.target_domain_hi).or_default();
            export_row.domain_hi = fact.target_domain_hi;
            if fact.origin == CrossShardOrigin::TrustedPeer {
                export_row.trusted_peer_observed_count =
                    export_row.trusted_peer_observed_count.saturating_add(1);
                out.trusted_peer_observed_count = out.trusted_peer_observed_count.saturating_add(1);
            }
            export_row.exported_count = export_row.exported_count.saturating_add(1);
            export_row.exported_amount = export_row.exported_amount.saturating_add(fact.amount);
            out.total_exported_count = out.total_exported_count.saturating_add(1);
            out.total_exported_amount = out.total_exported_amount.saturating_add(fact.amount);

            if fact.status == CrossShardStatus::Imported {
                let import_row = by_domain.entry(fact.source_domain_hi).or_default();
                import_row.domain_hi = fact.source_domain_hi;
                if fact.origin == CrossShardOrigin::TrustedPeer
                    && fact.source_domain_hi != fact.target_domain_hi
                {
                    import_row.trusted_peer_observed_count =
                        import_row.trusted_peer_observed_count.saturating_add(1);
                }
                import_row.imported_count = import_row.imported_count.saturating_add(1);
                import_row.imported_amount = import_row.imported_amount.saturating_add(fact.amount);
                out.total_imported_count = out.total_imported_count.saturating_add(1);
                out.total_imported_amount = out.total_imported_amount.saturating_add(fact.amount);
            } else {
                export_row.pending_count = export_row.pending_count.saturating_add(1);
                out.pending_count = out.pending_count.saturating_add(1);
            }
        }
        out.by_domain_hi = by_domain.into_values().collect();
        out
    }

    fn upsert(&mut self, mut fact: CrossShardFact) -> bool {
        let changed = match self.facts.get_mut(&fact.export_id) {
            Some(existing) => {
                if fact.status.rank() < existing.status.rank() {
                    return false;
                }
                if existing.origin == CrossShardOrigin::Local
                    && fact.origin == CrossShardOrigin::TrustedPeer
                {
                    fact.origin = CrossShardOrigin::Local;
                }
                fact.first_height = existing.first_height.min(fact.first_height);
                if fact.source.is_none() {
                    fact.source = existing.source;
                }
                if fact.intent_id.is_none() {
                    fact.intent_id = existing.intent_id;
                }
                let changed = *existing != fact;
                *existing = fact;
                changed
            }
            None => {
                self.facts.insert(fact.export_id, fact);
                true
            }
        };
        while self.facts.len() > FACT_CAP {
            if let Some(oldest) = self
                .facts
                .values()
                .min_by_key(|f| (f.last_height, f.export_id))
                .map(|f| f.export_id)
            {
                self.facts.remove(&oldest);
            } else {
                break;
            }
        }
        changed
    }
}

pub(crate) fn summary_log_line(summary: &CrossShardSummary) -> String {
    format!(
        "export/import summary: scope={} exported_count={} exported_amount={} imported_count={} imported_amount={} pending_count={} trusted_peer_observed_count={} domains={}",
        summary.scope,
        summary.total_exported_count,
        summary.total_exported_amount,
        summary.total_imported_count,
        summary.total_imported_amount,
        summary.pending_count,
        summary.trusted_peer_observed_count,
        summary
            .by_domain_hi
            .iter()
            .map(|d| format!(
                "0x{:02X}:ex={}/{} im={}/{} pend={} peer_obs={}",
                d.domain_hi,
                d.exported_count,
                d.exported_amount,
                d.imported_count,
                d.imported_amount,
                d.pending_count,
                d.trusted_peer_observed_count
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use pwm_core::tx::TxBody;
    use pwm_core::SignedTx;

    fn signed_export() -> (SignedTx, [u8; 32]) {
        let sk = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
        let to = [0x20u8; 32];
        let tx = SignedTx::sign_body(
            &sk,
            0x1001,
            0,
            0,
            TxBody::Export {
                to,
                target_domain: 0x2001,
                amount: 42,
                fee: 1,
            },
        );
        let export_id = tx.export_id().expect("export id");
        (tx, export_id)
    }

    #[test]
    fn export_fact_recorded() {
        let (tx, export_id) = signed_export();
        let mut ledger = CrossShardLedger::default();
        ledger.record_export(&tx, 7, None);
        let fact = ledger.facts.get(&export_id).expect("fact");
        assert_eq!(fact.status, CrossShardStatus::Exported);
        assert_eq!(fact.amount, 42);
        assert_eq!(fact.source_domain_hi, 0x10);
        assert_eq!(fact.target_domain_hi, 0x20);
    }

    #[test]
    fn import_fact_recorded() {
        let (export_tx, export_id) = signed_export();
        let mut ledger = CrossShardLedger::default();
        ledger.record_export(&export_tx, 7, None);
        let sk = ed25519_dalek::SigningKey::from_bytes(&[8u8; 32]);
        let tx = SignedTx::sign_body(
            &sk,
            0x2001,
            0,
            0,
            TxBody::Import {
                to: [0x20u8; 32],
                amount: 42,
                export_id,
            },
        );
        ledger.record_import(&tx, 9, None);
        let fact = ledger.facts.get(&export_id).expect("fact");
        assert_eq!(fact.status, CrossShardStatus::Imported);
        assert_eq!(ledger.summary().total_imported_count, 1);
    }

    /// Peer fact upserts must not regress imported lifecycle (formerly `peer_fact_update_preserves_imported_status`).
    #[test]
    fn peer_row_keep_imp() {
        let (tx, export_id) = signed_export();
        let mut ledger = CrossShardLedger::default();
        ledger.record_export(&tx, 7, None);
        let mut fact = ledger.facts.get(&export_id).cloned().expect("fact");
        fact.status = CrossShardStatus::Imported;
        fact.last_height = 8;
        assert!(ledger.insert_peer_fact(fact));
        assert_eq!(
            ledger.facts.get(&export_id).expect("fact").status,
            CrossShardStatus::Imported
        );
    }

    /// Summary labels peer rows as trusted observations (formerly `peer_fact_is_labeled_as_trusted_observation`).
    #[test]
    fn trusted_obs_label_ok() {
        let (tx, export_id) = signed_export();
        let mut peer_ledger = CrossShardLedger::default();
        peer_ledger.record_export(&tx, 7, None);
        let fact = peer_ledger.facts.get(&export_id).cloned().expect("fact");

        let mut ledger = CrossShardLedger::default();
        assert!(ledger.insert_peer_fact(fact));

        let summary = ledger.summary();
        assert_eq!(summary.scope, "local_plus_trusted_peer_observations");
        assert_eq!(summary.trusted_peer_observed_count, 1);
        assert_eq!(summary.by_domain_hi[0].trusted_peer_observed_count, 1);
    }

    #[test]
    fn summary_format_is_compact() {
        let (tx, _) = signed_export();
        let mut ledger = CrossShardLedger::default();
        ledger.record_export(&tx, 7, None);
        let line = summary_log_line(&ledger.summary());
        assert!(line.contains("export/import summary"));
        assert!(line.contains("pending_count=1"));
        assert!(line.contains("0x20"));
    }

    #[test]
    fn facts_target_ordered_by_height() {
        let mut ledger = CrossShardLedger::default();
        let mk = |id: u8, h: u64| CrossShardFact {
            export_id: [id; 32],
            source_domain_hi: 0x20,
            target_domain_hi: 0x10,
            amount: 1,
            status: CrossShardStatus::Registered,
            first_height: h,
            last_height: h,
            source: Some([1u8; 32]),
            to: [2u8; 32],
            intent_id: None,
            origin: CrossShardOrigin::Local,
        };
        assert!(ledger.insert_fact(mk(0x03, 12)));
        assert!(ledger.insert_fact(mk(0x01, 10)));
        assert!(ledger.insert_fact(mk(0x02, 10)));
        let rows = ledger.facts_for_target(0x10, 10, 2);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].last_height, 10);
        assert_eq!(rows[0].export_id, [0x01; 32]);
        assert_eq!(rows[1].last_height, 10);
        assert_eq!(rows[1].export_id, [0x02; 32]);
    }
}
