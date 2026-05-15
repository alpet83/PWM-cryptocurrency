//! Node-level federation shard height dictionary (Sprint 15 contract).

use crate::handshake::NodeHello;
use crate::identity::runtime_shard_label;
use crate::App;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;

pub(crate) const FEDERATION_TTL_SEC: u64 = 60;
const FED_TTL_MS: u64 = FEDERATION_TTL_SEC * 1000;
/// Cap gossip rows per heartbeat (wire size vs fan-out trade-off).
const GOSSIP_WIRE_MAX_ROWS: usize = 32;
/// Soft payload budget for serialized gossip rows (approximate).
const GOSSIP_WIRE_MAX_BYTES: usize = 4096;
const GOSSIP_INBOUND_MAX_ROWS: usize = 64;
const GOSSIP_SHARD_ID_MAX: usize = 128;
const GOSSIP_NODE_ID_MAX: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FedRowSource {
    Hello,
    Heartbeat,
    Status,
    /// Trusted relay: row arrived inside a peer heartbeat gossip bundle.
    Gossip,
}

impl FedRowSource {
    pub(crate) fn as_json(self) -> &'static str {
        match self {
            FedRowSource::Hello => "hello",
            FedRowSource::Heartbeat => "heartbeat",
            FedRowSource::Status => "status",
            FedRowSource::Gossip => "gossip",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FedShardRow {
    pub(crate) shard_id: String,
    pub(crate) latest_height: u64,
    pub(crate) last_seen_unix_ms: u64,
    pub(crate) source: FedRowSource,
    pub(crate) source_node_id: String,
}

/// Single federation row relayed inside a trusted peer heartbeat (JSON wire).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct FedGossipWireRow {
    pub(crate) shard_id: String,
    pub(crate) latest_height: u64,
    pub(crate) last_seen_unix_ms: u64,
    /// Original observer (`merge_row` provenance), not the relaying peer.
    pub(crate) source_node_id: String,
}

#[derive(Default)]
pub(crate) struct FederationTable {
    rows: HashMap<String, FedShardRow>,
}

impl FederationTable {
    /// Rows for trusted heartbeat gossip: freshest first, non-expired, bounded.
    pub(crate) fn gossip_wire_rows(&self, now_ms: u64) -> Vec<FedGossipWireRow> {
        let mut v: Vec<&FedShardRow> = self.rows.values().collect();
        v.retain(|r| now_ms < r.last_seen_unix_ms.saturating_add(FED_TTL_MS));
        v.sort_by(|a, b| b.last_seen_unix_ms.cmp(&a.last_seen_unix_ms));
        let mut out = Vec::new();
        let mut approx = 0usize;
        for r in v.into_iter().take(GOSSIP_WIRE_MAX_ROWS) {
            let row = FedGossipWireRow {
                shard_id: r.shard_id.clone(),
                latest_height: r.latest_height,
                last_seen_unix_ms: r.last_seen_unix_ms,
                source_node_id: r.source_node_id.clone(),
            };
            approx = approx.saturating_add(row.shard_id.len() + row.source_node_id.len() + 48);
            if approx > GOSSIP_WIRE_MAX_BYTES && !out.is_empty() {
                break;
            }
            out.push(row);
        }
        out
    }

    pub(crate) fn sweep_expired(&mut self, now_ms: u64) {
        self.rows
            .retain(|_, row| now_ms < row.last_seen_unix_ms.saturating_add(FED_TTL_MS));
    }

    pub(crate) fn merge_row(&mut self, incoming: FedShardRow) {
        let key = incoming.shard_id.clone();
        match self.rows.get_mut(&key) {
            None => {
                self.rows.insert(key, incoming);
            }
            Some(cur) => match incoming.latest_height.cmp(&cur.latest_height) {
                Ordering::Greater => *cur = incoming,
                Ordering::Equal => {
                    if incoming.last_seen_unix_ms > cur.last_seen_unix_ms {
                        cur.last_seen_unix_ms = incoming.last_seen_unix_ms;
                        cur.source = incoming.source;
                        cur.source_node_id = incoming.source_node_id;
                    }
                }
                Ordering::Less => {
                    cur.last_seen_unix_ms = cur.last_seen_unix_ms.max(incoming.last_seen_unix_ms);
                    cur.source = incoming.source;
                    cur.source_node_id = incoming.source_node_id;
                }
            },
        }
    }

    pub(crate) fn build_http_view(
        &self,
        now_ms: u64,
        expected: Option<u32>,
    ) -> FederationShardsOut {
        let mut rows_out = Vec::new();
        let mut active = 0u32;
        let mut stale = 0u32;
        for row in self.rows.values() {
            let expires_at_unix_ms = row.last_seen_unix_ms.saturating_add(FED_TTL_MS);
            let fresh = now_ms < expires_at_unix_ms;
            if fresh {
                active += 1;
            } else {
                stale += 1;
            }
            rows_out.push(FederationShardRowOut {
                shard_id: row.shard_id.clone(),
                latest_height: row.latest_height,
                last_seen_unix_ms: row.last_seen_unix_ms,
                ttl_sec: FEDERATION_TTL_SEC,
                expires_at_unix_ms,
                source: row.source.as_json(),
                source_node_id: row.source_node_id.clone(),
                fresh,
            });
        }
        rows_out.sort_by(|a, b| a.shard_id.cmp(&b.shard_id));
        let view_health = view_health_label(expected, active, stale);
        FederationShardsOut {
            generated_at_unix_ms: now_ms,
            ttl_sec: FEDERATION_TTL_SEC,
            view_health,
            expected_shard_count: expected,
            active_shard_count: active,
            stale_shard_count: stale,
            rows: rows_out,
        }
    }
}

fn view_health_label(expected: Option<u32>, active: u32, stale: u32) -> &'static str {
    if stale > 0 {
        return "stale";
    }
    match expected {
        Some(exp) => {
            if active == exp {
                "complete"
            } else {
                "partial"
            }
        }
        None => {
            if active == 0 {
                "partial"
            } else {
                "complete"
            }
        }
    }
}

fn merge_gossip_rows(table: &mut FederationTable, rows: Vec<FedGossipWireRow>) {
    for r in rows.into_iter().take(GOSSIP_INBOUND_MAX_ROWS) {
        if r.shard_id.is_empty()
            || r.shard_id.len() > GOSSIP_SHARD_ID_MAX
            || r.source_node_id.len() > GOSSIP_NODE_ID_MAX
        {
            continue;
        }
        table.merge_row(FedShardRow {
            shard_id: r.shard_id,
            latest_height: r.latest_height,
            last_seen_unix_ms: r.last_seen_unix_ms,
            source: FedRowSource::Gossip,
            source_node_id: r.source_node_id,
        });
    }
}

pub(crate) fn fallback_shard_key(hello: &NodeHello) -> String {
    format!(
        "{}:0x{:02x}",
        hello.cluster.cluster_id, hello.cluster.domain_hi
    )
}

pub(crate) async fn merge_remote_hello(app: &App, hello: &NodeHello, now_ms: u64) {
    let Some(height) = hello.chain_tip_height else {
        return;
    };
    let shard = hello
        .federation_shard_id
        .clone()
        .unwrap_or_else(|| fallback_shard_key(hello));
    let row = FedShardRow {
        shard_id: shard,
        latest_height: height,
        last_seen_unix_ms: now_ms,
        source: FedRowSource::Hello,
        source_node_id: hello.node.node_id.clone(),
    };
    let mut g = app.inner.write().await;
    g.federation.merge_row(row);
}

pub(crate) async fn merge_remote_hb(
    app: &App,
    trusted: bool,
    remote_hello: &NodeHello,
    remote_node_id: &str,
    observed_ms: u64,
    chain_tip_height: Option<u64>,
    federation_shard_id: Option<String>,
    federation_gossip: Option<Vec<FedGossipWireRow>>,
) {
    let mut g = app.inner.write().await;
    if trusted {
        if let Some(rows) = federation_gossip {
            merge_gossip_rows(&mut g.federation, rows);
        }
    }
    if !trusted {
        return;
    }
    let Some(height) = chain_tip_height else {
        return;
    };
    let shard = federation_shard_id
        .or_else(|| remote_hello.federation_shard_id.clone())
        .unwrap_or_else(|| fallback_shard_key(remote_hello));
    let row = FedShardRow {
        shard_id: shard,
        latest_height: height,
        last_seen_unix_ms: observed_ms,
        source: FedRowSource::Heartbeat,
        source_node_id: remote_node_id.to_string(),
    };
    g.federation.merge_row(row);
}

pub(crate) async fn merge_local_status(app: &App, now_ms: u64) {
    let tip;
    let shard_lbl;
    {
        let g = app.inner.read().await;
        tip = g.chain.tip_h();
        shard_lbl = runtime_shard_label(&app.identity, app.shard);
    }
    let row = FedShardRow {
        shard_id: shard_lbl,
        latest_height: tip,
        last_seen_unix_ms: now_ms,
        source: FedRowSource::Status,
        source_node_id: app.identity.node_id.clone(),
    };
    let mut g = app.inner.write().await;
    g.federation.merge_row(row);
}

pub(crate) async fn federation_http_snapshot(app: &App, now_ms: u64) -> FederationShardsOut {
    merge_local_status(app, now_ms).await;
    let mut g = app.inner.write().await;
    let out = g.federation.build_http_view(now_ms, None);
    g.federation.sweep_expired(now_ms);
    out
}

pub fn spawn_federation_sweep_loop(app: App) {
    tokio::spawn(async move {
        let mut iv = tokio::time::interval(std::time::Duration::from_millis(1000));
        loop {
            iv.tick().await;
            if !app.init.read().await.is_ready() {
                continue;
            }
            let Ok(now_ms) = crate::current_time_ms() else {
                continue;
            };
            let mut g = app.inner.write().await;
            g.federation.sweep_expired(now_ms);
        }
    });
}

#[derive(Serialize)]
pub struct FederationShardsOut {
    pub generated_at_unix_ms: u64,
    pub ttl_sec: u64,
    pub view_health: &'static str,
    pub expected_shard_count: Option<u32>,
    pub active_shard_count: u32,
    pub stale_shard_count: u32,
    pub rows: Vec<FederationShardRowOut>,
}

#[derive(Serialize)]
pub struct FederationShardRowOut {
    pub shard_id: String,
    pub latest_height: u64,
    pub last_seen_unix_ms: u64,
    pub ttl_sec: u64,
    pub expires_at_unix_ms: u64,
    pub source: &'static str,
    pub source_node_id: String,
    pub fresh: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hello_stub(shard_lbl: &str, node: &str, height: u64) -> NodeHello {
        NodeHello {
            network_id: "n".into(),
            genesis_hash: None,
            cluster: crate::handshake::NodeHelloCluster {
                domain_hi: 1,
                cluster_id: "c".into(),
            },
            node: crate::handshake::NodeHelloNode {
                node_id: node.into(),
                pubkey: [0u8; 32],
            },
            capabilities: crate::handshake::NodeHelloCapabilities {
                protocol_version: "0.1.0".into(),
                tx_features: vec!["t".into()],
                services: vec!["s".into()],
                sync_profile: None,
                deployment_profile: crate::handshake::DeploymentProfile::SingleSealer,
                seal_role: crate::handshake::SealRole::Active,
                validator_identity_hash: Some("vh-federation".into()),
                node_instance_id: Some("inst-federation".into()),
                lease_owner_id: None,
                lease_term: None,
                lease_expires_at_ms: None,
                lease_last_tip: None,
                lease_fence: None,
                cluster_attest_enabled: false,
                cluster_role: crate::handshake::ClusterRole::None,
                cluster_members: Vec::new(),
                cluster_quorum_k: None,
                cluster_quorum_n: None,
            },
            nonce: vec![1],
            timestamp_ms: 0,
            signature: vec![0u8; 64],
            chain_tip_height: Some(height),
            federation_shard_id: Some(shard_lbl.into()),
            bridge_commitment: None,
        }
    }

    /// Monotonic merges keep max height and fresh `last_seen` rules (formerly `merge_height_monotonic_and_seen_max`).
    #[test]
    fn fed_merge_seen_mono() {
        let mut t = FederationTable::default();
        t.merge_row(FedShardRow {
            shard_id: "A".into(),
            latest_height: 5,
            last_seen_unix_ms: 100,
            source: FedRowSource::Hello,
            source_node_id: "n1".into(),
        });
        t.merge_row(FedShardRow {
            shard_id: "A".into(),
            latest_height: 10,
            last_seen_unix_ms: 200,
            source: FedRowSource::Heartbeat,
            source_node_id: "n2".into(),
        });
        let r = t.rows.get("A").expect("row");
        assert_eq!(r.latest_height, 10);
        assert_eq!(r.last_seen_unix_ms, 200);
        assert_eq!(r.source, FedRowSource::Heartbeat);

        t.merge_row(FedShardRow {
            shard_id: "A".into(),
            latest_height: 10,
            last_seen_unix_ms: 150,
            source: FedRowSource::Hello,
            source_node_id: "n3".into(),
        });
        let r = t.rows.get("A").expect("row");
        assert_eq!(r.latest_height, 10);
        assert_eq!(r.last_seen_unix_ms, 200);

        t.merge_row(FedShardRow {
            shard_id: "A".into(),
            latest_height: 8,
            last_seen_unix_ms: 300,
            source: FedRowSource::Hello,
            source_node_id: "n4".into(),
        });
        let r = t.rows.get("A").expect("row");
        assert_eq!(r.latest_height, 10);
        assert_eq!(r.last_seen_unix_ms, 300);
        assert_eq!(r.source, FedRowSource::Hello);
    }

    #[test]
    fn sweep_drops_expired() {
        let mut t = FederationTable::default();
        t.merge_row(FedShardRow {
            shard_id: "A".into(),
            latest_height: 1,
            last_seen_unix_ms: 1_000,
            source: FedRowSource::Status,
            source_node_id: "local".into(),
        });
        t.sweep_expired(1_000 + FED_TTL_MS);
        assert!(t.rows.is_empty());
    }

    #[test]
    fn view_health_semantics() {
        let mut t = FederationTable::default();
        let now = 10_000u64;
        t.merge_row(FedShardRow {
            shard_id: "A".into(),
            latest_height: 1,
            last_seen_unix_ms: now,
            source: FedRowSource::Status,
            source_node_id: "x".into(),
        });
        let v = t.build_http_view(now, Some(2));
        assert_eq!(v.view_health, "partial");
        assert_eq!(v.active_shard_count, 1);
        assert_eq!(v.stale_shard_count, 0);

        let v2 = t.build_http_view(now, Some(1));
        assert_eq!(v2.view_health, "complete");

        let mut t2 = FederationTable::default();
        let expired_seen = 1_000u64;
        t2.merge_row(FedShardRow {
            shard_id: "B".into(),
            latest_height: 1,
            last_seen_unix_ms: expired_seen,
            source: FedRowSource::Hello,
            source_node_id: "y".into(),
        });
        let now = expired_seen + FED_TTL_MS + 1;
        let v3 = t2.build_http_view(now, None);
        assert_eq!(v3.stale_shard_count, 1);
        assert_eq!(v3.view_health, "stale");
    }

    #[test]
    fn fallback_shard_key_maps_cluster() {
        let mut h = hello_stub("explicit", "nid", 3);
        h.federation_shard_id = None;
        assert!(fallback_shard_key(&h).contains("c:"));
    }

    /// Two-hop relay delivers shard gossip without direct carrier hello (formerly `gossip_convergence_relays_shard_without_direct_carrier_session`).
    #[test]
    fn gossip_via_relay_shard() {
        let mut mid = FederationTable::default();
        mid.merge_row(FedShardRow {
            shard_id: "SHARD_X".into(),
            latest_height: 42,
            last_seen_unix_ms: 1_000,
            source: FedRowSource::Hello,
            source_node_id: "carrier".into(),
        });
        mid.merge_row(FedShardRow {
            shard_id: "MID_LOCAL".into(),
            latest_height: 3,
            last_seen_unix_ms: 1_100,
            source: FedRowSource::Status,
            source_node_id: "mid_node".into(),
        });
        let relay = mid.gossip_wire_rows(2_000);
        assert!(
            relay.iter().any(|r| r.shard_id == "SHARD_X"),
            "gossip pack must include foreign shard row"
        );

        let mut observer = FederationTable::default();
        observer.merge_row(FedShardRow {
            shard_id: "OBS".into(),
            latest_height: 1,
            last_seen_unix_ms: 2_000,
            source: FedRowSource::Status,
            source_node_id: "observer".into(),
        });
        merge_gossip_rows(&mut observer, relay);

        let row = observer
            .rows
            .get("SHARD_X")
            .expect("indirect shard visible");
        assert_eq!(row.latest_height, 42);
        assert_eq!(row.source_node_id, "carrier");
        assert_eq!(row.source, FedRowSource::Gossip);
    }
}
