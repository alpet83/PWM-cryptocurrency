//! Offline JsonFile epoch repair to a reproducible checkpoint height.

use super::epoch::{
    epoch_file_name, epoch_idx, epoch_range, manifest_file_path, mk_manifest, EpochMeta,
};
use super::genesis::snapshot_genesis_accounts;
use super::io::{decode_snap_raw, encode_snap_data_txt, load_snapshot};
use super::types::{BlocksStored, SnapshotData, SnapshotRoamingWire, SNAPSHOT_VERSION};
use crate::ledger::CrossShardLedger;
use pwm_core::block::{hdr_hash, txs_root, Block};
use pwm_core::chain::{pick_prod_idx, prev_gen, recompute_active_idxs, roll_epoch_if_needed};
use pwm_core::digest;
use pwm_core::genesis::GenCfg;
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug)]
pub struct SnapRepairOpts {
    pub target_h: Option<u64>,
    pub backup: bool,
    pub dry_run: bool,
}

#[derive(Clone, Debug)]
pub struct SnapRepairReport {
    pub last_good_h: u64,
    pub target_h: u64,
    pub tip_hash: String,
    pub wrote_files: bool,
    pub backup_dir: Option<PathBuf>,
    pub kept_aux_summary: bool,
}

#[derive(Clone, Debug)]
struct ReplayRes {
    last_good_h: u64,
    last_good_hash: String,
    state: pwm_core::State,
}

pub fn repair_json_epochs(
    summary_path: &Path,
    cfg: &GenCfg,
    opts: SnapRepairOpts,
) -> Result<SnapRepairReport, String> {
    let all = read_epoch_blocks(summary_path)?;
    let replay_all = replay_to(cfg, &all, None)?;
    let target_h = match opts.target_h {
        Some(h) => {
            if h > replay_all.last_good_h {
                return Err(format!(
                    "repair target too high: requested {h}, last reproducible {}",
                    replay_all.last_good_h
                ));
            }
            h
        }
        None => replay_all.last_good_h,
    };
    let replay_target = replay_to(cfg, &all, Some(target_h))?;
    if replay_target.last_good_h != target_h {
        return Err(format!(
            "repair target {target_h} is not reproducible (replayed to {})",
            replay_target.last_good_h
        ));
    }

    let (roaming, cross_shard, kept_aux_summary) = load_aux_summary(summary_path, cfg, target_h);
    let mut backup_dir = None;
    if !opts.dry_run {
        if opts.backup {
            backup_dir = Some(make_backup(summary_path)?);
        }
        rewrite_epochs(summary_path, &all, target_h)?;
        rewrite_manifest(summary_path, &all, target_h, &replay_target.last_good_hash)?;
        rewrite_summary(
            summary_path,
            cfg,
            replay_target.state.clone(),
            roaming,
            cross_shard,
            target_h,
        )?;
        let loaded = load_snapshot(summary_path, cfg)?
            .ok_or_else(|| "repair post-check failed: snapshot missing after write".to_string())?;
        if loaded.blocks.len() as u64 != target_h {
            return Err(format!(
                "repair post-check failed: loaded tip {} != target {}",
                loaded.blocks.len(),
                target_h
            ));
        }
    }
    Ok(SnapRepairReport {
        last_good_h: replay_all.last_good_h,
        target_h,
        tip_hash: replay_target.last_good_hash,
        wrote_files: !opts.dry_run,
        backup_dir,
        kept_aux_summary,
    })
}

fn read_epoch_blocks(summary_path: &Path) -> Result<Vec<Block>, String> {
    let mut files = epoch_files(summary_path)?;
    files.sort_by_key(|(idx, _)| *idx);
    let mut out = Vec::new();
    let mut want_h = 1u64;
    for (_, path) in files {
        let file = fs::File::open(&path).map_err(|e| format!("open {}: {e}", path.display()))?;
        for (line_i, line) in std::io::BufReader::new(file).lines().enumerate() {
            let line = line.map_err(|e| format!("read {} line {}: {e}", path.display(), line_i))?;
            if line.trim().is_empty() {
                continue;
            }
            let blk: Block = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => return Ok(out),
            };
            if blk.hdr.height != want_h {
                return Ok(out);
            }
            out.push(blk);
            want_h = want_h.saturating_add(1);
        }
    }
    Ok(out)
}

fn replay_to(cfg: &GenCfg, blocks: &[Block], target_h: Option<u64>) -> Result<ReplayRes, String> {
    if cfg.vals.set.is_empty() {
        return Err("repair replay: genesis config has zero validators".into());
    }
    let limit = target_h.unwrap_or(u64::MAX);
    let mut prev = prev_gen();
    let mut replay_state = cfg.state0();
    replay_state.active_validator_indices = recompute_active_idxs(cfg, &replay_state);
    let mut last_good_h = 0u64;
    let mut last_good_hash = String::new();
    for (i, blk) in blocks.iter().enumerate() {
        let h = (i as u64) + 1;
        if h > limit {
            break;
        }
        if blk.hdr.height != h || blk.hdr.prev_hash != prev {
            break;
        }
        if blk.hdr.tx_root != txs_root(&blk.txs) {
            break;
        }
        roll_epoch_if_needed(cfg, &mut replay_state, h);
        let want_prod_idx = pick_prod_idx(h, &replay_state.active_validator_indices)?;
        if blk.hdr.prod_idx != want_prod_idx {
            break;
        }
        let Some(prod) = cfg.vals.set.get(blk.hdr.prod_idx as usize) else {
            break;
        };
        if !blk.hdr.verify_sig(&prod.pubkey) {
            break;
        }
        let mut block_ok = true;
        replay_state.refund_exp_locks(h);
        for tx in &blk.txs {
            if replay_state
                .apply_tx_with_ctx(tx, blk.hdr.height, blk.hdr.ts, cfg)
                .is_err()
            {
                block_ok = false;
                break;
            }
        }
        if !block_ok {
            break;
        }
        replay_state.refund_exp_locks(h);
        replay_state.drain_conservation_at_height(h, cfg);
        let prod_acct = cfg.prod_acct(blk.hdr.prod_idx);
        if cfg.is_legacy_policy() {
            replay_state.reward_producer(&prod_acct, cfg.block_reward);
        } else {
            let season_ppm = cfg.season_ppm(blk.hdr.ts);
            replay_state.reward_producer_v2(
                &prod_acct,
                cfg.block_reward,
                cfg.pwm_stake_min,
                season_ppm,
            );
        }
        if blk.hdr.state_root != digest(&replay_state) {
            break;
        }
        last_good_h = h;
        last_good_hash = hex::encode(hdr_hash(&blk.hdr));
        prev = hdr_hash(&blk.hdr);
    }
    if let Some(h) = target_h {
        if h > last_good_h {
            return Err(format!(
                "repair replay failed at target {h}; reproducible only to {last_good_h}"
            ));
        }
    }
    Ok(ReplayRes {
        last_good_h,
        last_good_hash,
        state: replay_state,
    })
}

fn rewrite_epochs(summary_path: &Path, blocks: &[Block], target_h: u64) -> Result<(), String> {
    let epochs_dir = summary_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("epochs");
    fs::create_dir_all(&epochs_dir).map_err(|e| format!("repair epochs mkdir: {e}"))?;
    for (_, path) in epoch_files(summary_path)? {
        fs::remove_file(&path).map_err(|e| format!("repair remove {}: {e}", path.display()))?;
    }
    let mut per_epoch: BTreeMap<u64, Vec<String>> = BTreeMap::new();
    for blk in blocks {
        if blk.hdr.height > target_h {
            break;
        }
        let idx = epoch_idx(blk.hdr.height)?;
        per_epoch
            .entry(idx)
            .or_default()
            .push(serde_json::to_string(blk).map_err(|e| format!("repair encode block: {e}"))?);
    }
    for (idx, lines) in per_epoch {
        let path = epochs_dir.join(epoch_file_name(idx));
        let mut body = lines.join("\n");
        body.push('\n');
        write_atomic(&path, body)?;
    }
    Ok(())
}

fn rewrite_manifest(
    summary_path: &Path,
    blocks: &[Block],
    target_h: u64,
    tip_hash: &str,
) -> Result<(), String> {
    let mut per_epoch: BTreeMap<u64, (u64, u64)> = BTreeMap::new();
    for blk in blocks {
        if blk.hdr.height > target_h {
            break;
        }
        let idx = epoch_idx(blk.hdr.height)?;
        let row = per_epoch
            .entry(idx)
            .or_insert((blk.hdr.height, blk.hdr.height));
        row.1 = blk.hdr.height;
    }
    let mut epochs = Vec::new();
    for (idx, (first_h, last_h)) in per_epoch {
        epochs.push(EpochMeta {
            idx,
            first_h,
            last_h,
            file_name: epoch_file_name(idx),
        });
    }
    let man = mk_manifest(target_h, tip_hash.to_string(), epochs);
    let body = serde_json::to_string_pretty(&man).map_err(|e| format!("repair manifest: {e}"))?;
    write_atomic(&manifest_file_path(summary_path), body)
}

fn rewrite_summary(
    summary_path: &Path,
    cfg: &GenCfg,
    state: pwm_core::State,
    roaming: SnapshotRoamingWire,
    cross_shard: CrossShardLedger,
    target_h: u64,
) -> Result<(), String> {
    let snap = SnapshotData {
        version: SNAPSHOT_VERSION,
        genesis_accounts: snapshot_genesis_accounts(cfg),
        genesis_anchor: None,
        blocks: vec![],
        state,
        roaming,
        cross_shard,
        blocks_stored: BlocksStored::Epochs,
        checkpoint_height: target_h,
    };
    let txt = encode_snap_data_txt(&snap)?;
    write_atomic(summary_path, txt)
}

fn load_aux_summary(
    summary_path: &Path,
    cfg: &GenCfg,
    target_h: u64,
) -> (SnapshotRoamingWire, CrossShardLedger, bool) {
    let Ok(raw) = fs::read_to_string(summary_path) else {
        return (
            SnapshotRoamingWire::default(),
            CrossShardLedger::default(),
            false,
        );
    };
    let Ok(Some(snap)) = decode_snap_raw(&raw, cfg) else {
        return (
            SnapshotRoamingWire::default(),
            CrossShardLedger::default(),
            false,
        );
    };
    if snap.checkpoint_height == target_h {
        (snap.roaming, snap.cross_shard, true)
    } else {
        (
            SnapshotRoamingWire::default(),
            CrossShardLedger::default(),
            false,
        )
    }
}

fn make_backup(summary_path: &Path) -> Result<PathBuf, String> {
    let root = summary_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("backup clock: {e}"))?
        .as_secs();
    let dst = root.join(format!("repair-backup-{ts}"));
    fs::create_dir_all(&dst).map_err(|e| format!("backup mkdir {}: {e}", dst.display()))?;
    if summary_path.exists() {
        let dst_file = dst.join(
            summary_path
                .file_name()
                .unwrap_or_else(|| OsStr::new("pwm-data.json")),
        );
        fs::copy(summary_path, &dst_file).map_err(|e| {
            format!(
                "backup copy {} -> {}: {e}",
                summary_path.display(),
                dst_file.display()
            )
        })?;
    }
    let epochs = root.join("epochs");
    if epochs.exists() {
        copy_tree(&epochs, &dst.join("epochs"))?;
    }
    Ok(dst)
}

fn copy_tree(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| format!("backup mkdir {}: {e}", dst.display()))?;
    let rd = fs::read_dir(src).map_err(|e| format!("backup read_dir {}: {e}", src.display()))?;
    for ent in rd {
        let ent = ent.map_err(|e| format!("backup entry {}: {e}", src.display()))?;
        let p = ent.path();
        let q = dst.join(ent.file_name());
        if ent
            .file_type()
            .map_err(|e| format!("backup file_type {}: {e}", p.display()))?
            .is_dir()
        {
            copy_tree(&p, &q)?;
        } else {
            fs::copy(&p, &q)
                .map_err(|e| format!("backup copy {} -> {}: {e}", p.display(), q.display()))?;
        }
    }
    Ok(())
}

fn write_atomic(path: &Path, body: String) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("repair mkdir {}: {e}", dir.display()))?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut f = fs::File::create(&tmp).map_err(|e| format!("repair create tmp: {e}"))?;
        f.write_all(body.as_bytes())
            .map_err(|e| format!("repair write tmp: {e}"))?;
        f.sync_all().map_err(|e| format!("repair fsync tmp: {e}"))?;
    }
    fs::rename(&tmp, path).map_err(|e| format!("repair rename {}: {e}", path.display()))?;
    Ok(())
}

fn epoch_files(summary_path: &Path) -> Result<Vec<(u64, PathBuf)>, String> {
    let base = summary_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("epochs");
    if !base.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let rd = fs::read_dir(&base).map_err(|e| format!("repair read_dir {}: {e}", base.display()))?;
    for ent in rd {
        let ent = ent.map_err(|e| format!("repair read_dir entry: {e}"))?;
        if !ent
            .file_type()
            .map_err(|e| format!("repair file_type: {e}"))?
            .is_file()
        {
            continue;
        }
        let name = ent.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with("block_e") || !name.ends_with(".json") {
            continue;
        }
        let idx_txt = name.trim_start_matches("block_e").trim_end_matches(".json");
        let idx = idx_txt
            .parse::<u64>()
            .map_err(|e| format!("repair bad epoch file name {name}: {e}"))?;
        let er = epoch_range(idx);
        out.push((er.idx, ent.path()));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::app_from_dev_net;
    use crate::snapshot::incremental::append_tip_block;
    use crate::snapshot::io::save_checkpoint_summary;
    use pwm_core::hd::domain_of_account_id;
    use pwm_core::tx::{SignedTx, TxBody};

    fn tmp_summary(name: &str) -> PathBuf {
        let sfx = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("{name}-{sfx}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("mkdir");
        dir.join("pwm-data.json")
    }

    fn mk_chain(path: &Path, n: u64) {
        let app = app_from_dev_net();
        for _ in 0..n {
            let mut g = app.inner.try_write().expect("inner");
            g.chain.seal(vec![]).expect("seal");
            append_tip_block(path, &g).expect("append");
            save_checkpoint_summary(path, &g).expect("summary");
        }
    }

    fn corrupt_last(path: &Path) {
        let f = path
            .parent()
            .expect("parent")
            .join("epochs")
            .join("block_e0.json");
        let txt = fs::read_to_string(&f).expect("read");
        let mut lines: Vec<String> = txt.lines().map(ToString::to_string).collect();
        let mut blk: Block = serde_json::from_str(lines.last().expect("tail")).expect("decode");
        blk.hdr.state_root = [7u8; 32];
        *lines.last_mut().expect("tail mut") = serde_json::to_string(&blk).expect("encode");
        fs::write(&f, format!("{}\n", lines.join("\n"))).expect("write");
    }

    #[test]
    fn repair_to_h_after_tail() {
        let path = tmp_summary("pwm-repair-h");
        mk_chain(&path, 5);
        let (cfg, _) = pwm_core::dev_net();
        corrupt_last(&path);
        let bad = load_snapshot(&path, &cfg).expect_err("must fail before repair");
        assert!(
            bad.contains("snapshot"),
            "unexpected pre-repair error text: {bad}"
        );

        let rep = repair_json_epochs(
            &path,
            &cfg,
            SnapRepairOpts {
                target_h: Some(4),
                backup: true,
                dry_run: false,
            },
        )
        .expect("repair");
        assert_eq!(rep.last_good_h, 4);
        assert_eq!(rep.target_h, 4);
        assert!(rep.backup_dir.is_some());

        let fixed = load_snapshot(&path, &cfg).expect("load").expect("snapshot");
        assert_eq!(fixed.blocks.len(), 4);
        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[test]
    fn repair_auto_last_good() {
        let path = tmp_summary("pwm-repair-auto");
        mk_chain(&path, 6);
        let (cfg, _) = pwm_core::dev_net();
        corrupt_last(&path);
        let rep = repair_json_epochs(
            &path,
            &cfg,
            SnapRepairOpts {
                target_h: None,
                backup: false,
                dry_run: false,
            },
        )
        .expect("repair");
        assert_eq!(rep.last_good_h, 5);
        assert_eq!(rep.target_h, 5);
        let fixed = load_snapshot(&path, &cfg).expect("load").expect("snapshot");
        assert_eq!(fixed.blocks.len(), 5);
        let _ = fs::remove_dir_all(path.parent().expect("parent"));
    }

    #[test]
    fn repair_replay_uses_block_ctx() {
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
        let rep = replay_to(&cfg, &blocks, Some(1)).expect("repair replay");
        assert_eq!(rep.last_good_h, 1);
        let acc = rep.state.get(&signer).expect("signer account");
        assert_eq!(acc.marks_last_block, 1);
    }
}
