//! Cross-shard fact discovery + operator-triggered import backfill.

use super::common::{ensure_bridge_federation_ok, ensure_ready, ensure_user_tx_allowed, hex32};
use super::handlers_tx;
use super::types::{
    BackfillIn, BackfillOut, CrossShardFactOut, CrossShardFactsOut, CrossShardFactsQuery,
};
use crate::relay;
use crate::tx_policy::DUPLICATE_IMPORT_ERR_TEXT;
use crate::App;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use pwm_core::hd::domain_of_account_id;
use pwm_core::state::ExportProvenance;
use pwm_core::tx::{SignedTx, TxBody};
use serde::Deserialize;
use std::time::Duration;
use tracing::info;

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
    Json(input): Json<BackfillIn>,
) -> Result<Json<BackfillOut>, (StatusCode, String)> {
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
    let mut peers = if let Some(peer_base) = input.peer_base {
        vec![peer_base]
    } else {
        relay::relay_http_bases(&cfg)
            .into_iter()
            .map(|x| format!("http://{x}"))
            .collect()
    };
    if peers.is_empty() {
        return Ok(Json(out));
    }
    peers.sort();
    peers.dedup();
    let expected_network_id = a.identity.network_id.clone();
    let expected_genesis_hash = {
        let hs = a.handshake.read().await;
        hs.validation_ctx.expected_genesis_hash.clone()
    };
    let client = reqwest::Client::builder()
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
        })?;

    let mut selected_peer: Option<String> = None;
    let mut facts: Vec<CrossShardFactOut> = Vec::new();
    for base in peers {
        match trust_peer_status(
            &client,
            &base,
            &expected_network_id,
            expected_genesis_hash.as_deref(),
        )
        .await
        {
            Ok(()) => match fetch_peer_facts(&client, &base, local_hi, from_height, limit).await {
                Ok(rows) => {
                    selected_peer = Some(base);
                    facts = rows;
                    break;
                }
                Err(err) => {
                    out.rejected = out.rejected.saturating_add(1);
                    out.details.push(err);
                }
            },
            Err(err) => {
                out.untrusted = out.untrusted.saturating_add(1);
                out.details.push(err);
            }
        }
    }
    let Some(peer) = selected_peer else {
        return Ok(Json(out));
    };
    out.peer = Some(peer.clone());
    out.discovered = facts.len() as u64;

    for fact in facts.into_iter() {
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

        {
            let mut g = a.inner.write().await;
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

        let (tx, tx_domain) = match mk_import_tx(&a, &signer, export_id, to, fact.amount).await {
            Ok(tx) => tx,
            Err(err) => {
                out.rejected = out.rejected.saturating_add(1);
                out.details.push(err);
                continue;
            }
        };
        match handlers_tx::v1_tx(State(a.clone()), Json(tx)).await {
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
                    fact.export_id, fact.target_domain_hi, fact.to
                ));
            }
        }
    }
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
