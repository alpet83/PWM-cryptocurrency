//! EXPORT roaming intents: TTL readiness checks and competing-source locks.

use pwm_core::tx::TxBody;
use pwm_core::{AccountId, SignedTx};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(crate) const DEFAULT_INTENT_TTL_BLOCKS: u64 = 12;
pub(crate) const DEFAULT_READINESS_TTL_SEC: u64 = 30;
pub(crate) const MAX_READINESS_TTL_SEC: u64 = 300;
pub(crate) const ACTIVE_LOCK_ERR_TEXT: &str =
    "roaming intent lock active: competing tx for source funds is blocked";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReadinessCode {
    MissingPreflight,
    StalePreflight,
    BindingMismatch,
    NonceMismatch,
    HeightMismatch,
}

impl ReadinessCode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::MissingPreflight => "missing_preflight",
            Self::StalePreflight => "stale_preflight",
            Self::BindingMismatch => "binding_mismatch",
            Self::NonceMismatch => "nonce_mismatch",
            Self::HeightMismatch => "height_mismatch",
        }
    }

    pub(crate) fn hint(self) -> &'static str {
        match self {
            Self::MissingPreflight => "Run /v1/export-readiness for this exact EXPORT payload before submit.",
            Self::StalePreflight => "Preflight TTL expired; run /v1/export-readiness again and resubmit.",
            Self::BindingMismatch => "EXPORT payload changed since preflight; run /v1/export-readiness again with final payload.",
            Self::NonceMismatch => "Source nonce changed after preflight; regenerate EXPORT and run /v1/export-readiness again.",
            Self::HeightMismatch => {
                "Source chain tip is below preflight height (inconsistent); reload or retry readiness."
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReadinessReject {
    pub(crate) code: ReadinessCode,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ExportReadiness {
    pub(crate) export_id: [u8; 32],
    pub(crate) source: AccountId,
    pub(crate) to: AccountId,
    pub(crate) target_domain: u16,
    pub(crate) amount: u128,
    pub(crate) source_nonce_hint: u64,
    pub(crate) source_height_hint: u64,
    pub(crate) checked_at_unix_ms: u64,
    pub(crate) expires_at_unix_ms: u64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IntentStatus {
    Queued,
    Exported,
    Relayed,
    Imported,
    Expired,
    Failed,
}

impl IntentStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Exported => "exported",
            Self::Relayed => "relayed",
            Self::Imported => "imported",
            Self::Expired => "expired",
            Self::Failed => "failed",
        }
    }

    pub(crate) fn is_terminal(self) -> bool {
        matches!(self, Self::Imported | Self::Expired | Self::Failed)
    }

    pub(crate) fn is_locking(self) -> bool {
        matches!(self, Self::Queued | Self::Exported | Self::Relayed)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RoamingIntent {
    pub(crate) intent_id: [u8; 32],
    pub(crate) export_id: [u8; 32],
    pub(crate) source: AccountId,
    pub(crate) to: AccountId,
    pub(crate) target_domain: u16,
    pub(crate) amount: u128,
    pub(crate) fee: u128,
    pub(crate) status: IntentStatus,
    pub(crate) created_height: u64,
    pub(crate) expires_at_height: u64,
    pub(crate) last_error: Option<String>,
}

#[derive(Default, Clone)]
pub(crate) struct RoamingPool {
    intents: BTreeMap<[u8; 32], RoamingIntent>,
    export_to_intent: BTreeMap<[u8; 32], [u8; 32]>,
    active_locks: BTreeMap<AccountId, [u8; 32]>,
    readiness: BTreeMap<[u8; 32], ExportReadiness>,
}

impl RoamingPool {
    pub(crate) fn register_export(
        &mut self,
        tx: &SignedTx,
        now_height: u64,
        ttl_blocks: u64,
    ) -> Result<([u8; 32], bool), &'static str> {
        let TxBody::Export {
            to,
            target_domain,
            amount,
            fee,
        } = &tx.body
        else {
            return Err("roaming intent requires export tx body");
        };
        let source = tx.computed_account_id();
        let export_id = tx
            .export_id()
            .ok_or("roaming intent requires deterministic export id")?;
        if let Some(existing_intent_id) = self.export_to_intent.get(&export_id).copied() {
            return Ok((existing_intent_id, true));
        }
        if self.active_locks.contains_key(&source) {
            return Err(ACTIVE_LOCK_ERR_TEXT);
        }
        let intent_id = export_id;
        let intent = RoamingIntent {
            intent_id,
            export_id,
            source,
            to: *to,
            target_domain: *target_domain,
            amount: *amount,
            fee: *fee,
            status: IntentStatus::Queued,
            created_height: now_height,
            expires_at_height: now_height.saturating_add(ttl_blocks.max(1)),
            last_error: None,
        };
        self.export_to_intent.insert(export_id, intent_id);
        self.active_locks.insert(source, intent_id);
        self.intents.insert(intent_id, intent);
        Ok((intent_id, false))
    }

    pub(crate) fn mark_exported(&mut self, intent_id: [u8; 32]) {
        self.set_status(intent_id, IntentStatus::Exported, None);
    }

    pub(crate) fn mark_relayed(&mut self, intent_id: [u8; 32]) {
        self.set_status(intent_id, IntentStatus::Relayed, None);
    }

    pub(crate) fn mark_relay_error(&mut self, intent_id: [u8; 32], err: String) {
        if let Some(intent) = self.intents.get_mut(&intent_id) {
            intent.last_error = Some(err);
        }
    }

    pub(crate) fn register_readiness(
        &mut self,
        tx: &SignedTx,
        now_ms: u64,
        ttl_sec: u64,
        source_nonce_hint: u64,
        source_height_hint: u64,
    ) -> Result<ExportReadiness, &'static str> {
        let TxBody::Export {
            to,
            target_domain,
            amount,
            ..
        } = &tx.body
        else {
            return Err("export readiness requires export tx body");
        };
        let source = tx.computed_account_id();
        let export_id = tx
            .export_id()
            .ok_or("export readiness requires deterministic export id")?;
        if let Some(intent_id) = self.export_to_intent.get(&export_id).copied() {
            if let Some(intent) = self.intents.get(&intent_id) {
                if intent.status.is_terminal() {
                    return Err("export readiness rejected: intent is terminal");
                }
            }
        }
        let ttl_ms = ttl_sec.max(1).saturating_mul(1_000);
        let row = ExportReadiness {
            export_id,
            source,
            to: *to,
            target_domain: *target_domain,
            amount: *amount,
            source_nonce_hint,
            source_height_hint,
            checked_at_unix_ms: now_ms,
            expires_at_unix_ms: now_ms.saturating_add(ttl_ms),
        };
        self.readiness.insert(export_id, row.clone());
        Ok(row)
    }

    pub(crate) fn consume_readiness(
        &mut self,
        tx: &SignedTx,
        now_ms: u64,
        source_nonce: u64,
        source_height: u64,
    ) -> Result<(), ReadinessReject> {
        let TxBody::Export {
            to,
            target_domain,
            amount,
            ..
        } = &tx.body
        else {
            return Ok(());
        };
        let source = tx.computed_account_id();
        let export_id = tx.export_id().ok_or(ReadinessReject {
            code: ReadinessCode::BindingMismatch,
        })?;
        let Some(row) = self.readiness.remove(&export_id) else {
            return Err(ReadinessReject {
                code: ReadinessCode::MissingPreflight,
            });
        };
        if now_ms > row.expires_at_unix_ms {
            return Err(ReadinessReject {
                code: ReadinessCode::StalePreflight,
            });
        }
        if row.source != source
            || row.to != *to
            || row.target_domain != *target_domain
            || row.amount != *amount
        {
            return Err(ReadinessReject {
                code: ReadinessCode::BindingMismatch,
            });
        }
        if row.source_nonce_hint != source_nonce {
            return Err(ReadinessReject {
                code: ReadinessCode::NonceMismatch,
            });
        }
        // Tip may advance between preflight and submit (empty seal loop blocks) without changing
        // sender nonce; accept monotonic height. Reject only if tip moved backward vs preflight.
        if source_height < row.source_height_hint {
            return Err(ReadinessReject {
                code: ReadinessCode::HeightMismatch,
            });
        }
        Ok(())
    }

    pub(crate) fn mark_relayed_by_export(&mut self, export_id: [u8; 32]) {
        if let Some(intent_id) = self.export_to_intent.get(&export_id).copied() {
            self.set_status(intent_id, IntentStatus::Relayed, None);
        }
    }

    pub(crate) fn mark_import_by_export(&mut self, export_id: [u8; 32]) {
        if let Some(intent_id) = self.export_to_intent.get(&export_id).copied() {
            self.set_status(intent_id, IntentStatus::Imported, None);
        }
    }

    #[allow(dead_code)]
    pub(crate) fn mark_failed(&mut self, intent_id: [u8; 32], err: String) {
        self.set_status(intent_id, IntentStatus::Failed, Some(err));
    }

    pub(crate) fn lock_conflict_for(&self, tx: &SignedTx) -> Option<[u8; 32]> {
        match tx.body {
            TxBody::Import { .. } => None,
            _ => self.active_locks.get(&tx.computed_account_id()).copied(),
        }
    }

    pub(crate) fn expire_by_height(&mut self, now_height: u64) -> usize {
        let mut expired = 0usize;
        let ids: Vec<[u8; 32]> = self
            .intents
            .iter()
            .filter_map(|(id, intent)| {
                if !intent.status.is_terminal() && now_height > intent.expires_at_height {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        for intent_id in ids {
            self.set_status(
                intent_id,
                IntentStatus::Expired,
                Some("intent ttl exceeded at current height".to_string()),
            );
            expired += 1;
        }
        expired
    }

    pub(crate) fn get(&self, intent_id: &[u8; 32]) -> Option<&RoamingIntent> {
        self.intents.get(intent_id)
    }

    pub(crate) fn get_by_export_id(&self, export_id: &[u8; 32]) -> Option<&RoamingIntent> {
        let intent_id = self.export_to_intent.get(export_id)?;
        self.intents.get(intent_id)
    }

    pub(crate) fn intents_snapshot(&self) -> Vec<RoamingIntent> {
        self.intents.values().cloned().collect()
    }

    pub(crate) fn active_locks_snapshot(&self) -> Vec<(AccountId, [u8; 32])> {
        self.active_locks.iter().map(|(k, v)| (*k, *v)).collect()
    }

    pub(crate) fn restore_from_snapshot(
        intents: Vec<RoamingIntent>,
        lock_rows: Vec<(AccountId, [u8; 32])>,
    ) -> Result<Self, String> {
        let mut out = Self::default();
        for intent in intents {
            if out
                .intents
                .insert(intent.intent_id, intent.clone())
                .is_some()
            {
                return Err(
                    "snapshot roaming intent contract error: duplicate intent_id".to_string(),
                );
            }
            if out
                .export_to_intent
                .insert(intent.export_id, intent.intent_id)
                .is_some()
            {
                return Err(
                    "snapshot roaming intent contract error: duplicate export_id".to_string(),
                );
            }
        }
        for (source, intent_id) in lock_rows {
            if out.active_locks.insert(source, intent_id).is_some() {
                return Err(
                    "snapshot roaming intent contract error: duplicate source lock".to_string(),
                );
            }
        }
        out.rebuild_locks_from_status();
        Ok(out)
    }

    fn set_status(&mut self, intent_id: [u8; 32], status: IntentStatus, err: Option<String>) {
        let Some(intent) = self.intents.get_mut(&intent_id) else {
            return;
        };
        intent.status = status;
        if err.is_some() {
            intent.last_error = err;
        }
        if status.is_locking() {
            self.active_locks.insert(intent.source, intent_id);
        } else {
            self.active_locks.remove(&intent.source);
        }
    }

    fn rebuild_locks_from_status(&mut self) {
        self.active_locks.clear();
        for intent in self.intents.values() {
            if intent.status.is_locking() {
                self.active_locks.insert(intent.source, intent.intent_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pwm_core::hd::domain_of_account_id;
    use pwm_core::{dev_net, tx::TxBody};

    #[test]
    fn expires_after_ttl_height() {
        let (cfg, sks) = dev_net();
        let sk = &sks[0];
        let aid = cfg.accounts[0].acct;
        let sender_dom = domain_of_account_id(&aid);
        let mut recipient = aid;
        recipient[0] = sender_dom.to_be_bytes()[0].wrapping_add(1);
        recipient[1] = 0x01;
        let target_domain = domain_of_account_id(&recipient);
        let tx = SignedTx::sign_body(
            sk,
            sender_dom,
            0,
            0,
            TxBody::Export {
                to: recipient,
                target_domain,
                amount: 10,
                fee: 1,
            },
        );
        let mut pool = RoamingPool::default();
        let (intent_id, is_dup) = pool.register_export(&tx, 5, 3).expect("register");
        assert!(!is_dup);
        pool.mark_exported(intent_id);
        assert_eq!(pool.expire_by_height(8), 0);
        assert_eq!(pool.expire_by_height(9), 1);
        let st = pool.get(&intent_id).expect("intent");
        assert_eq!(st.status, IntentStatus::Expired);
    }

    #[test]
    fn export_rd_tip_advance_empty() {
        let (cfg, sks) = dev_net();
        let sk = &sks[0];
        let aid = cfg.accounts[0].acct;
        let sender_dom = domain_of_account_id(&aid);
        let mut recipient = aid;
        recipient[0] = sender_dom.to_be_bytes()[0].wrapping_add(1);
        recipient[1] = 0x01;
        let target_domain = domain_of_account_id(&recipient);
        let tx = SignedTx::sign_body(
            sk,
            sender_dom,
            0,
            0,
            TxBody::Export {
                to: recipient,
                target_domain,
                amount: 10,
                fee: 1,
            },
        );
        let mut pool = RoamingPool::default();
        pool.register_readiness(&tx, 1_000_000, 60, 0, 5)
            .expect("register readiness at height 5");
        pool.consume_readiness(&tx, 1_000_001, 0, 42)
            .expect("consume ok when tip advanced without nonce change");
    }
}
