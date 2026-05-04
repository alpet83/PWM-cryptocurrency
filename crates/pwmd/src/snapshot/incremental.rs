//! Json epoch files: one block per line (JSONL bytes inside `block_e*.json`), atomic manifest.
//!
//! There is no true lazy block cache yet: logs that mention sequential epoch JSONL read measure the
//! disk-bound phase; trimming the in-memory tip is `absorb_blocks_tail` in `lifecycle` (cheap).

use super::epoch::{
    epoch_file_name, epoch_file_path, epoch_idx, epoch_range, manifest_file_path, mk_manifest,
    EpochManifest, EpochMeta,
};
use super::telemetry::SNAP_STARTUP_TARGET;
use crate::state::Inner;
use pwm_core::block::{hdr_hash, Block};
use std::fs;
use std::io::{BufRead, Write};
use std::path::Path;
use std::time::Instant;

/// Lines hold one [`Block`] JSON per line (JSONL). `.json` suffix is historical; content is newline-delimited JSON.
pub(crate) fn append_block_for_epoch(summary_path: &Path, blk: &Block) -> Result<(), String> {
    let h = blk.hdr.height;
    let eidx = epoch_idx(h)?;
    let epoch_path = epoch_file_path(summary_path, eidx);
    let er = epoch_range(eidx);
    if let Some(dir) = epoch_path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("epochs mkdir: {e}"))?;
    }
    let lines = read_jsonl_lines(&epoch_path)?;
    if lines.is_empty() {
        if h != er.first_h {
            return Err(format!(
                "epoch append: first height in file must be {}, got {}",
                er.first_h, h
            ));
        }
    } else {
        let last: Block =
            serde_json::from_str(lines.last().expect("lines non-empty after is_empty=false"))
                .map_err(|e| format!("epoch file corrupt tail line: {e}"))?;
        if last.hdr.height != h - 1 {
            return Err(format!(
                "epoch append: want prev height {}, file tail height {}",
                h - 1,
                last.hdr.height
            ));
        }
    }
    let line = serde_json::to_string(blk).map_err(|e| format!("encode block: {e}"))?;
    let mut new_body = String::new();
    for (i, l) in lines.iter().enumerate() {
        if i > 0 {
            new_body.push('\n');
        }
        new_body.push_str(l);
    }
    if !new_body.is_empty() {
        new_body.push('\n');
    }
    new_body.push_str(&line);
    new_body.push('\n');

    let tmp_ep = epoch_path.with_extension("json.tmp");
    {
        let mut f = fs::File::create(&tmp_ep).map_err(|e| format!("epoch tmp create: {e}"))?;
        f.write_all(new_body.as_bytes())
            .map_err(|e| format!("epoch tmp write: {e}"))?;
        f.sync_all().map_err(|e| format!("epoch tmp fsync: {e}"))?;
    }
    fs::rename(&tmp_ep, &epoch_path).map_err(|e| format!("epoch rename: {e}"))?;

    let tip_hash = hex::encode(hdr_hash(&blk.hdr));
    let fname = epoch_file_name(eidx);
    let first_h = if lines.is_empty() {
        h
    } else {
        let fst: Block = serde_json::from_str(&lines[0])
            .map_err(|e| format!("epoch file corrupt head line: {e}"))?;
        fst.hdr.height
    };
    let meta = EpochMeta {
        idx: eidx,
        first_h,
        last_h: h,
        file_name: fname,
    };

    let mut man = if let Some(m) = load_manifest(summary_path)? {
        m
    } else {
        mk_manifest(h, tip_hash.clone(), vec![meta.clone()])
    };
    man.canonical_h = h;
    man.tip_hash = tip_hash;
    man.epoch_span = super::epoch::EPOCH_SPAN;
    if let Some(row) = man.epochs.iter_mut().find(|m| m.idx == eidx) {
        *row = meta;
    } else {
        man.epochs.push(meta);
        man.epochs.sort_by_key(|m| m.idx);
    }
    write_manifest(summary_path, &man)?;
    Ok(())
}

/// Appends the current tip block from RAM (must match [`Inner::chain`](crate::state::Inner::chain) tip height).
pub(crate) fn append_tip_block(summary_path: &Path, inner: &Inner) -> Result<(), String> {
    let blk = inner
        .chain
        .blocks
        .back()
        .ok_or_else(|| "epoch append: missing tip block".to_string())?;
    let h = blk.hdr.height;
    if h != inner.chain.tip_h() {
        return Err("epoch append: tip height mismatch".into());
    }
    append_block_for_epoch(summary_path, blk)
}

/// Brings epoch JSONL / manifest on disk up to [`Chain::tip_h`] using blocks still present in the tail deque.
/// Used before monolithic snapshot encode when the seal loop has not yet flushed the latest block(s).
pub(crate) fn sync_epoch_disk_to_tip(summary_path: &Path, inner: &Inner) -> Result<(), String> {
    let tip = inner.chain.tip_h();
    if tip == 0 {
        return Ok(());
    }
    loop {
        let disk_tip = load_manifest(summary_path)?
            .map(|m| m.canonical_h)
            .unwrap_or(0);
        if disk_tip >= tip {
            return Ok(());
        }
        let want_h = disk_tip + 1;
        let blk = inner.chain.blocks.iter().find(|b| b.hdr.height == want_h).ok_or_else(|| {
            format!(
                "epoch sync: need block height {want_h} on disk but it is not in memory tail (disk_tip={disk_tip} tip={tip})"
            )
        })?;
        tracing::debug!(
            target: "pwmd::snapshot",
            disk_tip,
            want_h,
            tip,
            "sync_epoch_disk_to_tip: appending missing block to epoch files"
        );
        append_block_for_epoch(summary_path, blk)?;
    }
}

pub(crate) fn load_blocks_from_epochs(summary_path: &Path) -> Result<Vec<Block>, String> {
    let mp = manifest_file_path(summary_path);
    if !mp.exists() {
        return Ok(vec![]);
    }
    let raw = fs::read_to_string(&mp).map_err(|e| format!("read epoch manifest: {e}"))?;
    let man: EpochManifest =
        serde_json::from_str(&raw).map_err(|e| format!("parse epoch manifest: {e}"))?;
    if man.schema_v != 1 {
        return Err(format!(
            "unsupported epoch manifest schema {}",
            man.schema_v
        ));
    }
    let base = summary_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("epochs");
    let mut all: Vec<Block> = Vec::new();
    let mut epochs_ms: u128 = 0;
    for em in &man.epochs {
        let p = base.join(&em.file_name);
        let ep_t = Instant::now();
        let lines = read_jsonl_lines(&p)?;
        let ep_ms = ep_t.elapsed().as_millis();
        epochs_ms += ep_ms;
        let line_n = lines.len();
        tracing::debug!(
            target: SNAP_STARTUP_TARGET,
            path = %p.display(),
            epoch_idx = em.idx,
            lines = line_n,
            ms = ep_ms as u64,
            epochs_ms_cumulative = epochs_ms as u64,
            "epoch jsonl read (sequential; not lazy cache)"
        );
        for (line_i, line) in lines.iter().enumerate() {
            let b: Block = serde_json::from_str(line).map_err(|e| {
                format!(
                    "epoch {} line {}: decode block: {}",
                    em.file_name, line_i, e
                )
            })?;
            all.push(b);
        }
    }
    if man.canonical_h == 0 {
        if !all.is_empty() {
            return Err("epoch manifest: canonical_h=0 but epoch files non-empty".into());
        }
        return Ok(all);
    }
    if all.len() as u64 != man.canonical_h {
        return Err(format!(
            "epoch replay: got {} blocks, manifest canonical_h {}",
            all.len(),
            man.canonical_h
        ));
    }
    for (i, b) in all.iter().enumerate() {
        let want = (i as u64) + 1;
        if b.hdr.height != want {
            return Err(format!(
                "epoch replay: block pos {} has height {}, want {}",
                i, b.hdr.height, want
            ));
        }
    }
    Ok(all)
}

fn read_jsonl_lines(p: &Path) -> Result<Vec<String>, String> {
    if !p.exists() {
        return Ok(vec![]);
    }
    let f = fs::File::open(p).map_err(|e| format!("open {}: {e}", p.display()))?;
    let mut out = Vec::new();
    for (i, line) in std::io::BufReader::new(f).lines().enumerate() {
        let line = line.map_err(|e| format!("read line {}: {e}", i))?;
        if line.trim().is_empty() {
            continue;
        }
        out.push(line);
    }
    Ok(out)
}

fn load_manifest(summary_path: &Path) -> Result<Option<EpochManifest>, String> {
    let p = manifest_file_path(summary_path);
    if !p.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&p).map_err(|e| format!("read manifest: {e}"))?;
    Ok(Some(
        serde_json::from_str(&raw).map_err(|e| format!("parse manifest: {e}"))?,
    ))
}

pub(crate) fn read_epoch_manifest(summary_path: &Path) -> Result<Option<EpochManifest>, String> {
    load_manifest(summary_path)
}

/// Single block from epoch JSONL (linear scan; used for trusted-load header linkage).
pub(crate) fn load_block_at_height(
    summary_path: &Path,
    height: u64,
) -> Result<Option<Block>, String> {
    if height == 0 {
        return Ok(None);
    }
    let Some(man) = read_epoch_manifest(summary_path)? else {
        return Ok(None);
    };
    if man.schema_v != 1 {
        return Err(format!(
            "unsupported epoch manifest schema {}",
            man.schema_v
        ));
    }
    let base = summary_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("epochs");
    for em in &man.epochs {
        if em.last_h < height || em.first_h > height {
            continue;
        }
        let p = base.join(&em.file_name);
        let lines = read_jsonl_lines(&p)?;
        for (line_i, line) in lines.iter().enumerate() {
            let b: Block = serde_json::from_str(line).map_err(|e| {
                format!(
                    "epoch {} line {}: decode block: {}",
                    em.file_name, line_i, e
                )
            })?;
            if b.hdr.height == height {
                return Ok(Some(b));
            }
        }
        return Err(format!(
            "epoch file {} missing height {} (meta {:?})",
            em.file_name, height, em
        ));
    }
    Ok(None)
}

/// Loads only heights `[tip - tail_cap + 1, tip]` from epoch JSONL (manifest-authoritative tip).
pub(crate) fn load_tail_blocks_from_epochs(
    summary_path: &Path,
    tail_cap: usize,
) -> Result<Vec<Block>, String> {
    let Some(man) = read_epoch_manifest(summary_path)? else {
        return Err("epoch tail load: missing manifest".into());
    };
    if man.schema_v != 1 {
        return Err(format!(
            "unsupported epoch manifest schema {}",
            man.schema_v
        ));
    }
    let tip = man.canonical_h;
    if tip == 0 {
        if man.epochs.iter().any(|e| e.last_h > 0) {
            return Err("epoch tail load: canonical_h=0 but epochs non-empty".into());
        }
        return Ok(vec![]);
    }
    let start_h = tip.saturating_sub(tail_cap as u64).saturating_add(1).max(1);
    let expect_n = (tip - start_h + 1) as usize;
    let base = summary_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("epochs");
    let mut out: Vec<Block> = Vec::with_capacity(expect_n);
    for em in &man.epochs {
        if em.last_h < start_h {
            continue;
        }
        if em.first_h > tip {
            break;
        }
        let p = base.join(&em.file_name);
        let lines = read_jsonl_lines(&p)?;
        for (line_i, line) in lines.iter().enumerate() {
            let b: Block = serde_json::from_str(line).map_err(|e| {
                format!(
                    "epoch {} line {}: decode block: {}",
                    em.file_name, line_i, e
                )
            })?;
            let h = b.hdr.height;
            if h < start_h {
                continue;
            }
            if h > tip {
                return Err(format!(
                    "epoch tail load: block height {} beyond manifest tip {}",
                    h, tip
                ));
            }
            out.push(b);
        }
    }
    if out.len() != expect_n {
        return Err(format!(
            "epoch tail load: want {} blocks (heights {}..={}), got {}",
            expect_n,
            start_h,
            tip,
            out.len()
        ));
    }
    for (i, b) in out.iter().enumerate() {
        let want_h = start_h + i as u64;
        if b.hdr.height != want_h {
            return Err(format!(
                "epoch tail load: pos {} height {} want {}",
                i, b.hdr.height, want_h
            ));
        }
    }
    Ok(out)
}

fn write_manifest(summary_path: &Path, man: &EpochManifest) -> Result<(), String> {
    let p = manifest_file_path(summary_path);
    if let Some(dir) = p.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("manifest mkdir: {e}"))?;
    }
    let body = serde_json::to_string_pretty(man).map_err(|e| format!("encode manifest: {e}"))?;
    let tmp = p.with_extension("json.tmp");
    {
        let mut f = fs::File::create(&tmp).map_err(|e| format!("manifest tmp: {e}"))?;
        f.write_all(body.as_bytes())
            .map_err(|e| format!("manifest write: {e}"))?;
        f.sync_all().map_err(|e| format!("manifest fsync: {e}"))?;
    }
    fs::rename(&tmp, &p).map_err(|e| format!("manifest rename: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bootstrap::app_from_dev_net;
    use crate::snapshot::epoch::SNAP_CHK_BLK_IV;
    use crate::snapshot::io::save_checkpoint_summary;
    use crate::snapshot::{encode_inner_snap_json, load_snapshot};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn legacy_inline_load_roundtrip() {
        let sfx = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let pb = std::env::temp_dir().join(format!("pwm-legacy-snap-{sfx}.json"));
        let app = app_from_dev_net();
        {
            let mut g = app.inner.try_write().expect("inner lock");
            g.chain.seal(vec![]).expect("seal");
            let txt = encode_inner_snap_json(&g, Some(&pb)).expect("encode");
            std::fs::write(&pb, txt).expect("write");
        }
        let (cfg, _) = pwm_core::dev_net();
        let got = load_snapshot(&pb, &cfg).expect("load").expect("some");
        assert_eq!(got.blocks.len(), 1);
        let _ = std::fs::remove_file(&pb);
    }

    #[test]
    fn epochs_save_reload_span_chk() {
        let sfx = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("pwm-epoch-snap-dir-{sfx}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let pb = dir.join("pwm-data.json");

        let app = app_from_dev_net();
        let (cfg, _) = pwm_core::dev_net();

        for _ in 0..105 {
            let mut g = app.inner.try_write().expect("inner");
            g.chain.seal(vec![]).expect("seal");
            append_tip_block(&pb, &g).expect("append");
            let h = g.chain.tip_h();
            if h > 0 && h % SNAP_CHK_BLK_IV == 0 {
                save_checkpoint_summary(&pb, &g).expect("chk");
            }
        }

        let got = load_snapshot(&pb, &cfg)
            .expect("load")
            .expect("expected snapshot");
        assert_eq!(got.blocks.len(), 105);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Regression: API-only seal + `save_snapshot` must flush epoch tail (same invariant as background `json_file_seal_persist`).
    #[test]
    fn monolithic_save_syncs_when_disk_behind_memory() {
        let sfx = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("pwm-epoch-sync-{sfx}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let pb = dir.join("pwm-data.json");

        let app = app_from_dev_net();
        let (cfg, _) = pwm_core::dev_net();

        {
            let mut g = app.inner.try_write().expect("inner");
            g.chain.seal(vec![]).expect("seal");
            append_tip_block(&pb, &g).expect("append disk=1");
            g.chain.seal(vec![]).expect("seal mem=2 disk still 1");
            crate::snapshot::io::save_snapshot(&pb, &g).expect("monolithic save must sync epochs");
        }

        let got = load_snapshot(&pb, &cfg).expect("load").expect("snap");
        assert_eq!(got.blocks.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `json_file_runtime_persist` (API hot path) must not require monolithic encode / full epoch re-read.
    #[test]
    fn runtime_persist_after_disk_lag_loads() {
        let sfx = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("pwm-rt-persist-{sfx}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let pb = dir.join("pwm-data.json");

        let app = app_from_dev_net();
        let (cfg, _) = pwm_core::dev_net();

        {
            let mut g = app.inner.try_write().expect("inner");
            g.chain.seal(vec![]).expect("seal");
            append_tip_block(&pb, &g).expect("append disk=1");
            g.chain.seal(vec![]).expect("seal mem=2 disk still 1");
            crate::snapshot::io::json_file_runtime_persist(&pb, &g).expect("runtime persist");
        }

        let got = load_snapshot(&pb, &cfg).expect("load").expect("snap");
        assert_eq!(got.blocks.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn epoch_trust_load_respects_tail_cap() {
        use crate::snapshot::io::{load_snapshot_timed, SnapshotLoadOpts};

        let sfx = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("pwm-epoch-trust-tail-{sfx}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let pb = dir.join("pwm-data.json");

        let app = app_from_dev_net();
        let (cfg, _) = pwm_core::dev_net();
        const N: u64 = 1100;
        for _ in 0..N {
            let mut g = app.inner.try_write().expect("inner");
            g.chain.seal(vec![]).expect("seal");
            append_tip_block(&pb, &g).expect("append");
            let h = g.chain.tip_h();
            if h > 0 && h % SNAP_CHK_BLK_IV == 0 {
                save_checkpoint_summary(&pb, &g).expect("chk");
            }
        }
        {
            let g = app.inner.try_write().expect("inner");
            save_checkpoint_summary(&pb, &g).expect("tip-aligned summary");
        }

        let full = load_snapshot(&pb, &cfg).expect("load").expect("snap");
        assert_eq!(full.blocks.len() as u64, N);

        let trust = load_snapshot_timed(
            &pb,
            &cfg,
            SnapshotLoadOpts {
                verify_chain: false,
            },
        )
        .expect("timed")
        .0
        .expect("snap");
        assert_eq!(trust.blocks.len(), pwm_core::TAIL_BLOCK_CAP);
        assert_eq!(trust.blocks.last().expect("tip").hdr.height, N);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
