//! Tokio harness exercising idle peer sessions with synthetic listeners.

use super::super::*;
use super::harness::{
    format_events, reserve_loopback_addr, run_inbound_peer, run_outbound_peer, HarnessDiag,
    HarnessNode,
};

/// Micro-node harness keeps idle outbound sessions alive across heartbeat/timer windows.
#[tokio::test]
async fn peer_micro_idle_hb_ok() {
    let addr_a = reserve_loopback_addr();
    let addr_b = reserve_loopback_addr();
    let listener_a = tokio::net::TcpListener::bind(addr_a)
        .await
        .expect("bind node a");
    let listener_b = tokio::net::TcpListener::bind(addr_b)
        .await
        .expect("bind node b");
    let node_a = HarnessNode {
        node_id: "node-a",
        cluster_id: "cluster-a",
        domain_hi: 0x10,
        listen: addr_a,
        seed: addr_b,
    };
    let node_b = HarnessNode {
        node_id: "node-b",
        cluster_id: "cluster-b",
        domain_hi: 0x20,
        listen: addr_b,
        seed: addr_a,
    };
    assert_eq!(node_a.listen, addr_a);
    assert_eq!(node_b.listen, addr_b);
    let diag = HarnessDiag::new();
    let inbound_a = tokio::spawn(run_inbound_peer(listener_a, node_a.clone(), diag.clone()));
    let inbound_b = tokio::spawn(run_inbound_peer(listener_b, node_b.clone(), diag.clone()));
    let outbound_a = tokio::spawn(run_outbound_peer(node_a, diag.clone(), 5));
    let outbound_b = tokio::spawn(run_outbound_peer(node_b, diag.clone(), 5));
    let out = tokio::time::timeout(Duration::from_secs(5), async {
        let a = outbound_a.await.expect("outbound a task");
        let b = outbound_b.await.expect("outbound b task");
        (a, b)
    })
    .await
    .expect("peer-only harness timed out");
    let events = diag.snapshot().await;
    let dump = format_events(&events);
    assert!(out.0.is_ok(), "outbound a failed: {:?}\n{}", out.0, dump);
    assert!(out.1.is_ok(), "outbound b failed: {:?}\n{}", out.1, dump);
    inbound_a.abort();
    inbound_b.abort();

    let heartbeat_sent = events
        .iter()
        .filter(|e| e.action == "sent" && e.frame == Some("heartbeat"))
        .count();
    let heartbeat_acks = events
        .iter()
        .filter(|e| e.action == "read" && e.frame == Some("heartbeat_ack"))
        .count();
    let idle_reads = events.iter().filter(|e| e.action == "idle").count();
    let unexpected_errors = events
        .iter()
        .filter(|e| {
            e.action.ends_with("error")
                || e.error
                    .as_deref()
                    .unwrap_or_default()
                    .contains("wire_read_failed")
                || e.error
                    .as_deref()
                    .unwrap_or_default()
                    .contains("heartbeat_read_failed")
        })
        .count();
    assert!(
        heartbeat_sent >= 10,
        "expected reciprocal heartbeats\n{}",
        dump
    );
    assert!(heartbeat_acks >= 10, "expected heartbeat acks\n{}", dump);
    assert!(idle_reads >= 1, "expected idle timeout evidence\n{}", dump);
    assert_eq!(unexpected_errors, 0, "unexpected peer errors\n{}", dump);
}
