//! Operator-local peer sync scoring (non-consensus).

use std::collections::HashMap;

use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PeerScoreEvent {
    SyncRound,
    ValidBlocks,
    Timeout,
    ForkMismatch,
    BridgeTrustRefusal,
}

impl PeerScoreEvent {
    pub(crate) fn delta(self) -> i64 {
        match self {
            Self::SyncRound => 1,
            Self::ValidBlocks => 1,
            Self::Timeout => -2,
            Self::ForkMismatch => -5,
            Self::BridgeTrustRefusal => -10,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
pub(crate) struct PeerSyncScore {
    pub(crate) peer_id: String,
    pub(crate) score: i64,
    pub(crate) sync_rounds: u64,
    pub(crate) valid_blocks: u64,
    pub(crate) timeouts: u64,
    pub(crate) fork_mismatches: u64,
    pub(crate) bridge_trust_refusals: u64,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct PeerSyncScoreCache {
    pub(crate) scores: HashMap<String, PeerSyncScore>,
}

impl PeerSyncScoreCache {
    pub(crate) fn apply(&mut self, peer_id: &str, event: PeerScoreEvent) -> i64 {
        let row = self
            .scores
            .entry(peer_id.to_string())
            .or_insert_with(|| PeerSyncScore {
                peer_id: peer_id.to_string(),
                ..PeerSyncScore::default()
            });
        row.score = row.score.saturating_add(event.delta());
        match event {
            PeerScoreEvent::SyncRound => row.sync_rounds = row.sync_rounds.saturating_add(1),
            PeerScoreEvent::ValidBlocks => row.valid_blocks = row.valid_blocks.saturating_add(1),
            PeerScoreEvent::Timeout => row.timeouts = row.timeouts.saturating_add(1),
            PeerScoreEvent::ForkMismatch => {
                row.fork_mismatches = row.fork_mismatches.saturating_add(1)
            }
            PeerScoreEvent::BridgeTrustRefusal => {
                row.bridge_trust_refusals = row.bridge_trust_refusals.saturating_add(1)
            }
        }
        row.score
    }

    pub(crate) fn get(&self, peer_id: &str) -> i64 {
        self.scores.get(peer_id).map(|row| row.score).unwrap_or(0)
    }
}

pub(crate) fn score_sort(peer_ids: &mut [String], scores: &PeerSyncScoreCache) {
    peer_ids.sort_by(|a, b| scores.get(b).cmp(&scores.get(a)).then_with(|| a.cmp(b)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_score_deterministic_deltas() {
        let mut scores = PeerSyncScoreCache::default();
        assert_eq!(scores.apply("peer-a", PeerScoreEvent::SyncRound), 1);
        assert_eq!(scores.apply("peer-a", PeerScoreEvent::ValidBlocks), 2);
        assert_eq!(scores.apply("peer-a", PeerScoreEvent::Timeout), 0);
        assert_eq!(scores.apply("peer-a", PeerScoreEvent::ForkMismatch), -5);
        assert_eq!(
            scores.apply("peer-a", PeerScoreEvent::BridgeTrustRefusal),
            -15
        );
        let row = scores.scores.get("peer-a").expect("score row");
        assert_eq!(row.sync_rounds, 1);
        assert_eq!(row.valid_blocks, 1);
        assert_eq!(row.timeouts, 1);
        assert_eq!(row.fork_mismatches, 1);
        assert_eq!(row.bridge_trust_refusals, 1);
    }

    #[test]
    fn peer_score_tie_order() {
        let mut scores = PeerSyncScoreCache::default();
        scores.apply("peer-b", PeerScoreEvent::SyncRound);
        scores.apply("peer-a", PeerScoreEvent::SyncRound);
        scores.apply("peer-c", PeerScoreEvent::Timeout);
        let mut peer_ids = vec![
            "peer-c".to_string(),
            "peer-b".to_string(),
            "peer-a".to_string(),
        ];
        score_sort(&mut peer_ids, &scores);
        assert_eq!(peer_ids, vec!["peer-a", "peer-b", "peer-c"]);
    }
}
