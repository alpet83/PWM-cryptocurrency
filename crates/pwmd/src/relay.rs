//! Relay IMPORT flows via configured trusted peers (HTTP status surfaces).

use crate::api::common::{rollback_commit, take_bak};
use crate::api::ExportHandoffOut;
use crate::config::TransportConfig;
use crate::state::{App, FlowTraceRow, InitState};
use axum::http::StatusCode;
use pwm_core::hd::domain_of_account_id;
use pwm_core::tx::TxBody;
use pwm_core::SignedTx;
use serde::Deserialize;
use serde_json::Value;
use std::net::SocketAddr;
use std::time::Duration;
use tracing::{error, info, warn};

pub(crate) const RELAY_MODE: &str = "peer_relay_one_window";
pub(crate) const GENESIS_FETCH_STATUS: &str = "stub_parent_peer_fetch_not_enabled";
pub(crate) const GENESIS_FETCH_HINT: &str =
    "Subordinate genesis fetch is a safe stub: inspect parent peer status, but never replace local genesis silently.";

#[derive(Debug)]
pub(crate) struct RelayErr {
    pub(crate) status: StatusCode,
    pub(crate) message: String,
}

#[derive(Debug)]
struct RelayTarget {
    seed: SocketAddr,
    base: String,
}

#[derive(Deserialize)]
struct SeedStatus {
    #[serde(default)]
    ready: bool,
    #[serde(default)]
    network_id: Option<String>,
    #[serde(default)]
    cluster_domain_hi: Option<u8>,
    #[serde(default)]
    effective_genesis_hash: Option<String>,
    #[serde(default)]
    genesis_guard: Option<String>,
    #[serde(default)]
    bridge_federation_trust: Option<String>,
    #[serde(default)]
    bridge_refusal_reason: Option<String>,
}

#[derive(Deserialize)]
struct PeerHelloAck {
    accepted: bool,
    #[serde(default)]
    reason: Option<String>,
}

fn http_base(addr: SocketAddr) -> String {
    format!("http://{addr}")
}

/// Bases for HTTP relay: explicit `relay_http_seeds`, else derive `peer_tcp.port - 100` per host (matches default `listen`+100 peer listener).
pub(crate) fn relay_http_bases(cfg: &TransportConfig) -> Vec<SocketAddr> {
    if !cfg.relay_http_seeds.is_empty() {
        return cfg.relay_http_seeds.clone();
    }
    let mut out = Vec::with_capacity(cfg.peer_seeds.len());
    for tcp in &cfg.peer_seeds {
        let Some(rpc_port) = tcp.port().checked_sub(100) else {
            warn!(
                peer_tcp = %tcp,
                "relay: skip deriving relay HTTP base (peer port < 100); use --transport-relay-http-seed"
            );
            continue;
        };
        out.push(SocketAddr::new(tcp.ip(), rpc_port));
    }
    out
}

fn target_hi_for_import(tx: &SignedTx) -> Option<u8> {
    match tx.body {
        TxBody::Import { to, .. } => Some(domain_of_account_id(&to).to_be_bytes()[0]),
        _ => None,
    }
}

pub(crate) fn is_foreign_import(tx: &SignedTx, local_hi: u8) -> bool {
    target_hi_for_import(tx)
        .map(|target_hi| target_hi != local_hi)
        .unwrap_or(false)
}

fn relay_err(status: StatusCode, msg: impl Into<String>) -> RelayErr {
    RelayErr {
        status,
        message: msg.into(),
    }
}

fn status_trust_mismatch(
    status: &SeedStatus,
    target_hi: u8,
    expected_network_id: &str,
    expected_genesis_hash: Option<&str>,
) -> Option<String> {
    if !status.ready {
        return Some("ready=false".to_string());
    }
    if status.cluster_domain_hi != Some(target_hi) {
        return Some(format!(
            "cluster_domain_hi={:?} need=0x{target_hi:02X}",
            status.cluster_domain_hi
        ));
    }
    if status.genesis_guard.as_deref().unwrap_or("ok") != "ok" {
        return Some(format!("genesis_guard={:?}", status.genesis_guard));
    }
    if status.network_id.as_deref() != Some(expected_network_id) {
        return Some(format!(
            "network_id={:?} expected={expected_network_id}",
            status.network_id
        ));
    }
    if let Some(expected) = expected_genesis_hash {
        if status.effective_genesis_hash.as_deref() != Some(expected) {
            return Some(format!(
                "effective_genesis_hash={:?} expected={expected}",
                status.effective_genesis_hash
            ));
        }
    }
    if status.bridge_federation_trust.as_deref() == Some("bridge_federation_trust_refused") {
        return Some(format!(
            "bridge_federation_trust_refused: {}",
            status
                .bridge_refusal_reason
                .as_deref()
                .unwrap_or("(no detail)")
        ));
    }
    None
}

/// Log-safe HTTP body: JSON values only (no keys), truncated. Falls back to trimmed text.
fn http_body_log_snippet(body: &str, max: usize) -> String {
    let t = body.trim();
    if t.is_empty() {
        return "(empty)".to_string();
    }
    let compact: String = if let Ok(v) = serde_json::from_str::<Value>(t) {
        json_values_flat(&v)
    } else {
        t.chars().filter(|c| !c.is_control()).collect()
    };
    if compact.chars().count() <= max {
        compact
    } else {
        format!(
            "{}…",
            compact
                .chars()
                .take(max.saturating_sub(1))
                .collect::<String>()
        )
    }
}

fn json_values_flat(v: &Value) -> String {
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().map(json_values_flat).collect::<Vec<_>>().join(" "),
        Value::Object(m) => m
            .values()
            .map(json_values_flat)
            .collect::<Vec<_>>()
            .join(" "),
    }
}

#[derive(Clone, Copy, Debug)]
struct RelayTrace<'a> {
    op: &'static str,
    intent_id: Option<&'a str>,
    export_id: Option<&'a str>,
    target_hi: u8,
}

impl<'a> RelayTrace<'a> {
    fn for_handoff(h: &'a ExportHandoffOut) -> Self {
        Self {
            op: "handoff",
            intent_id: Some(h.intent_id.as_str()),
            export_id: Some(h.export_id.as_str()),
            target_hi: h.target_domain.to_be_bytes()[0],
        }
    }
}

async fn select_target(app: &App, rel: RelayTrace<'_>) -> Result<RelayTarget, RelayErr> {
    {
        let hs = crate::transport::handshake_read_traced(app, "relay").await;
        if hs.bridge_trust.refused {
            return Err(relay_err(
                StatusCode::CONFLICT,
                hs.bridge_trust
                    .refusal_reason
                    .clone()
                    .unwrap_or_else(|| "bridge_federation_trust_refused".to_string()),
            ));
        }
    }
    let target_hi = rel.target_hi;
    let need_domain = format!("0x{target_hi:02X}");
    info!(
        op = rel.op,
        intent_id = rel.intent_id.unwrap_or("-"),
        export_id = rel.export_id.unwrap_or("-"),
        need_domain_hi = %need_domain,
        "relay: begin select_target (GET peer /v1/status)"
    );
    let cfg = app.transport_config.read().await.clone();
    let http_bases = relay_http_bases(&cfg);
    if http_bases.is_empty() {
        warn!(
            op = rel.op,
            intent_id = rel.intent_id.unwrap_or("-"),
            export_id = rel.export_id.unwrap_or("-"),
            "relay: no HTTP relay targets (configure --transport-peer-seed or --transport-relay-http-seed)"
        );
        return Err(relay_err(
            StatusCode::SERVICE_UNAVAILABLE,
            "peer relay unavailable: no HTTP relay base configured (peer seeds or explicit relay-http)",
        ));
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(
            cfg.connect_timeout_ms
                .saturating_add(cfg.handshake_timeout_ms)
                .max(500),
        ))
        .build()
        .map_err(|e| {
            relay_err(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("relay client: {e}"),
            )
        })?;
    let mut last = None;
    let expected_network_id = app.identity.network_id.clone();
    let expected_genesis_hash = {
        let hs = crate::transport::handshake_read_traced(app, "relay").await;
        hs.validation_ctx.expected_genesis_hash.clone()
    };
    for seed in http_bases {
        let base = http_base(seed);
        let url = format!("{base}/v1/status");
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => match resp.json::<SeedStatus>().await {
                Ok(status) => {
                    if let Some(reason) = status_trust_mismatch(
                        &status,
                        target_hi,
                        &expected_network_id,
                        expected_genesis_hash.as_deref(),
                    ) {
                        warn!(
                            op = rel.op,
                            intent_id = rel.intent_id.unwrap_or("-"),
                            export_id = rel.export_id.unwrap_or("-"),
                            %seed,
                            reason = %reason,
                            "relay: select_target status mismatch"
                        );
                        last = Some(format!(
                            "seed {seed} trust gate mismatch for domain_hi=0x{target_hi:02X}: {reason}"
                        ));
                    } else {
                        info!(
                            op = rel.op,
                            intent_id = rel.intent_id.unwrap_or("-"),
                            export_id = rel.export_id.unwrap_or("-"),
                            %seed,
                            "relay: select_target matched peer seed"
                        );
                        return Ok(RelayTarget { seed, base });
                    }
                }
                Err(e) => {
                    warn!(
                        op = rel.op,
                        %seed,
                        err = %e,
                        "relay: select_target status json decode failed"
                    );
                    last = Some(format!("seed {seed} status decode failed: {e}"));
                }
            },
            Ok(resp) => {
                let code = resp.status();
                warn!(
                    op = rel.op,
                    %seed,
                    status = %code,
                    "relay: select_target /v1/status non-success"
                );
                last = Some(format!("seed {seed} status HTTP {code}"));
            }
            Err(e) => {
                warn!(
                    op = rel.op,
                    %seed,
                    err = %e,
                    "relay: select_target /v1/status request failed"
                );
                last = Some(format!("seed {seed} status unavailable: {e}"));
            }
        }
    }
    warn!(
        op = rel.op,
        intent_id = rel.intent_id.unwrap_or("-"),
        export_id = rel.export_id.unwrap_or("-"),
        "relay: select_target exhausted peer_seeds"
    );
    Err(relay_err(
        StatusCode::SERVICE_UNAVAILABLE,
        last.unwrap_or_else(|| format!("no peer seed matched target domain_hi=0x{target_hi:02X}")),
    ))
}

async fn post_peer_hello(
    app: &App,
    client: &reqwest::Client,
    target: &RelayTarget,
    rel: RelayTrace<'_>,
) -> Result<(), RelayErr> {
    info!(
        op = rel.op,
        intent_id = rel.intent_id.unwrap_or("-"),
        export_id = rel.export_id.unwrap_or("-"),
        seed = %target.seed,
        "relay: POST /v1/peer/hello"
    );
    let genesis_hash = {
        let hs = crate::transport::handshake_read_traced(app, "relay").await;
        hs.validation_ctx.expected_genesis_hash.clone()
    };
    let now_ms = crate::current_time_ms().map_err(|e| {
        relay_err(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("peer relay hello clock failed: {}", e.1),
        )
    })?;
    let chain_tip_height = {
        let g = app.inner.read().await;
        Some(g.chain.tip_h())
    };
    let bridge_commitment = crate::bridge_trust::local_bridge_commitment(app).await;
    let hello = crate::transport::build_local_node_hello(
        app,
        genesis_hash,
        Some(bridge_commitment),
        now_ms,
        chain_tip_height,
    );
    let url = format!("{}/v1/peer/hello", target.base);
    let resp = client.post(&url).json(&hello).send().await.map_err(|e| {
        warn!(
            op = rel.op,
            seed = %target.seed,
            err = %e,
            "relay: peer hello transport error"
        );
        relay_err(
            StatusCode::SERVICE_UNAVAILABLE,
            format!("peer relay hello failed seed={}: {e}", target.seed),
        )
    })?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        let snip = http_body_log_snippet(&body, 200);
        warn!(
            op = rel.op,
            intent_id = rel.intent_id.unwrap_or("-"),
            export_id = rel.export_id.unwrap_or("-"),
            seed = %target.seed,
            http_status = %status,
            body_snippet = %snip,
            "relay: peer hello HTTP error"
        );
        return Err(relay_err(
            StatusCode::BAD_GATEWAY,
            format!(
                "peer relay hello rejected seed={} status={status}: {}",
                target.seed, snip
            ),
        ));
    }
    let ack = resp.json::<PeerHelloAck>().await.map_err(|e| {
        warn!(
            op = rel.op,
            seed = %target.seed,
            err = %e,
            "relay: peer hello ack json decode failed"
        );
        relay_err(
            StatusCode::BAD_GATEWAY,
            format!("peer relay hello decode failed seed={}: {e}", target.seed),
        )
    })?;
    if !ack.accepted {
        let reason = ack.reason.unwrap_or_else(|| "unknown".to_string());
        let reason_snip = http_body_log_snippet(&reason, 200);
        warn!(
            op = rel.op,
            seed = %target.seed,
            reason = %reason_snip,
            "relay: peer hello rejected (accepted=false)"
        );
        return Err(relay_err(
            StatusCode::BAD_GATEWAY,
            format!(
                "peer relay hello rejected seed={}: {} intent_id={} export_id={}",
                target.seed,
                reason_snip,
                rel.intent_id.unwrap_or("-"),
                rel.export_id.unwrap_or("-")
            ),
        ));
    }
    info!(
        op = rel.op,
        seed = %target.seed,
        "relay: peer hello ok"
    );
    Ok(())
}

async fn push_relay_flow(
    app: &App,
    h: u64,
    kind: &str,
    tx_id: String,
    export_id: Option<String>,
    intent_id: Option<String>,
    note: String,
) {
    let mut g = app.inner.write().await;
    g.push_flow(FlowTraceRow {
        at_height: h,
        kind: kind.to_string(),
        tx_id,
        export_id,
        intent_id,
        note: Some(note),
    });
}

pub(crate) async fn relay_handoff(app: &App, handoff: &ExportHandoffOut) -> Result<(), RelayErr> {
    let rel = RelayTrace::for_handoff(handoff);
    let target = select_target(app, rel).await?;
    let client = reqwest::Client::new();
    post_peer_hello(app, &client, &target, rel).await?;
    let url = format!("{}/v1/export-provenance", target.base);
    info!(
        op = rel.op,
        intent_id = rel.intent_id.unwrap_or("-"),
        export_id = rel.export_id.unwrap_or("-"),
        seed = %target.seed,
        url = %url,
        "relay: POST /v1/export-provenance"
    );
    let resp = client.post(&url).json(handoff).send().await.map_err(|e| {
        warn!(
            op = rel.op,
            intent_id = rel.intent_id.unwrap_or("-"),
            export_id = rel.export_id.unwrap_or("-"),
            seed = %target.seed,
            err = %e,
            "relay: export-provenance request failed"
        );
        relay_err(
            StatusCode::SERVICE_UNAVAILABLE,
            format!(
                "peer relay handoff failed seed={}: {e} intent_id={} export_id={}",
                target.seed, handoff.intent_id, handoff.export_id
            ),
        )
    })?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        let snip = http_body_log_snippet(&body, 200);
        warn!(
            op = rel.op,
            intent_id = rel.intent_id.unwrap_or("-"),
            export_id = rel.export_id.unwrap_or("-"),
            seed = %target.seed,
            http_status = %status,
            body_snippet = %snip,
            "relay: export-provenance HTTP error"
        );
        return Err(relay_err(
            StatusCode::BAD_GATEWAY,
            format!(
                "peer relay handoff rejected seed={} status={status}: {} intent_id={} export_id={}",
                target.seed, snip, handoff.intent_id, handoff.export_id
            ),
        ));
    }
    info!(
        op = rel.op,
        intent_id = rel.intent_id.unwrap_or("-"),
        export_id = rel.export_id.unwrap_or("-"),
        seed = %target.seed,
        target_domain = handoff.target_domain,
        "relay: export-provenance ok (handoff delivered)"
    );
    let h = app.inner.read().await.chain.tip_h();
    push_relay_flow(
        app,
        h,
        "relayed:export_provenance",
        handoff.export_id.clone(),
        Some(handoff.export_id.clone()),
        Some(handoff.intent_id.clone()),
        format!("peer relay delivered handoff to {}", target.seed),
    )
    .await;
    Ok(())
}

pub(crate) async fn relay_import(app: &App, tx: &SignedTx) -> Result<(), RelayErr> {
    let target_hi = target_hi_for_import(tx).ok_or_else(|| {
        relay_err(
            StatusCode::BAD_REQUEST,
            "peer relay import requires import tx body",
        )
    })?;
    let export_hex = match tx.body {
        TxBody::Import { export_id, .. } => Some(hex::encode(export_id)),
        _ => None,
    };
    let rel = RelayTrace {
        op: "import",
        intent_id: None,
        export_id: export_hex.as_deref(),
        target_hi,
    };
    let target = select_target(app, rel).await?;
    let client = reqwest::Client::new();
    let url = format!("{}/v1/tx", target.base);
    info!(
        op = rel.op,
        export_id = rel.export_id.unwrap_or("-"),
        seed = %target.seed,
        url = %url,
        "relay: POST /v1/tx (import)"
    );
    let resp = client.post(&url).json(tx).send().await.map_err(|e| {
        warn!(
            op = rel.op,
            export_id = rel.export_id.unwrap_or("-"),
            seed = %target.seed,
            err = %e,
            "relay: import request failed"
        );
        relay_err(
            StatusCode::SERVICE_UNAVAILABLE,
            format!(
                "peer relay import failed seed={}: {e} export_id={}",
                target.seed,
                export_hex.as_deref().unwrap_or("-")
            ),
        )
    })?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        let snip = http_body_log_snippet(&body, 200);
        warn!(
            op = rel.op,
            export_id = rel.export_id.unwrap_or("-"),
            seed = %target.seed,
            http_status = %status,
            body_snippet = %snip,
            "relay: import HTTP error"
        );
        return Err(relay_err(
            StatusCode::BAD_GATEWAY,
            format!(
                "peer relay import rejected seed={} status={status}: {} export_id={}",
                target.seed,
                snip,
                export_hex.as_deref().unwrap_or("-")
            ),
        ));
    }
    let export_key = match &tx.body {
        TxBody::Import { export_id, .. } => *export_id,
        _ => unreachable!("relay_import only accepts Import body"),
    };
    info!(
        op = rel.op,
        export_id = %hex::encode(export_key),
        seed = %target.seed,
        "relay: import delivered"
    );
    let h = app.inner.read().await.chain.tip_h();
    push_relay_flow(
        app,
        h,
        "relayed:import",
        hex::encode(tx.tx_hash()),
        Some(hex::encode(export_key)),
        None,
        format!("peer relay delivered import to {}", target.seed),
    )
    .await;

    // Mirror relay progress only; trusted cross-shard facts confirm final import.
    {
        let mut g = app.inner.write().await;
        let bak = take_bak(&g);
        g.roaming_pool.mark_relayed_by_export(export_key);
        let h2 = g.chain.tip_h();
        let export_hex = hex::encode(export_key);
        g.push_flow(FlowTraceRow {
            at_height: h2,
            kind: "roaming_status:relayed".to_string(),
            tx_id: export_hex.clone(),
            export_id: Some(export_hex.clone()),
            intent_id: Some(export_hex),
            note: Some("source roaming marked relayed after relay_import delivered".into()),
        });
        let save_pair = crate::snapshot::SnapshotBackend::from_data_file(app.data_file.as_ref())
            .map(|b| {
                let path = b.init_state_path();
                let res = b.save_tip_summary(&g);
                (path, res)
            });
        drop(g);
        if let Some((path, res)) = save_pair {
            match res {
                Ok(()) => {
                    let mut st = app.init.write().await;
                    *st = InitState::ready(path);
                }
                Err(e) => {
                    warn!(
                        path = %path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "-".into()),
                        err = %e,
                        "relay_import: snapshot save failed after source roaming mark; rolling back"
                    );
                    let mut g = app.inner.write().await;
                    rollback_commit(&mut g, bak);
                    drop(g);
                    let mut st = app.init.write().await;
                    *st = InitState::ready_degraded(path, e);
                }
            }
        }
    }
    Ok(())
}

pub(crate) async fn log_relay_absence(
    app: &App,
    context: &str,
    err: &RelayErr,
    export_id: Option<&str>,
    intent_id: Option<&str>,
) {
    error!(
        context = context,
        export_id = export_id.unwrap_or("-"),
        intent_id = intent_id.unwrap_or("-"),
        "peer relay {} failed: {}",
        context,
        err.message
    );
    let h = app.inner.read().await.chain.tip_h();
    let tx_id = export_id.unwrap_or("-").to_string();
    push_relay_flow(
        app,
        h,
        "relay_absent:peer",
        tx_id,
        export_id.map(|s| s.to_string()),
        intent_id.map(|s| s.to_string()),
        format!("peer relay {context}: {}", err.message),
    )
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::TransportConfig;
    use pwm_core::dev_net;

    /// Without explicit seeds infer HTTP relay port as peer TCP−100 (formerly `relay_http_bases_derives_listen_port_from_peer_tcp_minus_100`).
    #[test]
    fn relay_http_peer_tcp_m100() {
        let mut cfg = TransportConfig::default();
        cfg.peer_seeds
            .push(SocketAddr::from(([127, 0, 0, 1], 3131)));
        let bases = relay_http_bases(&cfg);
        assert_eq!(bases, vec![SocketAddr::from(([127, 0, 0, 1], 3031))]);
    }

    /// Explicit `relay_http_seeds` wins over derived peer addresses (formerly `relay_http_bases_prefers_explicit_list`).
    #[test]
    fn relay_http_explicit_list() {
        let mut cfg = TransportConfig::default();
        cfg.peer_seeds
            .push(SocketAddr::from(([127, 0, 0, 1], 3131)));
        cfg.relay_http_seeds
            .push(SocketAddr::from(([127, 0, 0, 1], 9999)));
        let bases = relay_http_bases(&cfg);
        assert_eq!(bases, vec![SocketAddr::from(([127, 0, 0, 1], 9999))]);
    }

    #[test]
    fn foreign_import_detects_target_domain() {
        let (cfg, sks) = dev_net();
        let to = cfg.accounts[0].acct;
        let dom = domain_of_account_id(&to);
        let tx = SignedTx::sign_body(
            &sks[0],
            dom,
            cfg.accounts[0].der_idx,
            1,
            TxBody::Import {
                to,
                amount: 1,
                export_id: [7u8; 32],
            },
        );
        let local_hi = dom.to_be_bytes()[0].wrapping_add(1);
        assert!(is_foreign_import(&tx, local_hi));
        assert!(!is_foreign_import(&tx, dom.to_be_bytes()[0]));
    }
}
