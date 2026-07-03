//! Handlers for offchain batch Merkle roots and inclusion proofs.

use super::common::ensure_ready;
use super::types::{OffchainBatchOut, OffchainEntryIn, OffchainProofOut};
use crate::offchain::{entry_leaf, merkle_proof, parse_entry, verify_proof};
use crate::App;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;

const MAX_BATCH_ENTRIES: usize = 4096;

pub(super) async fn v1_off_batch(
    State(app): State<App>,
    Json(entries): Json<Vec<OffchainEntryIn>>,
) -> Result<Json<OffchainBatchOut>, (StatusCode, String)> {
    ensure_ready(&app).await?;
    if entries.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "empty offchain batch".into()));
    }
    if entries.len() > MAX_BATCH_ENTRIES {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("offchain batch exceeds {MAX_BATCH_ENTRIES} entries"),
        ));
    }
    let parsed = entries
        .iter()
        .map(|row| parse_entry(&row.account_id, &row.amount, row.nonce))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| (StatusCode::BAD_REQUEST, err))?;
    let (tip_hash, tip_height) = {
        let guard = app.inner.read().await;
        (guard.chain.tip_hash(), guard.chain.tip_h())
    };
    let record = app.offchain.insert(parsed, tip_hash, tip_height);
    Ok(Json(batch_out(&record)))
}

pub(super) async fn v1_off_batch_get(
    State(app): State<App>,
    Path(batch_id): Path<u64>,
) -> Result<Json<OffchainBatchOut>, (StatusCode, String)> {
    let record = app
        .offchain
        .get(batch_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "offchain batch not found".into()))?;
    Ok(Json(batch_out(&record)))
}

pub(super) async fn v1_off_proof(
    State(app): State<App>,
    Path((batch_id, entry_index)): Path<(u64, usize)>,
) -> Result<Json<OffchainProofOut>, (StatusCode, String)> {
    let record = app
        .offchain
        .get(batch_id)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "offchain batch not found".into()))?;
    let entry = record
        .entries
        .get(entry_index)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "offchain entry not found".into()))?;
    let proof = merkle_proof(&record, entry_index)
        .ok_or_else(|| (StatusCode::NOT_FOUND, "offchain entry not found".into()))?;
    let leaf = entry_leaf(entry);
    if !verify_proof(leaf, &proof, record.root) {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "offchain proof verification failed".into(),
        ));
    }
    Ok(Json(OffchainProofOut {
        batch_id,
        entry_index,
        leaf_hash: hex::encode(leaf),
        merkle_root: hex::encode(record.root),
        anchor_tx_hash: hex::encode(record.anchor_tx_hash),
        proof,
    }))
}

fn batch_out(record: &crate::offchain::BatchRecord) -> OffchainBatchOut {
    OffchainBatchOut {
        batch_id: record.batch_id,
        merkle_root: hex::encode(record.root),
        entry_count: record.entries.len() as u64,
        anchor_tx_hash: hex::encode(record.anchor_tx_hash),
    }
}
