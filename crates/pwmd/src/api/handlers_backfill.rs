//! Cross-shard fact discovery + operator-triggered import backfill.

use super::common::{
    ensure_bridge_federation_ok, ensure_operator_auth, ensure_ready, ensure_user_tx_allowed, hex32,
};
use super::handlers_tx;
use super::types::{
    BackfillIn, BackfillOut, CrossShardFactOut, CrossShardFactsOut, CrossShardFactsQuery,
};
use crate::config::TransportConfig;
use crate::relay;
use crate::tx_policy::DUPLICATE_IMPORT_ERR_TEXT;
use crate::App;
use axum::{
    extract::{ConnectInfo, Query, State},
    http::{HeaderMap, StatusCode},
    Json,
};
use pwm_core::hd::domain_of_account_id;
use pwm_core::state::ExportProvenance;
use pwm_core::tx::{SignedTx, TxBody};
use serde::Deserialize;
use std::{net::SocketAddr, time::Duration};
use tracing::info;
use url::{Host, Url};

const FACTS_DEFAULT_LIMIT: usize = 256;
const BACKFILL_DEFAULT_LIMIT: usize = 256;

#[derive(Debug, Deserialize)]
struct PeerStatus {
    #[serde(default)]
    ready: bool,
    #[serde(default)]
    network_id: Option<String>,
    #[serde(default)]
    effective_genesis_hash: Option<String>,
    #[serde(default)]
    genesis_guard: Option<String>,
}

#[derive(Clone)]
struct BackfillSigner {
    sk: ed25519_dalek::SigningKey,
    sender: [u8; 32],
    der_idx: u32,
    domain: u16,
}

pub(super) async fn v1_cross_shard_facts(
    State(a): State<App>,
    Query(q): Query<CrossShardFactsQuery>,
) -> Result<Json<CrossShardFactsOut>, (StatusCode, String)> {
    ensure_ready(&a).await?;
    let from_height = q.from_height.unwrap_or(0);
    let limit = q.limit.unwrap_or(FACTS_DEFAULT_LIMIT).clamp(1, 4096);
    let facts = {
        let g = a.inner.read().await;
        g.cross_shard
            .facts_for_target(q.target_domain_hi, from_height, limit)
    };
    let facts = facts
        .into_iter()
        .map(|fact| CrossShardFactOut {
            export_id: hex::encode(fact.export_id),
            source_domain_hi: fact.source_domain_hi,
            target_domain_hi: fact.target_domain_hi,
            amount: fact.amount,
            status: fact.status,
            first_height: fact.first_height,
            last_height: fact.last_height,
            source: fact.source.map(|x| hex32(&x)),
            to: hex32(&fact.to),
            intent_id: fact.intent_id.map(|x| hex32(&x)),
        })
        .collect::<Vec<_>>();
    Ok(Json(CrossShardFactsOut {
        target_domain_hi: q.target_domain_hi,
        from_height,
        limit,
        total: facts.len(),
        facts,
    }))
}

pub(super) async fn v1_cross_shard_backfill(
    State(a): State<App>,
    conn: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(input): Json<BackfillIn>,
) -> Result<Json<BackfillOut>, (StatusCode, String)> {
    ensure_operator_auth(&a, conn.map(|x| x.0), &headers)?;
    // Backfill must obey the same tx admission gate as `/v1/tx`.
    // This prevents any side-effects on degraded/genesis-blocked nodes.
    ensure_user_tx_allowed(&a).await?;
    ensure_bridge_federation_ok(&a).await?;
    let from_height = input.from_height.unwrap_or(0);
    let limit = input.limit.unwrap_or(BACKFILL_DEFAULT_LIMIT).clamp(1, 4096);
    let signer = pick_backfill_signer(&a).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("backfill signer selection failed: {e}"),
        )
    })?;
    let local_hi = signer.domain.to_be_bytes()[0];
    let mut out = BackfillOut {
        peer: None,
        discovered: 0,
        imported: 0,
        skipped_existing: 0,
        rejected: 0,
        untrusted: 0,
        details: Vec::new(),
    };
    let cfg = a.transport_config.read().await.clone();
    let mut peers = backfill_peers(input.peer_base, &cfg)?;
    if peers.is_empty() {
        return Ok(Json(out));
    }
    let expected_network_id = a.identity.network_id.clone();
    let expected_genesis_hash = {
        let hs = crate::transport::handshake_read_traced(&a, "api_backfill").await;
        crate::transport::score_sort(&mut peers, &hs.peer_scores);
        hs.validation_ctx.expected_genesis_hash.clone()
    };
    let client = backfill_client(&cfg)?;
    let fetch_ctx = PeerFetchCtx {
        client: &client,
        network_id: &expected_network_id,
        genesis_hash: expected_genesis_hash.as_deref(),
        local_hi,
        from_height,
        limit,
    };
    let Some((peer, facts)) = select_peer_facts(fetch_ctx, peers, &mut out).await else {
        return Ok(Json(out));
    };
    out.peer = Some(peer.clone());
    out.discovered = facts.len() as u64;

    import_backfill_facts(&a, &signer, facts, local_hi, &mut out).await;
    info!(
        peer = %peer,
        discovered = out.discovered,
        imported = out.imported,
        skipped_existing = out.skipped_existing,
        rejected = out.rejected,
        untrusted = out.untrusted,
        "cross-shard backfill finished"
    );
    Ok(Json(out))
}

struct PeerFetchCtx<'a> {
    client: &'a reqwest::Client,
    network_id: &'a str,
    genesis_hash: Option<&'a str>,
    local_hi: u8,
    from_height: u64,
    limit: usize,
}

struct ImportTxCtx<'a> {
    app: &'a App,
    signer: &'a BackfillSigner,
    fact: &'a CrossShardFactOut,
    export_id: [u8; 32],
    to: [u8; 32],
}

fn backfill_peers(
    peer_base: Option<String>,
    cfg: &TransportConfig,
) -> Result<Vec<String>, (StatusCode, String)> {
    let relay_bases = relay::relay_http_bases(cfg);
    let mut peers = if let Some(base) = peer_base {
        vec![peer_base_from_input(&base, &relay_bases)?]
    } else {
        relay_bases
            .into_iter()
            .map(|x| format!("http://{x}"))
            .collect()
    };
    peers.sort();
    peers.dedup();
    Ok(peers)
}

fn backfill_client(cfg: &TransportConfig) -> Result<reqwest::Client, (StatusCode, String)> {
    reqwest::Client::builder()
        .timeout(Duration::from_millis(
            cfg.connect_timeout_ms
                .saturating_add(cfg.handshake_timeout_ms)
                .max(500),
        ))
        .build()
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("backfill client init failed: {e}"),
            )
        })
}

async fn select_peer_facts(
    ctx: PeerFetchCtx<'_>,
    peers: Vec<String>,
    out: &mut BackfillOut,
) -> Option<(String, Vec<CrossShardFactOut>)> {
    for base in peers {
        match trust_peer_status(ctx.client, &base, ctx.network_id, ctx.genesis_hash).await {
            Ok(()) => {
                match fetch_peer_facts(ctx.client, &base, ctx.local_hi, ctx.from_height, ctx.limit)
                    .await
                {
                    Ok(rows) => return Some((base, rows)),
                    Err(err) => {
                        out.rejected = out.rejected.saturating_add(1);
                        out.details.push(err);
                    }
                }
            }
            Err(err) => {
                out.untrusted = out.untrusted.saturating_add(1);
                out.details.push(err);
            }
        }
    }
    None
}

async fn import_backfill_facts(
    app: &App,
    signer: &BackfillSigner,
    facts: Vec<CrossShardFactOut>,
    local_hi: u8,
    out: &mut BackfillOut,
) {
    for fact in facts {
        let Ok(export_id) = decode_id(&fact.export_id) else {
            out.rejected = out.rejected.saturating_add(1);
            continue;
        };
        let Ok(to) = decode_id(&fact.to) else {
            out.rejected = out.rejected.saturating_add(1);
            continue;
        };
        if fact.target_domain_hi != local_hi {
            out.rejected = out.rejected.saturating_add(1);
            continue;
        }
        let Some(source) = fact.source.as_deref().and_then(|x| decode_id(x).ok()) else {
            out.rejected = out.rejected.saturating_add(1);
            continue;
        };

        record_backfill_fact(app, &fact, export_id, source, to).await;
        let ctx = ImportTxCtx {
            app,
            signer,
            fact: &fact,
            export_id,
            to,
        };
        submit_import_tx(ctx, out).await;
    }
}

async fn record_backfill_fact(
    app: &App,
    fact: &CrossShardFactOut,
    export_id: [u8; 32],
    source: [u8; 32],
    to: [u8; 32],
) {
    let mut g = app.inner.write().await;
    let h = g.chain.tip_h();
    g.cross_shard.record_handoff(
        export_id,
        fact.source_domain_hi,
        source,
        to,
        ((fact.target_domain_hi as u16) << 8) | 1,
        fact.amount,
        h,
        None,
    );
}

async fn submit_import_tx(ctx: ImportTxCtx<'_>, out: &mut BackfillOut) {
    let (tx, tx_domain) =
        match mk_import_tx(ctx.app, ctx.signer, ctx.export_id, ctx.to, ctx.fact.amount).await {
            Ok(tx) => tx,
            Err(err) => {
                out.rejected = out.rejected.saturating_add(1);
                out.details.push(err);
                return;
            }
        };
    match handlers_tx::v1_tx(State(ctx.app.clone()), Json(tx)).await {
        Ok(_) => {
            out.imported = out.imported.saturating_add(1);
        }
        Err((StatusCode::CONFLICT, msg))
            if msg.contains(DUPLICATE_IMPORT_ERR_TEXT) || msg.contains("already consumed") =>
        {
            out.skipped_existing = out.skipped_existing.saturating_add(1);
        }
        Err((_, msg)) => {
            out.rejected = out.rejected.saturating_add(1);
            out.details.push(format!(
                "{msg}; export_id={}; tx_domain=0x{tx_domain:04X}; target_hi=0x{:02X}; to={}",
                ctx.fact.export_id, ctx.fact.target_domain_hi, ctx.fact.to
            ));
        }
    }
}

fn peer_base_from_input(raw: &str, allowed: &[SocketAddr]) -> Result<String, (StatusCode, String)> {
    let url = Url::parse(raw.trim()).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            format!("invalid peer_base URL: {e}"),
        )
    })?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(bad_peer_base("peer_base scheme must be http or https"));
    }
    if url.path() != "/" || url.query().is_some() || url.fragment().is_some() {
        return Err(bad_peer_base("peer_base must be a bare peer origin URL"));
    }
    let ip = peer_base_ip(&url)?;
    if ip.is_loopback() {
        return Err(bad_peer_base("peer_base loopback host is not allowed"));
    }
    let Some(port) = url.port_or_known_default() else {
        return Err(bad_peer_base("peer_base must include a port"));
    };
    let addr = SocketAddr::new(ip, port);
    if !allowed.contains(&addr) {
        return Err(bad_peer_base(
            "peer_base is not in the configured relay peer set",
        ));
    }
    Ok(format!("{}://{}", url.scheme(), addr))
}

fn peer_base_ip(url: &Url) -> Result<std::net::IpAddr, (StatusCode, String)> {
    match url.host() {
        Some(Host::Ipv4(ip)) => Ok(ip.into()),
        Some(Host::Ipv6(ip)) => Ok(ip.into()),
        Some(Host::Domain(host)) if host.eq_ignore_ascii_case("localhost") => {
            Err(bad_peer_base("peer_base loopback host is not allowed"))
        }
        Some(Host::Domain(_)) => Err(bad_peer_base(
            "peer_base host must be a configured IP relay seed",
        )),
        None => Err(bad_peer_base("peer_base must include a host")),
    }
}

fn bad_peer_base(msg: &str) -> (StatusCode, String) {
    (StatusCode::BAD_REQUEST, msg.to_string())
}

async fn fetch_peer_facts(
    client: &reqwest::Client,
    base: &str,
    target_domain_hi: u8,
    from_height: u64,
    limit: usize,
) -> Result<Vec<CrossShardFactOut>, String> {
    let url = format!(
        "{base}/v1/cross-shard/facts?target_domain_hi={target_domain_hi}&from_height={from_height}&limit={limit}"
    );
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("peer facts fetch failed {base}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "peer facts fetch HTTP {} from {base}",
            resp.status()
        ));
    }
    let out = resp
        .json::<CrossShardFactsOut>()
        .await
        .map_err(|e| format!("peer facts decode failed {base}: {e}"))?;
    Ok(out.facts)
}

async fn trust_peer_status(
    client: &reqwest::Client,
    base: &str,
    expected_network_id: &str,
    expected_genesis_hash: Option<&str>,
) -> Result<(), String> {
    let url = format!("{base}/v1/status");
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("peer status unavailable {base}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("peer status HTTP {} from {base}", resp.status()));
    }
    let status = resp
        .json::<PeerStatus>()
        .await
        .map_err(|e| format!("peer status decode failed {base}: {e}"))?;
    if !status.ready {
        return Err(format!("peer {base} not ready"));
    }
    if status.genesis_guard.as_deref().unwrap_or("ok") != "ok" {
        return Err(format!("peer {base} genesis_guard blocked"));
    }
    if status.network_id.as_deref() != Some(expected_network_id) {
        return Err(format!(
            "peer {base} network mismatch {:?} != {expected_network_id}",
            status.network_id
        ));
    }
    if let Some(expected) = expected_genesis_hash {
        if status.effective_genesis_hash.as_deref() != Some(expected) {
            return Err(format!(
                "peer {base} genesis mismatch {:?} != {expected}",
                status.effective_genesis_hash
            ));
        }
    }
    Ok(())
}

async fn mk_import_tx(
    app: &App,
    signer: &BackfillSigner,
    export_id: [u8; 32],
    to: [u8; 32],
    amount: u128,
) -> Result<(SignedTx, u16), String> {
    let nonce = {
        let g = app.inner.read().await;
        g.chain
            .st
            .get(&signer.sender)
            .map(|x| x.nonce)
            .ok_or_else(|| "backfill signer state account missing".to_string())?
    };
    let mut tx = SignedTx::sign_body(
        &signer.sk,
        signer.domain,
        signer.der_idx,
        nonce,
        TxBody::Import {
            to,
            amount,
            export_id,
        },
    );
    tx.set_import_provenance_signed(
        &signer.sk,
        Some(ExportProvenance {
            to,
            target_domain: signer.domain,
            amount,
        }),
    );
    Ok((tx, signer.domain))
}

fn decode_id(value: &str) -> Result<[u8; 32], ()> {
    let raw = hex::decode(value.trim()).map_err(|_| ())?;
    if raw.len() != 32 {
        return Err(());
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&raw);
    Ok(out)
}

async fn pick_backfill_signer(app: &App) -> Result<BackfillSigner, String> {
    let g = app.inner.read().await;
    let local_hi = app.identity.cluster_domain_hi;
    let pick = g
        .chain
        .cfg
        .accounts
        .iter()
        .enumerate()
        .find(|(_, row)| domain_of_account_id(&row.acct).to_be_bytes()[0] == local_hi)
        .or_else(|| g.chain.cfg.accounts.iter().enumerate().next())
        .ok_or_else(|| "backfill signer account missing".to_string())?;
    let sk = g
        .chain
        .val_sks
        .get(pick.0)
        .or_else(|| g.chain.val_sks.first())
        .ok_or_else(|| "backfill signer key missing".to_string())?
        .clone();
    Ok(BackfillSigner {
        sk,
        sender: pick.1.acct,
        der_idx: pick.1.der_idx,
        domain: domain_of_account_id(&pick.1.acct),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    fn allowed_set() -> Vec<SocketAddr> {
        vec![SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)), 8080)]
    }

    #[test]
    fn valid_bare_ip_origin_accepted() {
        let res = peer_base_from_input("http://1.2.3.4:8080/", &allowed_set());
        assert!(res.is_ok(), "expected ok, got {res:?}");
        assert_eq!(res.unwrap(), "http://1.2.3.4:8080");
    }

    #[test]
    fn domain_host_rejected() {
        let res = peer_base_from_input("http://example.com:8080/", &allowed_set());
        assert!(res.is_err());
    }

    #[test]
    fn loopback_ip_rejected() {
        let res = peer_base_from_input("http://127.0.0.1:8080/", &allowed_set());
        assert!(res.is_err());
    }

    #[test]
    fn localhost_name_rejected() {
        let res = peer_base_from_input("http://localhost:8080/", &allowed_set());
        assert!(res.is_err());
    }

    #[test]
    fn ip_not_in_allowed_set_rejected() {
        let res = peer_base_from_input("http://9.9.9.9:8080/", &allowed_set());
        assert!(res.is_err());
    }

    #[test]
    fn path_with_segment_rejected() {
        let allowed = vec![SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)), 8080)];
        let res = peer_base_from_input("http://1.2.3.4:8080/api", &allowed);
        assert!(res.is_err());
    }

    #[test]
    fn query_string_rejected() {
        let allowed = vec![SocketAddr::new(IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4)), 8080)];
        let res = peer_base_from_input("http://1.2.3.4:8080/?x=1", &allowed);
        assert!(res.is_err());
    }
}
