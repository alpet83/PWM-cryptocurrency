//! Snapshot filesystem load/save, migration helpers, and cfg validation.
//! Primary-load wall times are recorded in [`load_snapshot_timed`] for `pwmd::startup::snapshot`.

use super::anchor;
use super::epoch::{ensure_epoch_man_schema, manifest_file_path};
use super::genesis::snapshot_genesis_accounts;
use super::incremental;
use super::telemetry::{JsonSnapTiming, SNAP_STARTUP_TARGET};
use super::types::{
    data_from_v2, data_from_v3, data_from_v4, data_to_v4, roaming_to_wire, BlocksStored,
    SnapshotData, SnapshotDataLegacyV0, SnapshotDataV2, SnapshotDataV3, SnapshotDataV4,
    SnapshotRoamingWire, SNAPSHOT_V1, SNAPSHOT_V2, SNAPSHOT_V3, SNAPSHOT_VERSION,
};
use crate::ledger::CrossShardLedger;
use crate::Inner;
use ed25519_dalek::SigningKey;
use pwm_core::block::Block;
use pwm_core::block::{hdr_hash, txs_root};
use pwm_core::chain::{pick_prod_idx, prev_gen, recompute_active_idxs, roll_epoch_if_needed};
use pwm_core::compute_block_reward;
use pwm_core::digest;
use pwm_core::genesis::GenCfg;
use pwm_core::hd::account_id_from_parts;
use pwm_core::tx::TxBody;
use pwm_core::Chain;
use pwm_core::TAIL_BLOCK_CAP;
use serde_json::Value;
use std::path::Path as FsPath;
use std::time::Instant;
use tracing::{info, warn};

/// JsonFile epoch load: full replay vs trust checkpoint + tail-only blocks (see `validate_snapshot_trusted`).
#[derive(Clone, Debug)]
pub(crate) struct SnapshotLoadOpts {
    pub verify_chain: bool,
    pub anchor_sk: Option<SigningKey>,
    pub anchor_idx: u32,
}

impl SnapshotLoadOpts {
    pub(crate) fn verify_full() -> Self {
        Self {
            verify_chain: true,
            anchor_sk: None,
            anchor_idx: 0,
        }
    }
}

fn validate_snapshot_state_accounts(snapshot: &SnapshotData) -> Result<(), String> {
    for (id, account) in snapshot.state.accounts.iter() {
        if !account.initialized {
            continue;
        }
        let derived = account_id_from_parts(&account.signing_pubkey, account.derivation_index);
        if derived != *id {
            return Err(format!(
                "snapshot state mismatch: account id {} does not match signing_pubkey/derivation_index",
                hex::encode(id)
            ));
        }
    }
    Ok(())
}

fn block_tx_kind(blk: &Block) -> &'static str {
    if blk.txs.is_empty() {
        return "empty";
    }
    let mut has_export = false;
    let mut has_import = false;
    let mut has_other = false;
    for tx in &blk.txs {
        match tx.body {
            TxBody::Export { .. } => has_export = true,
            TxBody::Import { .. } => has_import = true,
            _ => has_other = true,
        }
    }
    if has_export && has_import {
        "mixed"
    } else if has_export && !has_other {
        "Export"
    } else if has_import && !has_other {
        "Import"
    } else if has_export || has_import {
        "mixed"
    } else {
        "other"
    }
}

fn mismatch_class(snapshot: &SnapshotData, tx_kind: &str) -> &'static str {
    if snapshot.blocks_stored == BlocksStored::Epochs
        && snapshot.checkpoint_height > 0
        && snapshot.checkpoint_height != snapshot.blocks.len() as u64
    {
        "manifest_summary_drift"
    } else if tx_kind == "other" {
        "other"
    } else {
        "state_root_divergence"
    }
}

fn find_blk1(snapshot: &SnapshotData, summary_path: &FsPath) -> Result<Option<Block>, String> {
    if let Some(blk) = snapshot.blocks.iter().find(|b| b.hdr.height == 1) {
        return Ok(Some(blk.clone()));
    }
    incremental::load_block_at_height(summary_path, 1)
}

fn preflight_blk1(
    snapshot: &SnapshotData,
    cfg: &GenCfg,
    summary_path: &FsPath,
) -> Result<[u8; 32], String> {
    if snapshot.checkpoint_height == 0 {
        return Ok([0u8; 32]);
    }
    let blk = find_blk1(snapshot, summary_path)?.ok_or_else(|| {
        "snapshot trust validation: missing genesis anchor block1 (pruned)".to_string()
    })?;
    if blk.hdr.height != 1 {
        return Err("snapshot trust validation: genesis preflight expects block height 1".into());
    }
    if blk.hdr.prev_hash != prev_gen() {
        return Err("snapshot trust validation: block1 prev_hash mismatch with genesis".into());
    }
    let want_tx_root = txs_root(&blk.txs);
    if blk.hdr.tx_root != want_tx_root {
        return Err("snapshot trust validation: block1 tx_root is invalid".into());
    }
    let prod = cfg
        .vals
        .set
        .get(blk.hdr.prod_idx as usize)
        .ok_or_else(|| "snapshot trust validation: block1 prod_idx out of range".to_string())?;
    if !blk.hdr.verify_sig(&prod.pubkey) {
        return Err("snapshot trust validation: block1 has invalid producer signature".into());
    }
    let mut st = cfg.state0();
    st.active_validator_indices = recompute_active_idxs(cfg, &st);
    roll_epoch_if_needed(cfg, &mut st, 1);
    st.refund_exp_locks(1);
    for (tx_i, tx) in blk.txs.iter().enumerate() {
        st.apply_tx_with_ctx(tx, 1, blk.hdr.ts, cfg).map_err(|e| {
            format!("snapshot trust validation: block1 tx[{tx_i}] replay failed: {e}")
        })?;
    }
    st.refund_exp_locks(1);
    st.drain_conservation_at_height(1, cfg);
    let prod_acct = cfg.prod_acct(blk.hdr.prod_idx);
    if cfg.is_legacy_policy() {
        st.reward_producer(&prod_acct, cfg.block_reward);
    } else {
        let rew = compute_block_reward(cfg, 1);
        st.reward_producer_v2(&prod_acct, rew, cfg.pwm_stake_min, 1_000_000);
    }
    let st_root = digest(&st);
    if blk.hdr.state_root != st_root {
        return Err("snapshot trust validation: block1 state_root mismatch after replay".into());
    }
    Ok(hdr_hash(&blk.hdr))
}

fn attach_anch(
    snapshot: &mut SnapshotData,
    cfg: &GenCfg,
    summary_path: &FsPath,
    opts: &SnapshotLoadOpts,
) -> Result<(), String> {
    if snapshot.genesis_anchor.is_some() {
        return Ok(());
    }
    let blk1_hash = preflight_blk1(snapshot, cfg, summary_path)?;
    let Some(sk) = opts.anchor_sk.as_ref() else {
        return Err(
            "snapshot trust validation: legacy snapshot missing genesis_anchor and signer key unavailable; rerun with --snapshot-verify-chain or set PWM_SNAPSHOT_ANCHOR_MIGRATE=1 for temporary bypass"
                .to_string(),
        );
    };
    let anch = anchor::mk_anch(cfg, blk1_hash, opts.anchor_idx, sk)?;
    snapshot.genesis_anchor = Some(anch);
    warn!("snapshot genesis_anchor migrated at load");
    Ok(())
}

fn allow_legacy_env() -> bool {
    match std::env::var("PWM_SNAPSHOT_ANCHOR_MIGRATE") {
        Ok(v) => {
            let v = v.trim().to_ascii_lowercase();
            !(v.is_empty() || v == "0" || v == "false" || v == "no" || v == "off")
        }
        Err(_) => false,
    }
}

fn snap_blk1(path: &FsPath, inner: &Inner, tip: u64) -> Result<[u8; 32], String> {
    if tip == 0 {
        return Ok([0u8; 32]);
    }
    if let Some(blk) = inner.chain.blocks.iter().find(|b| b.hdr.height == 1) {
        return Ok(hdr_hash(&blk.hdr));
    }
    let blk = incremental::load_block_at_height(path, 1)?
        .ok_or_else(|| "snapshot persist: missing genesis anchor block1 (pruned)".to_string())?;
    Ok(hdr_hash(&blk.hdr))
}

fn fill_anch(path: &FsPath, inner: &Inner, snap: &mut SnapshotData) -> Result<(), String> {
    let tip = snap.checkpoint_height;
    let blk1_hash = snap_blk1(path, inner, tip)?;
    let sk = inner.chain.val_sks.first().ok_or_else(|| {
        "snapshot persist: missing validator signing key for genesis_anchor".to_string()
    })?;
    let anch = anchor::mk_anch(&inner.chain.cfg, blk1_hash, 0, sk)?;
    snap.genesis_anchor = Some(anch);
    Ok(())
}

fn validate_snapshot(snapshot: &mut SnapshotData, cfg: &GenCfg) -> Result<(), String> {
    if cfg.vals.set.is_empty() {
        return Err("snapshot validation error: genesis config has zero validators".into());
    }
    if snapshot.version != SNAPSHOT_VERSION {
        return Err(format!(
            "snapshot version mismatch: got {}, expected {}",
            snapshot.version, SNAPSHOT_VERSION
        ));
    }
    if snapshot.blocks.len() > u64::MAX as usize {
        return Err(
            "snapshot chain mismatch: blocks length exceeds supported u64 height range".into(),
        );
    }
    let want = snapshot_genesis_accounts(cfg);
    if snapshot.genesis_accounts.len() != want.len() {
        return Err(format!(
            "snapshot genesis mismatch: rows {} != {}",
            snapshot.genesis_accounts.len(),
            want.len()
        ));
    }
    for (i, (got, exp)) in snapshot
        .genesis_accounts
        .iter()
        .zip(want.iter())
        .enumerate()
    {
        if got.pubkey != exp.pubkey || got.acct != exp.acct || got.der_idx != exp.der_idx {
            return Err(format!("snapshot genesis mismatch at row {i}"));
        }
    }
    if snapshot.blocks_stored == BlocksStored::Epochs {
        snapshot.state = cfg.state0();
    }
    let mut prev = prev_gen();
    let mut replay_state = cfg.state0();
    replay_state.active_validator_indices = recompute_active_idxs(cfg, &replay_state);
    let total_blocks = snapshot.blocks.len() as u64;
    let replay_start = Instant::now();
    let mut last_log = replay_start;
    info!(
        target: SNAP_STARTUP_TARGET,
        stage = "chain_verify",
        total_blocks,
        "chain_verify started"
    );
    for (i, blk) in snapshot.blocks.iter().enumerate() {
        let now = Instant::now();
        if now.duration_since(last_log).as_secs() >= 10 {
            let percent_complete = if total_blocks == 0 {
                100
            } else {
                ((i as u64) * 100) / total_blocks
            };
            info!(
                target: SNAP_STARTUP_TARGET,
                stage = "chain_verify",
                height = blk.hdr.height,
                total_blocks,
                percent_complete,
                elapsed_ms = replay_start.elapsed().as_millis() as u64,
                "chain_verify progress"
            );
            last_log = now;
        }
        let h = (i as u64) + 1;
        if blk.hdr.height != h {
            return Err(format!(
                "snapshot chain mismatch: block[{i}] has height {}, expected {h}",
                blk.hdr.height
            ));
        }
        if blk.hdr.prev_hash != prev {
            return Err(format!(
                "snapshot chain mismatch: block[{i}] prev_hash does not match previous header hash"
            ));
        }
        let want_tx_root = txs_root(&blk.txs);
        if blk.hdr.tx_root != want_tx_root {
            return Err(format!(
                "snapshot chain mismatch: block[{i}] tx_root is invalid"
            ));
        }
        roll_epoch_if_needed(cfg, &mut replay_state, h);
        let want_prod_idx =
            pick_prod_idx(h, &replay_state.active_validator_indices).map_err(|e| {
                format!("snapshot chain mismatch: block[{i}] cannot pick proposer: {e}")
            })?;
        if blk.hdr.prod_idx != want_prod_idx {
            return Err(format!(
                "snapshot chain mismatch: block[{i}] prod_idx {}, expected {}",
                blk.hdr.prod_idx, want_prod_idx
            ));
        }
        let prod =
            cfg.vals.set.get(blk.hdr.prod_idx as usize).ok_or_else(|| {
                format!("snapshot chain mismatch: block[{i}] prod_idx out of range")
            })?;
        if !blk.hdr.verify_sig(&prod.pubkey) {
            return Err(format!(
                "snapshot chain mismatch: block[{i}] has invalid producer signature"
            ));
        }
        replay_state.refund_exp_locks(h);
        for (tx_i, tx) in blk.txs.iter().enumerate() {
            replay_state
                .apply_tx_with_ctx(tx, blk.hdr.height, blk.hdr.ts, cfg)
                .map_err(|e| {
                    format!(
                    "snapshot chain mismatch: block[{i}] tx[{tx_i}] is invalid during replay: {e}"
                )
                })?;
        }
        replay_state.refund_exp_locks(h);
        replay_state.drain_conservation_at_height(h, cfg);
        let prod_acct = cfg.prod_acct(blk.hdr.prod_idx);
        if cfg.is_legacy_policy() {
            replay_state.reward_producer(&prod_acct, cfg.block_reward);
        } else {
            let rew = compute_block_reward(cfg, h);
            replay_state.reward_producer_v2(&prod_acct, rew, cfg.pwm_stake_min, 1_000_000);
        }
        let replay_root = digest(&replay_state);
        if blk.hdr.state_root != replay_root {
            let tx_kind = block_tx_kind(blk);
            let class = mismatch_class(snapshot, tx_kind);
            return Err(format!(
                "snapshot chain mismatch: first_bad_height={h} block_idx={i} block_hash={} tx_kind={} class={} header_root={} replay_root={}",
                hex::encode(hdr_hash(&blk.hdr)),
                tx_kind,
                class,
                hex::encode(blk.hdr.state_root),
                hex::encode(replay_root),
            ));
        }
        prev = hdr_hash(&blk.hdr);
    }
    if snapshot.blocks_stored == BlocksStored::Epochs {
        snapshot.state = replay_state.clone();
    }
    validate_snapshot_state_accounts(snapshot)?;
    if let Some(last) = snapshot.blocks.last() {
        if last.hdr.height != snapshot.blocks.len() as u64 {
            return Err(format!(
                "snapshot chain mismatch: tip height {} does not match blocks length {}",
                last.hdr.height,
                snapshot.blocks.len()
            ));
        }
        let st_root = digest(&snapshot.state);
        if last.hdr.state_root != st_root {
            return Err("snapshot state root mismatch with tip block".into());
        }
        let replay_root = digest(&replay_state);
        if replay_root != st_root {
            return Err(
                "snapshot state mismatch: persisted state does not match replayed chain state"
                    .into(),
            );
        }
    } else {
        let genesis_root = digest(&cfg.state0());
        let st_root = digest(&snapshot.state);
        if st_root != genesis_root {
            return Err(
                "snapshot state mismatch: empty block history must match genesis state".into(),
            );
        }
    }
    info!(
        target: SNAP_STARTUP_TARGET,
        stage = "chain_verify",
        total_blocks,
        elapsed_ms = replay_start.elapsed().as_millis() as u64,
        "chain_verify done"
    );
    Ok(())
}

/// Trust-disk checkpoint: no genesis→tip replay; verifies manifest/summary/tail linkage and PoA headers on the tail.
fn validate_snapshot_trusted(
    snapshot: &mut SnapshotData,
    cfg: &GenCfg,
    summary_path: &FsPath,
    opts: &SnapshotLoadOpts,
) -> Result<(), String> {
    if cfg.vals.set.is_empty() {
        return Err("snapshot validation error: genesis config has zero validators".into());
    }
    if snapshot.version != SNAPSHOT_VERSION {
        return Err(format!(
            "snapshot version mismatch: got {}, expected {}",
            snapshot.version, SNAPSHOT_VERSION
        ));
    }
    if snapshot.blocks.len() > u64::MAX as usize {
        return Err(
            "snapshot chain mismatch: blocks length exceeds supported u64 height range".into(),
        );
    }
    let want_ga = snapshot_genesis_accounts(cfg);
    if snapshot.genesis_accounts.len() != want_ga.len() {
        return Err(format!(
            "snapshot genesis mismatch: rows {} != {}",
            snapshot.genesis_accounts.len(),
            want_ga.len()
        ));
    }
    for (i, (got, exp)) in snapshot
        .genesis_accounts
        .iter()
        .zip(want_ga.iter())
        .enumerate()
    {
        if got.pubkey != exp.pubkey || got.acct != exp.acct || got.der_idx != exp.der_idx {
            return Err(format!("snapshot genesis mismatch at row {i}"));
        }
    }
    if snapshot.blocks_stored != BlocksStored::Epochs {
        return Err("snapshot trust validation: blocks_stored must be epochs".into());
    }
    let man = incremental::read_epoch_manifest(summary_path)?
        .ok_or_else(|| "snapshot trust validation: missing epoch manifest".to_string())?;
    ensure_epoch_man_schema(man.schema_v)?;
    let tip = man.canonical_h;
    if tip != snapshot.checkpoint_height {
        return Err(format!(
            "snapshot trust validation: summary checkpoint_height {} != manifest canonical_h {}",
            snapshot.checkpoint_height, tip
        ));
    }
    if tip == 0 {
        if !snapshot.blocks.is_empty() {
            return Err("snapshot trust validation: tip=0 but blocks non-empty".into());
        }
        let genesis_root = anchor::st_root(cfg);
        let st_root = digest(&snapshot.state);
        if st_root != genesis_root {
            return Err("snapshot state mismatch: empty chain must match genesis state".into());
        }
        if let Some(ref anch) = snapshot.genesis_anchor {
            anchor::chk_anch(anch, cfg, [0u8; 32])?;
        } else if opts.anchor_sk.is_some() {
            attach_anch(snapshot, cfg, summary_path, opts)?;
        } else if !opts.verify_chain && !allow_legacy_env() {
            return Err(
                "snapshot trust validation: missing genesis_anchor on legacy snapshot; set PWM_SNAPSHOT_ANCHOR_MIGRATE=1 (unsafe) or rerun with --snapshot-verify-chain"
                    .to_string(),
            );
        }
        return validate_snapshot_state_accounts(snapshot);
    }
    let n = snapshot.blocks.len();
    if n == 0 {
        return Err("snapshot trust validation: tip>0 but tail blocks missing".into());
    }
    let last = snapshot.blocks.last().expect("non-empty blocks");
    if last.hdr.height != tip {
        return Err(format!(
            "snapshot trust validation: tail tip height {} != manifest tip {}",
            last.hdr.height, tip
        ));
    }
    let start_h = tip
        .saturating_sub(TAIL_BLOCK_CAP as u64)
        .saturating_add(1)
        .max(1);
    let expect_n = (tip - start_h + 1) as usize;
    if n != expect_n {
        return Err(format!(
            "snapshot trust validation: expected {} tail blocks, got {}",
            expect_n, n
        ));
    }
    let first_h = tip - (n as u64) + 1;
    if first_h != start_h {
        return Err(format!(
            "snapshot trust validation: tail range mismatch (first_h {first_h} vs start_h {start_h})"
        ));
    }
    let want_tip_hex = hex::encode(hdr_hash(&last.hdr));
    if man.tip_hash != want_tip_hex {
        return Err(format!(
            "snapshot trust validation: manifest tip_hash {} != hdr_hash(last) {}",
            man.tip_hash, want_tip_hex
        ));
    }
    let st_root = digest(&snapshot.state);
    if last.hdr.state_root != st_root {
        return Err("snapshot trust validation: state root mismatch with tip block".into());
    }
    let want_tail_prod_idx = trust_tail_prod_idx(cfg, summary_path, &snapshot.state, tip, first_h)?;
    let total_blocks = snapshot.blocks.len() as u64;
    let trust_start = Instant::now();
    let mut last_log = trust_start;
    info!(
        target: SNAP_STARTUP_TARGET,
        stage = "trust_validate",
        total_blocks,
        tail_first_h = first_h,
        tip_h = tip,
        "trust_validate started"
    );
    for (i, blk) in snapshot.blocks.iter().enumerate() {
        let now = Instant::now();
        if now.duration_since(last_log).as_secs() >= 10 {
            let percent_complete = if total_blocks == 0 {
                100
            } else {
                ((i as u64) * 100) / total_blocks
            };
            info!(
                target: SNAP_STARTUP_TARGET,
                stage = "trust_validate",
                height = blk.hdr.height,
                total_blocks,
                percent_complete,
                elapsed_ms = trust_start.elapsed().as_millis() as u64,
                "trust_validate progress"
            );
            last_log = now;
        }
        let h = first_h + i as u64;
        if blk.hdr.height != h {
            return Err(format!(
                "snapshot trust validation: block[{i}] height {}, expected {h}",
                blk.hdr.height
            ));
        }
        if i == 0 {
            if h == 1 {
                if blk.hdr.prev_hash != prev_gen() {
                    return Err(
                        "snapshot trust validation: block[0] at height 1 must chain from genesis"
                            .into(),
                    );
                }
            } else {
                let parent =
                    incremental::load_block_at_height(summary_path, h - 1)?.ok_or_else(|| {
                        format!(
                            "snapshot trust validation: missing parent block at height {}",
                            h - 1
                        )
                    })?;
                if blk.hdr.prev_hash != hdr_hash(&parent.hdr) {
                    return Err(format!(
                        "snapshot trust validation: block[{i}] prev_hash does not link to height {}",
                        h - 1
                    ));
                }
            }
        } else {
            let prev_blk = &snapshot.blocks[i - 1];
            if blk.hdr.prev_hash != hdr_hash(&prev_blk.hdr) {
                return Err(format!(
                    "snapshot trust validation: block[{i}] prev_hash does not match previous header hash"
                ));
            }
        }
        let want_tx_root = txs_root(&blk.txs);
        if blk.hdr.tx_root != want_tx_root {
            return Err(format!(
                "snapshot trust validation: block[{i}] tx_root is invalid"
            ));
        }
        let want_prod_idx = *want_tail_prod_idx.get(i).ok_or_else(|| {
            format!("snapshot trust validation: block[{i}] missing expected proposer")
        })?;
        if let Some(want_prod_idx) = want_prod_idx {
            if blk.hdr.prod_idx != want_prod_idx {
                return Err(format!(
                    "snapshot trust validation: block[{i}] prod_idx {}, expected {}",
                    blk.hdr.prod_idx, want_prod_idx
                ));
            }
        }
        let prod = cfg.vals.set.get(blk.hdr.prod_idx as usize).ok_or_else(|| {
            format!("snapshot trust validation: block[{i}] prod_idx out of range")
        })?;
        if !blk.hdr.verify_sig(&prod.pubkey) {
            return Err(format!(
                "snapshot trust validation: block[{i}] has invalid producer signature"
            ));
        }
    }
    let blk1_hash = preflight_blk1(snapshot, cfg, summary_path)?;
    if let Some(ref anch) = snapshot.genesis_anchor {
        anchor::chk_anch(anch, cfg, blk1_hash)?;
    } else if opts.anchor_sk.is_some() {
        attach_anch(snapshot, cfg, summary_path, opts)?;
    } else if !opts.verify_chain && !allow_legacy_env() {
        return Err(
            "snapshot trust validation: missing genesis_anchor on legacy snapshot; set PWM_SNAPSHOT_ANCHOR_MIGRATE=1 (unsafe) or rerun with --snapshot-verify-chain"
                .to_string(),
        );
    }
    info!(
        target: SNAP_STARTUP_TARGET,
        stage = "trust_validate",
        total_blocks,
        elapsed_ms = trust_start.elapsed().as_millis() as u64,
        "trust_validate done"
    );
    validate_snapshot_state_accounts(snapshot)
}

fn tail_epoch_bnd_h(cfg: &GenCfg, first_h: u64, tip_h: u64) -> Option<u64> {
    let epoch_len = cfg.epoch_length_blocks;
    if epoch_len == 0 {
        return None;
    }
    let rem = first_h % epoch_len;
    let bnd_h = if rem == 0 {
        first_h
    } else {
        first_h.saturating_add(epoch_len.saturating_sub(rem))
    };
    (bnd_h <= tip_h).then_some(bnd_h)
}

fn trust_tail_prod_idx(
    cfg: &GenCfg,
    _summary_path: &FsPath,
    snap_state: &pwm_core::State,
    tip_h: u64,
    tail_first_h: u64,
) -> Result<Vec<Option<u32>>, String> {
    let mut want = vec![None; (tip_h - tail_first_h + 1) as usize];
    if let Some(bnd_h) = tail_epoch_bnd_h(cfg, tail_first_h, tip_h) {
        for h in bnd_h..=tip_h {
            let prod_idx = pick_prod_idx(h, &snap_state.active_validator_indices).map_err(|e| {
                format!("snapshot trust validation: height {h} proposer pick failed: {e}")
            })?;
            let tail_pos = usize::try_from(h.saturating_sub(tail_first_h))
                .map_err(|_| "snapshot trust validation: tail index overflow".to_string())?;
            if tail_pos >= want.len() {
                return Err(format!(
                    "snapshot trust validation: height {h} outside tail {tail_first_h}..={tip_h}"
                ));
            }
            want[tail_pos] = Some(prod_idx);
        }
        return Ok(want);
    }

    for h in tail_first_h..=tip_h {
        let prod_idx = pick_prod_idx(h, &snap_state.active_validator_indices).map_err(|e| {
            format!("snapshot trust validation: height {h} proposer pick failed: {e}")
        })?;
        let tail_pos = usize::try_from(h.saturating_sub(tail_first_h))
            .map_err(|_| "snapshot trust validation: tail index overflow".to_string())?;
        want[tail_pos] = Some(prod_idx);
    }
    Ok(want)
}

/// Decodes canonical snapshot JSON with replay (ClickHouse `ch_load`; JsonFile uses [`load_snapshot`]).
#[cfg_attr(not(feature = "clickhouse-snapshot"), allow(dead_code))]
pub(crate) fn decode_snapshot_txt(txt: &str, cfg: &GenCfg) -> Result<Option<SnapshotData>, String> {
    let mut snap = decode_snap_raw(txt, cfg)?;
    if let Some(ref mut s) = snap {
        if s.blocks_stored == BlocksStored::Epochs && s.blocks.is_empty() {
            return Err(
                "epoch snapshot: empty blocks in JSON; use load_snapshot with epochs dir".into(),
            );
        }
        validate_snapshot(s, cfg)?;
    }
    Ok(snap)
}

/// JSON parse + decode **without** replay validation. **Bench / diagnostics only.**
pub(crate) fn decode_snap_raw(txt: &str, cfg: &GenCfg) -> Result<Option<SnapshotData>, String> {
    let raw: Value = serde_json::from_str(txt).map_err(|e| format!("parse snapshot JSON: {e}"))?;
    decode_snap_value_raw(raw, cfg)
}

/// Full chain replay verification (same work as inside a normal load after decode).
pub(crate) fn replay_validate(snap: &mut SnapshotData, cfg: &GenCfg) -> Result<(), String> {
    validate_snapshot(snap, cfg)
}

/// True when [`Chain::blocks`] holds the full chain `1..=tip` (no eviction yet).
pub(crate) fn blocks_cover_full_history(chain: &Chain) -> bool {
    let tip = chain.tip_h();
    if tip == 0 {
        return true;
    }
    let n = chain.blocks.len();
    if n != tip as usize {
        return false;
    }
    chain.blocks.iter().next().map(|b| b.hdr.height) == Some(1)
}

fn verify_tail_vs_epochs(chain: &Chain, full: &[Block]) -> Result<(), String> {
    let n = chain.blocks.len();
    if n == 0 {
        return Err("snapshot encode tail verify: empty memory tail".into());
    }
    if full.len() < n {
        return Err(format!(
            "snapshot encode tail verify: epoch assembly len {} < tail {}",
            full.len(),
            n
        ));
    }
    let start = full.len() - n;
    for (i, b) in chain.blocks.iter().enumerate() {
        if full.get(start + i) != Some(b) {
            let ep = full.get(start + i);
            return Err(format!(
                "snapshot encode tail verify: height {} mismatch vs epochs (memory_tip={} assembled_blocks_len={} expected_at_tail_idx={} epoch_block_height={:?})",
                b.hdr.height,
                chain.tip_h(),
                full.len(),
                start + i,
                ep.map(|x| x.hdr.height)
            ));
        }
    }
    Ok(())
}

/// Monolithic snapshot JSON for migrations / debug save. Uses epoch files when memory holds a tail slice only.
pub(crate) fn encode_inner_snap_json(
    inner: &Inner,
    summary_path: Option<&FsPath>,
) -> Result<String, String> {
    let blocks: Vec<Block> = if inner.chain.tip_h() == 0 {
        vec![]
    } else if blocks_cover_full_history(&inner.chain) {
        inner.chain.blocks.iter().cloned().collect()
    } else {
        let p = summary_path.ok_or_else(|| {
            "monolithic snapshot encode: tail-only chain in memory; pass pwm-data.json path"
                .to_string()
        })?;
        if super::epoch::manifest_file_path(p).exists() {
            incremental::sync_epoch_to_tip(p, inner)?;
        }
        let full = incremental::load_blocks_from_epochs(p)?;
        verify_tail_vs_epochs(&inner.chain, &full)?;
        full
    };
    let snap = SnapshotData {
        version: SNAPSHOT_VERSION,
        genesis_accounts: snapshot_genesis_accounts(&inner.chain.cfg),
        genesis_anchor: None,
        blocks,
        state: inner.chain.st.clone(),
        roaming: roaming_to_wire(&inner.roaming_pool),
        cross_shard: inner.cross_shard.clone(),
        blocks_stored: BlocksStored::Inline,
        checkpoint_height: 0,
    };
    let mut snap = snap;
    if let Some(path) = summary_path {
        fill_anch(path, inner, &mut snap)?;
    }
    encode_snap_data_txt(&snap)
}

/// Canonical JSON for ClickHouse `snapshot_json` (pretty v4 wire, matches [`snap_wire_json_bytes`]).
pub(crate) fn encode_snap_data_txt(snap: &SnapshotData) -> Result<String, String> {
    let wire = data_to_v4(snap);
    serde_json::to_string_pretty(&wire).map_err(|e| format!("encode snapshot: {e}"))
}

/// Stable bytes for equality of two-loaded snapshots: `serde_json::to_vec` of v4 wire (`data_to_v4`).
pub(crate) fn snap_wire_json_bytes(snap: &SnapshotData) -> Result<Vec<u8>, String> {
    let v4 = data_to_v4(snap);
    serde_json::to_vec(&v4).map_err(|e| format!("snap wire json: {e}"))
}

pub(crate) fn load_snapshot(path: &FsPath, cfg: &GenCfg) -> Result<Option<SnapshotData>, String> {
    load_snapshot_timed(path, cfg, SnapshotLoadOpts::verify_full()).map(|(s, _)| s)
}

/// Same as [`load_snapshot`] plus millisecond breakdown for startup telemetry.
pub(crate) fn load_snapshot_timed(
    path: &FsPath,
    cfg: &GenCfg,
    opts: SnapshotLoadOpts,
) -> Result<(Option<SnapshotData>, JsonSnapTiming), String> {
    let mut br = JsonSnapTiming::default();
    let mp = manifest_file_path(path);
    let has_summary = path.exists();
    if !has_summary && !mp.exists() {
        return Ok((None, br));
    }
    let snap = if has_summary {
        let t_sum = Instant::now();
        let txt = std::fs::read_to_string(path).map_err(|e| {
            warn!(
                target: SNAP_STARTUP_TARGET,
                stage = "summary_read",
                elapsed_ms = t_sum.elapsed().as_millis() as u64,
                err = %e,
                "json snapshot load failed"
            );
            format!("read snapshot: {e}")
        })?;
        let raw = decode_snap_raw(&txt, cfg).map_err(|e| {
            warn!(
                target: SNAP_STARTUP_TARGET,
                stage = "summary_decode",
                elapsed_ms = t_sum.elapsed().as_millis() as u64,
                err = %e,
                "json snapshot load failed"
            );
            e
        })?;
        br.summary_read_ms = t_sum.elapsed().as_millis() as u64;
        raw
    } else {
        Some(SnapshotData {
            version: SNAPSHOT_VERSION,
            genesis_accounts: snapshot_genesis_accounts(cfg),
            genesis_anchor: None,
            blocks: vec![],
            state: cfg.state0(),
            roaming: SnapshotRoamingWire::default(),
            cross_shard: CrossShardLedger::default(),
            blocks_stored: BlocksStored::Epochs,
            checkpoint_height: 0,
        })
    };
    let Some(mut snap) = snap else {
        return Ok((None, br));
    };
    let mut effective_opts = opts;
    let mut load_mode = "trust";
    let mut lag_forced_verify = false;
    let mut man_tip = None;
    if snap.blocks_stored == BlocksStored::Epochs && !effective_opts.verify_chain && mp.exists() {
        if let Ok(Some(man)) = incremental::read_epoch_manifest(path) {
            man_tip = Some(man.canonical_h);
            if man.canonical_h > 0 && man.canonical_h != snap.checkpoint_height {
                lag_forced_verify = true;
                effective_opts.verify_chain = true;
            }
        }
    }
    if snap.blocks_stored == BlocksStored::Epochs {
        let load_reason = if effective_opts.verify_chain {
            load_mode = "full_verify";
            if lag_forced_verify {
                "summary_manifest_lag"
            } else {
                "verify_chain_flag"
            }
        } else {
            "trust_checkpoint"
        };
        info!(
            target: SNAP_STARTUP_TARGET,
            snapshot_load_mode = load_mode,
            reason = load_reason,
            summary_checkpoint = snap.checkpoint_height,
            manifest_tip = man_tip.unwrap_or(0),
            "snapshot load mode selected"
        );
    }
    if snap.blocks_stored == BlocksStored::Epochs {
        let te = Instant::now();
        let epoch_load = if effective_opts.verify_chain {
            incremental::load_blocks_from_epochs(path)
        } else {
            incremental::load_tail_blocks(path, TAIL_BLOCK_CAP)
        };
        snap.blocks = epoch_load.map_err(|e| {
            warn!(
                target: SNAP_STARTUP_TARGET,
                stage = "epochs_load",
                elapsed_ms = te.elapsed().as_millis() as u64,
                err = %e,
                "json snapshot load failed"
            );
            e
        })?;
        br.epochs_ms = te.elapsed().as_millis() as u64;
    }
    let tv = Instant::now();
    let validate_err = match (&snap.blocks_stored, effective_opts.verify_chain) {
        (BlocksStored::Epochs, false) => {
            validate_snapshot_trusted(&mut snap, cfg, path, &effective_opts)
        }
        _ => validate_snapshot(&mut snap, cfg),
    };
    validate_err.map_err(|e| {
        warn!(
            target: SNAP_STARTUP_TARGET,
            stage = "validate",
            elapsed_ms = tv.elapsed().as_millis() as u64,
            err = %e,
            "json snapshot load failed"
        );
        e
    })?;
    br.validate_ms = tv.elapsed().as_millis() as u64;
    br.used_full_verify = snap.blocks_stored == BlocksStored::Epochs && effective_opts.verify_chain;
    br.lag_forced_verify = lag_forced_verify;
    Ok((Some(snap), br))
}

fn decode_snap_value_raw(raw: Value, cfg: &GenCfg) -> Result<Option<SnapshotData>, String> {
    let obj = raw
        .as_object()
        .ok_or_else(|| "parse snapshot JSON: root must be an object".to_string())?;
    let has_version = obj.contains_key("version");
    let has_genesis_accounts = obj.contains_key("genesis_accounts");
    let has_blocks = obj.contains_key("blocks");
    let has_state = obj.contains_key("state");
    let snap: SnapshotData = if has_version || has_genesis_accounts {
        if has_version != has_genesis_accounts {
            return Err(
                "snapshot contract error: canonical snapshot requires both 'version' and 'genesis_accounts'; regenerate snapshot from current pwmd"
                    .into(),
            );
        }
        if !has_blocks || !has_state {
            return Err(
                "snapshot contract error: canonical snapshot must contain both 'blocks' and 'state'; regenerate snapshot from current pwmd"
                    .into(),
            );
        }
        let version = obj.get("version").and_then(Value::as_u64).ok_or_else(|| {
            "snapshot contract error: version must be an unsigned integer".to_string()
        })?;
        let mut canonical = serde_json::Map::new();
        canonical.insert(
            "version".to_string(),
            obj.get("version")
                .ok_or_else(|| {
                    "snapshot contract error: canonical snapshot missing required field 'version'"
                        .to_string()
                })?
                .clone(),
        );
        canonical.insert(
            "genesis_accounts".to_string(),
            obj.get("genesis_accounts")
                .ok_or_else(|| {
                    "snapshot contract error: canonical snapshot missing required field 'genesis_accounts'"
                        .to_string()
                })?
                .clone(),
        );
        if let Some(anch) = obj.get("genesis_anchor") {
            canonical.insert("genesis_anchor".to_string(), anch.clone());
        }
        canonical.insert(
            "blocks".to_string(),
            obj.get("blocks")
                .ok_or_else(|| {
                    "snapshot contract error: canonical snapshot missing required field 'blocks'"
                        .to_string()
                })?
                .clone(),
        );
        canonical.insert(
            "state".to_string(),
            obj.get("state")
                .ok_or_else(|| {
                    "snapshot contract error: canonical snapshot missing required field 'state'"
                        .to_string()
                })?
                .clone(),
        );
        if let Some(roaming) = obj.get("roaming") {
            canonical.insert("roaming".to_string(), roaming.clone());
        }
        if let Some(cross_shard) = obj.get("cross_shard") {
            canonical.insert("cross_shard".to_string(), cross_shard.clone());
        }
        if let Some(bs) = obj.get("blocks_stored") {
            canonical.insert("blocks_stored".to_string(), bs.clone());
        }
        if let Some(ch) = obj.get("checkpoint_height") {
            canonical.insert("checkpoint_height".to_string(), ch.clone());
        }
        let dropped = obj
            .keys()
            .filter(|k| {
                *k != "version"
                    && *k != "genesis_accounts"
                    && *k != "genesis_anchor"
                    && *k != "blocks"
                    && *k != "state"
                    && *k != "roaming"
                    && *k != "cross_shard"
                    && *k != "blocks_stored"
                    && *k != "checkpoint_height"
            })
            .cloned()
            .collect::<Vec<_>>();
        if !dropped.is_empty() {
            warn!(
                "snapshot canonical-only mode: ignoring non-canonical top-level fields: {}",
                dropped.join(", ")
            );
        }
        match version {
            v if v == u64::from(SNAPSHOT_VERSION) => {
                let wire: SnapshotDataV4 = serde_json::from_value(Value::Object(canonical))
                    .map_err(|e| format!("parse canonical snapshot JSON: {e}"))?;
                data_from_v4(wire).map_err(|e| format!("parse canonical snapshot JSON: {e}"))?
            }
            v if v == u64::from(SNAPSHOT_V3) => {
                let wire: SnapshotDataV3 = serde_json::from_value(Value::Object(canonical))
                    .map_err(|e| format!("parse v3 snapshot JSON: {e}"))?;
                data_from_v3(wire).map_err(|e| format!("parse v3 snapshot JSON: {e}"))?
            }
            v if v == u64::from(SNAPSHOT_V2) => {
                let wire: SnapshotDataV2 = serde_json::from_value(Value::Object(canonical))
                    .map_err(|e| format!("parse v2 snapshot JSON: {e}"))?;
                data_from_v2(wire).map_err(|e| format!("parse v2 snapshot JSON: {e}"))?
            }
            v if v == u64::from(SNAPSHOT_V1) => {
                let mut snap: SnapshotData = serde_json::from_value(Value::Object(canonical))
                    .map_err(|e| format!("parse v1 snapshot JSON: {e}"))?;
                snap.version = SNAPSHOT_VERSION;
                snap
            }
            other => {
                return Err(format!(
                    "snapshot version mismatch: got {other}, expected {SNAPSHOT_VERSION}, {SNAPSHOT_V3}, {SNAPSHOT_V2} or {SNAPSHOT_V1}"
                ));
            }
        }
    } else {
        if !has_blocks || !has_state {
            return Err(
                "snapshot legacy migration error: legacy snapshot must contain both 'blocks' and 'state'; regenerate snapshot from current pwmd"
                    .into(),
            );
        }
        let dropped = obj
            .keys()
            .filter(|k| *k != "blocks" && *k != "state")
            .cloned()
            .collect::<Vec<_>>();
        if !dropped.is_empty() {
            warn!(
                "snapshot legacy migration: ignoring non-canonical top-level fields: {}",
                dropped.join(", ")
            );
        }
        let mut legacy_obj = serde_json::Map::new();
        legacy_obj.insert(
            "blocks".to_string(),
            obj.get("blocks")
                .ok_or_else(|| {
                    "snapshot legacy migration error: legacy snapshot missing required field 'blocks'"
                        .to_string()
                })?
                .clone(),
        );
        legacy_obj.insert(
            "state".to_string(),
            obj.get("state")
                .ok_or_else(|| {
                    "snapshot legacy migration error: legacy snapshot missing required field 'state'"
                        .to_string()
                })?
                .clone(),
        );
        let legacy: SnapshotDataLegacyV0 = serde_json::from_value(Value::Object(legacy_obj))
            .map_err(|e| format!("parse legacy snapshot JSON: {e}"))?;
        SnapshotData {
            version: SNAPSHOT_VERSION,
            genesis_accounts: snapshot_genesis_accounts(cfg),
            genesis_anchor: None,
            blocks: legacy.blocks,
            state: legacy.state,
            roaming: SnapshotRoamingWire::default(),
            cross_shard: CrossShardLedger::default(),
            blocks_stored: BlocksStored::Inline,
            checkpoint_height: 0,
        }
    };
    Ok(Some(snap))
}

pub(crate) fn save_snapshot(path: &FsPath, inner: &Inner) -> Result<(), String> {
    let txt = encode_inner_snap_json(inner, Some(path))?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| format!("create snapshot dir: {e}"))?;
        }
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, txt).map_err(|e| format!("write snapshot temp: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("replace snapshot: {e}"))?;
    Ok(())
}

/// JsonFile hot save (`snap_save_locked`): sync epoch JSONL to tip, rewrite summary only.
/// Avoids monolithic `encode_inner_snap_json` (no full `load_blocks_from_epochs` for merge).
/// Legacy trees without epoch manifest still use [`save_snapshot`].
pub(crate) fn json_file_runtime_persist(path: &FsPath, inner: &Inner) -> Result<(), String> {
    if manifest_file_path(path).exists() {
        incremental::sync_epoch_to_tip(path, inner)?;
        save_checkpoint_summary(path, inner)
    } else {
        save_snapshot(path, inner)
    }
}

/// Persist summary checkpoint (`pwm-data.json` without `blocks[]`); chain in `epochs/` JSONL.
pub(crate) fn save_checkpoint_summary(path: &FsPath, inner: &Inner) -> Result<(), String> {
    let h = inner.chain.tip_h();
    let mut snap = SnapshotData {
        version: SNAPSHOT_VERSION,
        genesis_accounts: snapshot_genesis_accounts(&inner.chain.cfg),
        genesis_anchor: None,
        blocks: vec![],
        state: inner.chain.st.clone(),
        roaming: roaming_to_wire(&inner.roaming_pool),
        cross_shard: inner.cross_shard.clone(),
        blocks_stored: BlocksStored::Epochs,
        checkpoint_height: h,
    };
    fill_anch(path, inner, &mut snap)?;
    let txt = encode_snap_data_txt(&snap)?;
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent).map_err(|e| format!("create snapshot dir: {e}"))?;
        }
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, txt).map_err(|e| format!("write snapshot temp: {e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("replace snapshot: {e}"))?;
    Ok(())
}

/// Same wire as checkpoint summary (tip-aligned state); used when seal did not run (e.g. relay).
pub(crate) fn save_epochs_sum_tip(path: &FsPath, inner: &Inner) -> Result<(), String> {
    save_checkpoint_summary(path, inner)
}

/// Seal-time JsonFile flush: sync epoch JSONL tail to tip, then rewrite tip-aligned summary.
pub(crate) fn json_file_seal_persist(path: &FsPath, inner: &Inner) -> Result<(), String> {
    incremental::sync_epoch_to_tip(path, inner)?;
    save_checkpoint_summary(path, inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::incremental::append_block_for_epoch;
    use pwm_core::hd::domain_of_account_id;
    use pwm_core::tx::{SignedTx, TxBody};
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn snap_replay_uses_blk_ctx() {
        let (cfg, sks) = pwm_core::dev_net();
        let mut chain = pwm_core::Chain::boot(cfg.clone(), sks.clone());
        let signer = cfg.accounts[0].acct;
        let tx = SignedTx::sign_body(
            &sks[0],
            domain_of_account_id(&signer),
            cfg.accounts[0].der_idx,
            0,
            TxBody::Stake { amount: 1 },
        );
        chain.seal(vec![tx]).expect("seal");
        let blocks = chain.blocks.iter().cloned().collect::<Vec<_>>();
        let mut snap = SnapshotData {
            version: SNAPSHOT_VERSION,
            genesis_accounts: snapshot_genesis_accounts(&cfg),
            genesis_anchor: None,
            blocks,
            state: chain.st.clone(),
            roaming: SnapshotRoamingWire::default(),
            cross_shard: CrossShardLedger::default(),
            blocks_stored: BlocksStored::Inline,
            checkpoint_height: 0,
        };
        validate_snapshot(&mut snap, &cfg).expect("replay with block context");
    }

    #[test]
    fn v4_replay_det_gate_ok() {
        let (cfg, sks) = pwm_core::dev_net();
        let mut chain = pwm_core::Chain::boot(cfg.clone(), sks.clone());
        let signer = cfg.accounts[0].acct;
        let tx = SignedTx::sign_body(
            &sks[0],
            domain_of_account_id(&signer),
            cfg.accounts[0].der_idx,
            0,
            TxBody::Stake { amount: 1 },
        );
        chain.seal(vec![tx]).expect("seal");
        chain.seal(vec![]).expect("seal #2");
        let blocks = chain.blocks.iter().cloned().collect::<Vec<_>>();
        let base = SnapshotData {
            version: SNAPSHOT_VERSION,
            genesis_accounts: snapshot_genesis_accounts(&cfg),
            genesis_anchor: None,
            blocks,
            state: chain.st.clone(),
            roaming: SnapshotRoamingWire::default(),
            cross_shard: CrossShardLedger::default(),
            blocks_stored: BlocksStored::Epochs,
            checkpoint_height: chain.tip_h(),
        };
        let mut run_a = base.clone();
        let mut run_b = base;
        replay_validate(&mut run_a, &cfg).expect("replay A");
        replay_validate(&mut run_b, &cfg).expect("replay B");
        assert_eq!(digest(&run_a.state), digest(&run_b.state));
        assert_eq!(
            run_a.blocks.last().map(|b| hdr_hash(&b.hdr)),
            run_b.blocks.last().map(|b| hdr_hash(&b.hdr))
        );
    }

    #[test]
    fn legacy_snapshot_defaults_policy_fields() {
        let (cfg, _sks) = pwm_core::dev_net();
        let snap = SnapshotData {
            version: SNAPSHOT_VERSION,
            genesis_accounts: snapshot_genesis_accounts(&cfg),
            genesis_anchor: None,
            blocks: vec![],
            state: cfg.state0(),
            roaming: SnapshotRoamingWire::default(),
            cross_shard: CrossShardLedger::default(),
            blocks_stored: BlocksStored::Inline,
            checkpoint_height: 0,
        };
        let txt = encode_snap_data_txt(&snap).expect("encode");
        let mut raw: Value = serde_json::from_str(&txt).expect("json");
        let accounts = raw
            .as_object_mut()
            .and_then(|x| x.get_mut("state"))
            .and_then(Value::as_object_mut)
            .and_then(|x| x.get_mut("accounts"))
            .and_then(Value::as_array_mut)
            .expect("state.accounts");
        let account_obj = accounts
            .first_mut()
            .and_then(Value::as_object_mut)
            .and_then(|x| x.get_mut("account"))
            .and_then(Value::as_object_mut)
            .expect("account row");
        account_obj.remove("rescue_address");
        account_obj.remove("active_policies");
        account_obj.remove("dormant_policies");
        account_obj.remove("finalized");
        account_obj.remove("owner_kind");
        account_obj.remove("owner_display_name");
        account_obj.remove("owner_country_hint");
        account_obj.remove("company_metadata_commitment");
        account_obj.remove("external_verification_ref");
        account_obj.remove("requested_domain_lo");
        let loaded = decode_snap_raw(&raw.to_string(), &cfg)
            .expect("decode")
            .expect("some");
        let one = loaded.state.accounts.values().next().expect("has account");
        assert_eq!(one.rescue_address, None);
        assert_eq!(one.active_policies, 0);
        assert_eq!(one.dormant_policies, 0);
        assert!(!one.finalized);
    }

    #[test]
    fn preflight_blk1_tamper_tx_root() {
        let (cfg, sks) = pwm_core::dev_net();
        let mut chain = Chain::boot(cfg.clone(), sks.clone());
        chain.seal(vec![]).expect("seal #1");
        let mut blk1 = chain.blocks[0].clone();
        blk1.hdr.tx_root[0] ^= 0x01;
        let snap = SnapshotData {
            version: SNAPSHOT_VERSION,
            genesis_accounts: snapshot_genesis_accounts(&cfg),
            genesis_anchor: None,
            blocks: vec![blk1],
            state: chain.st.clone(),
            roaming: SnapshotRoamingWire::default(),
            cross_shard: CrossShardLedger::default(),
            blocks_stored: BlocksStored::Epochs,
            checkpoint_height: 1,
        };
        let err = preflight_blk1(&snap, &cfg, Path::new("pwm-data.json"))
            .expect_err("tampered block1 must be rejected");
        assert!(err.contains("tx_root is invalid"));
    }

    #[test]
    fn attach_anchor_legacy_with_signer() {
        let (cfg, sks) = pwm_core::dev_net();
        let mut chain = Chain::boot(cfg.clone(), sks.clone());
        chain.seal(vec![]).expect("seal #1");
        let blk1 = chain.blocks[0].clone();
        let mut snap = SnapshotData {
            version: SNAPSHOT_VERSION,
            genesis_accounts: snapshot_genesis_accounts(&cfg),
            genesis_anchor: None,
            blocks: vec![blk1.clone()],
            state: chain.st.clone(),
            roaming: SnapshotRoamingWire::default(),
            cross_shard: CrossShardLedger::default(),
            blocks_stored: BlocksStored::Epochs,
            checkpoint_height: 1,
        };
        let opts = SnapshotLoadOpts {
            verify_chain: false,
            anchor_sk: Some(sks[0].clone()),
            anchor_idx: 0,
        };
        attach_anch(&mut snap, &cfg, Path::new("pwm-data.json"), &opts).expect("migrate");
        let anch = snap.genesis_anchor.as_ref().expect("anchor attached");
        anchor::chk_anch(anch, &cfg, hdr_hash(&blk1.hdr)).expect("anchor verifies");
    }

    #[test]
    fn trust_prod_no_bnd_set() {
        let (cfg, _sks) = pwm_core::dev_net();
        let mut snap_state = cfg.state0();
        snap_state.active_validator_indices = vec![2, 0, 1];
        let tip_h = 2_500;
        let first_h = 2_001;
        let got = trust_tail_prod_idx(
            &cfg,
            Path::new("pwm-data.json"),
            &snap_state,
            tip_h,
            first_h,
        )
        .expect("tail proposer idx");
        assert_eq!(got.len() as u64, tip_h - first_h + 1);
        for (i, row) in got.iter().enumerate() {
            let h = first_h + i as u64;
            let want = pick_prod_idx(h, &snap_state.active_validator_indices).expect("pick");
            assert_eq!(*row, Some(want), "height={h}");
        }
    }

    #[test]
    fn trust_prod_tail_bnd_tx_ok() {
        let sfx = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("pwm-trust-bnd-tail-{sfx}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let pb = dir.join("pwm-data.json");

        let (mut cfg, sks) = pwm_core::dev_net();
        cfg.epoch_length_blocks = 5;
        let mut chain = Chain::boot(cfg.clone(), sks.clone());
        let signer = cfg.accounts[0].acct;
        for i in 0..16 {
            let txs = if i >= 14 {
                let nonce = (i - 14) as u64;
                vec![SignedTx::sign_body(
                    &sks[0],
                    domain_of_account_id(&signer),
                    cfg.accounts[0].der_idx,
                    nonce,
                    TxBody::Stake { amount: 1 },
                )]
            } else {
                vec![]
            };
            chain.seal(txs).expect("seal");
            let blk = chain.blocks.back().expect("tip block");
            append_block_for_epoch(&pb, blk).expect("append");
        }
        let tip_h = chain.tip_h();
        let first_h = 12;
        let got = trust_tail_prod_idx(&cfg, &pb, &chain.st, tip_h, first_h).expect("tail prod");
        assert_eq!(got.len(), 5);
        assert_eq!(got[0], None);
        assert_eq!(got[1], None);
        assert_eq!(got[2], None);
        let want_h15 = pick_prod_idx(15, &chain.st.active_validator_indices).expect("pick h15");
        let want_h16 = pick_prod_idx(16, &chain.st.active_validator_indices).expect("pick h16");
        assert_eq!(got[3], Some(want_h15));
        assert_eq!(got[4], Some(want_h16));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
