//! Epoch files: one block per line (JSONL, `block_e*.jsonl`), atomic manifest.
//!
//! There is no true lazy block cache yet: logs that mention sequential epoch JSONL read measure the
//! disk-bound phase; trimming the in-memory tip is `absorb_blocks_tail` in `lifecycle` (cheap).

use super::epoch::{
    ensure_epoch_man_schema, epoch_file_name, epoch_file_path, epoch_idx, epoch_range,
    manifest_file_path, mk_manifest, EpochManifest, EpochMeta,
};
use super::telemetry::SNAP_STARTUP_TARGET;
use crate::state::Inner;
use pwm_core::block::{hdr_hash, Block};
use std::fs;
use std::io::{BufRead, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::Instant;

const TAIL_WINDOW: u64 = 128 * 1024;

/// Lines hold one [`Block`] JSON per line (JSONL). `.json` suffix is historical; content is newline-delimited JSON.
pub(crate) fn append_block_for_epoch(summary_path: &Path, blk: &Block) -> Result<(), String> {
    let h = blk.hdr.height;
    let eidx = epoch_idx(h)?;
    let epoch_path = epoch_file_path(summary_path, eidx);
    let er = epoch_range(eidx);
    if let Some(dir) = epoch_path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("epochs mkdir: {e}"))?;
    }
    let last_h = read_last_block_height(&epoch_path)?;
    if last_h.is_none() {
        if h != er.first_h {
            cleanup_empty_gap_file(&epoch_path, h, er.first_h)?;
            return Err(format!(
                "epoch append gap: first height in file must be {}, got {}",
                er.first_h, h
            ));
        }
    } else {
        let prev_h = h
            .checked_sub(1)
            .ok_or_else(|| "epoch append: height 0 has no previous height".to_string())?;
        let tail_h = last_h.expect("last height present after is_none=false");
        if tail_h != prev_h {
            return Err(format!(
                "epoch append: want prev height {}, file tail height {}",
                prev_h, tail_h
            ));
        }
    }
    let mut man = if let Some(m) = load_manifest(summary_path)? {
        m
    } else {
        mk_manifest(h, hex::encode(hdr_hash(&blk.hdr)), vec![])
    };
    let first_h = if last_h.is_none() {
        er.first_h
    } else {
        man.epochs
            .iter()
            .find(|m| m.idx == eidx)
            .map(|m| m.first_h)
            .ok_or_else(|| format!("epoch append: existing file missing manifest row {eidx}"))?
    };
    let line = serde_json::to_string(blk).map_err(|e| format!("encode block: {e}"))?;
    {
        let mut f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&epoch_path)
            .map_err(|e| format!("epoch append open: {e}"))?;
        f.write_all(line.as_bytes())
            .and_then(|_| f.write_all(b"\n"))
            .map_err(|e| format!("epoch append write: {e}"))?;
        f.flush().map_err(|e| format!("epoch append flush: {e}"))?;
        f.sync_all()
            .map_err(|e| format!("epoch append fsync: {e}"))?;
    }

    let tip_hash = hex::encode(hdr_hash(&blk.hdr));
    let fname = epoch_file_name(eidx);
    let meta = EpochMeta {
        idx: eidx,
        first_h,
        last_h: h,
        file_name: fname,
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

fn cleanup_empty_gap_file(epoch_path: &Path, h: u64, first_h: u64) -> Result<(), String> {
    match fs::metadata(epoch_path) {
        Ok(meta) if meta.len() == 0 => {
            fs::remove_file(epoch_path)
                .map_err(|e| format!("epoch append gap cleanup {}: {e}", epoch_path.display()))?;
            tracing::warn!(
                path = %epoch_path.display(),
                height = h,
                first_h,
                "removed empty epoch file after append gap"
            );
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("stat {}: {e}", epoch_path.display())),
    }
    Ok(())
}

fn read_last_block_height(p: &Path) -> Result<Option<u64>, String> {
    let mut f = match fs::File::open(p) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("open {}: {e}", p.display())),
    };
    let len = f
        .metadata()
        .map_err(|e| format!("stat {}: {e}", p.display()))?
        .len();
    if len == 0 {
        return Ok(None);
    }

    let start = len.saturating_sub(TAIL_WINDOW);
    f.seek(SeekFrom::Start(start))
        .map_err(|e| format!("seek {} tail: {e}", p.display()))?;
    let mut tail = Vec::with_capacity((len - start) as usize);
    f.read_to_end(&mut tail)
        .map_err(|e| format!("read {} tail: {e}", p.display()))?;

    let Some(end) = tail.iter().rposition(|b| !b.is_ascii_whitespace()) else {
        if start > 0 {
            return Err(format!(
                "epoch file tail line starts before {TAIL_WINDOW} byte window: {}",
                p.display()
            ));
        }
        return Ok(None);
    };
    let line_start = match tail[..end].iter().rposition(|b| *b == b'\n') {
        Some(i) => i + 1,
        None if start == 0 => 0,
        None => {
            return Err(format!(
                "epoch file tail line starts before {TAIL_WINDOW} byte window: {}",
                p.display()
            ))
        }
    };
    let line = std::str::from_utf8(&tail[line_start..=end])
        .map_err(|e| format!("epoch file corrupt tail utf-8: {e}"))?;
    let last: Block =
        serde_json::from_str(line).map_err(|e| format!("epoch file corrupt tail line: {e}"))?;
    Ok(Some(last.hdr.height))
}

/// Appends the current tip block from RAM (must match [`Inner::chain`](crate::state::Inner::chain) tip height).
#[cfg(test)]
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
/// Syncs epoch files on disk up to the current chain tip.
pub(crate) fn sync_epoch_to_tip(summary_path: &Path, inner: &Inner) -> Result<(), String> {
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
            "sync_epoch_to_tip: appending missing block to epoch files"
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
    ensure_epoch_man_schema(man.schema_v)?;
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

/// Loads up to `limit` consecutive blocks starting from `from_h` using epoch metadata windows.
pub(crate) fn load_cons_blocks_epochs(
    summary_path: &Path,
    from_h: u64,
    limit: usize,
) -> Result<Vec<Block>, String> {
    if from_h == 0 || limit == 0 {
        return Ok(vec![]);
    }
    let man = read_epoch_manifest(summary_path)?
        .ok_or_else(|| "epoch consecutive load: missing manifest".to_string())?;
    ensure_epoch_man_schema(man.schema_v)?;
    let base = summary_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("epochs");
    let end_h = from_h.saturating_add(limit as u64).saturating_sub(1);
    let mut out = Vec::with_capacity(limit);
    let mut next_h = from_h;
    for em in &man.epochs {
        if next_h > end_h {
            break;
        }
        if em.last_h < next_h {
            continue;
        }
        if em.first_h > next_h {
            return Err(format!(
                "epoch consecutive load: missing height {} before epoch {}",
                next_h, em.idx
            ));
        }
        let read_from_h = next_h.max(em.first_h);
        let read_to_h = end_h.min(em.last_h);
        let line_start = usize::try_from(read_from_h.saturating_sub(em.first_h))
            .map_err(|_| "epoch consecutive load: line_start overflow".to_string())?;
        let line_count = usize::try_from(read_to_h.saturating_sub(read_from_h).saturating_add(1))
            .map_err(|_| "epoch consecutive load: line_count overflow".to_string())?;
        let p = base.join(&em.file_name);
        let chunk = read_jsonl_range(&p, line_start, line_count)?;
        if chunk.len() != line_count {
            return Err(format!(
                "epoch consecutive load: file {} returned {} lines, want {}",
                em.file_name,
                chunk.len(),
                line_count
            ));
        }
        for b in chunk {
            if b.hdr.height != next_h {
                return Err(format!(
                    "epoch consecutive load: continuity break at {}, got {}",
                    next_h, b.hdr.height
                ));
            }
            out.push(b);
            next_h = next_h.saturating_add(1);
            if next_h > end_h {
                break;
            }
        }
    }
    if out.is_empty() || out.first().map(|b| b.hdr.height) != Some(from_h) {
        return Err(format!(
            "epoch consecutive load: missing start height {}",
            from_h
        ));
    }
    Ok(out)
}

pub(crate) use load_cons_blocks_epochs as load_consecutive_blocks_from_epochs;

/// Expensive compatibility path for legacy peers that request blocks by hashes only.
pub(crate) fn load_hash_scan_blocks(
    summary_path: &Path,
    hashes: &[String],
) -> Result<Vec<Option<Block>>, String> {
    if hashes.is_empty() {
        return Ok(vec![]);
    }
    let man = read_epoch_manifest(summary_path)?
        .ok_or_else(|| "epoch hash scan: missing manifest".to_string())?;
    ensure_epoch_man_schema(man.schema_v)?;
    let mut out = vec![None; hashes.len()];
    let mut need = hashes.len();
    let base = summary_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("epochs");
    for em in &man.epochs {
        if need == 0 {
            break;
        }
        let p = base.join(&em.file_name);
        let lines = read_jsonl_lines(&p)?;
        for (line_i, line) in lines.iter().enumerate() {
            let blk: Block = serde_json::from_str(line).map_err(|e| {
                format!(
                    "epoch {} line {}: decode block: {}",
                    em.file_name, line_i, e
                )
            })?;
            let got = hex::encode(hdr_hash(&blk.hdr));
            for (ix, want) in hashes.iter().enumerate() {
                if out[ix].is_none() && *want == got {
                    out[ix] = Some(blk.clone());
                    need = need.saturating_sub(1);
                }
            }
            if need == 0 {
                break;
            }
        }
    }
    Ok(out)
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

fn read_jsonl_range(p: &Path, start: usize, count: usize) -> Result<Vec<Block>, String> {
    if count == 0 {
        return Ok(vec![]);
    }
    if !p.exists() {
        return Ok(vec![]);
    }
    let f = fs::File::open(p).map_err(|e| format!("open {}: {e}", p.display()))?;
    let mut out = Vec::with_capacity(count);
    let end = start.saturating_add(count);
    for (line_i, line) in std::io::BufReader::new(f).lines().enumerate() {
        if line_i < start {
            continue;
        }
        if line_i >= end {
            break;
        }
        let line = line.map_err(|e| format!("read line {}: {e}", line_i))?;
        if line.trim().is_empty() {
            continue;
        }
        let b: Block = serde_json::from_str(&line)
            .map_err(|e| format!("decode line {} in {}: {}", line_i, p.display(), e))?;
        out.push(b);
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
    ensure_epoch_man_schema(man.schema_v)?;
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
/// Loads tail blocks from the epoch JSONL files.
pub(crate) fn load_tail_blocks(summary_path: &Path, tail_cap: usize) -> Result<Vec<Block>, String> {
    let Some(man) = read_epoch_manifest(summary_path)? else {
        return Err("epoch tail load: missing manifest".into());
    };
    ensure_epoch_man_schema(man.schema_v)?;
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
    ensure_epoch_man_schema(man.schema_v)?;
    let p = manifest_file_path(summary_path);
    if let Some(dir) = p.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("manifest mkdir: {e}"))?;
    }
    let body = serde_json::to_string_pretty(man).map_err(|e| format!("encode manifest: {e}"))?;
    let tmp = p.with_extension("json.tmp"); // manifest остаётся .json, не .jsonl
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
    use crate::block_writer::BlockWriter;
    use crate::bootstrap::app_from_dev_net;
    use crate::snapshot::epoch::epoch_file_path;
    use crate::snapshot::epoch::EPOCH_MAN_SCHEMA_CUR;
    use crate::snapshot::epoch::SNAP_CHK_BLK_IV;
    use crate::snapshot::io::{json_file_seal_persist, save_checkpoint_summary};
    use crate::snapshot::{encode_inner_snap_json, load_snapshot};
    use pwm_core::block::Block;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_case(tag: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let sfx = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("pwm-{tag}-{sfx}"));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let summary = dir.join("pwm-data.json");
        (dir, summary)
    }

    fn sealed_blocks(count: usize) -> Vec<Block> {
        let app = app_from_dev_net();
        let mut out = Vec::with_capacity(count);
        let mut g = app.inner.try_write().expect("inner");
        for _ in 0..count {
            g.chain.seal(vec![]).expect("seal");
            out.push(g.chain.blocks.back().expect("tip").clone());
        }
        out
    }

    #[test]
    fn epoch_gap_mid_start() {
        let (dir, summary) = temp_case("epoch-gap-mid-start");
        let app = app_from_dev_net();
        let block_30 = {
            let mut inner = app.inner.try_write().expect("inner");
            for _ in 0..29 {
                inner.chain.seal(vec![]).expect("seal");
            }
            sync_epoch_to_tip(&summary, &inner).expect("sync to 29");
            inner.chain.seal(vec![]).expect("seal 30");
            inner.chain.blocks.back().expect("block 30").clone()
        };

        let writer = BlockWriter::new(summary.clone()).expect("writer");
        writer.enqueue(Arc::new(block_30)).expect("enqueue");
        writer.flush().expect("flush");
        writer.shutdown().expect("shutdown");

        let blocks = load_blocks_from_epochs(&summary).expect("load");
        assert_eq!(blocks.len(), 30);
        assert_eq!(blocks.last().map(|block| block.hdr.height), Some(30));
        std::fs::remove_dir_all(dir).expect("cleanup");
    }

    #[test]
    fn append_continuity() {
        let (dir, summary) = temp_case("append-continuity");
        let blocks = sealed_blocks(2);
        append_block_for_epoch(&summary, &blocks[0]).expect("append one");
        append_block_for_epoch(&summary, &blocks[1]).expect("append two");

        let epoch = epoch_file_path(&summary, 0);
        assert_eq!(read_last_block_height(&epoch).expect("tail"), Some(2));
        let man = read_epoch_manifest(&summary)
            .expect("manifest")
            .expect("present");
        assert_eq!(man.epochs[0].first_h, 1);
        assert_eq!(man.epochs[0].last_h, 2);
        assert_eq!(read_jsonl_lines(&epoch).expect("lines").len(), 2);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_duplicate_gap() {
        let (dir, summary) = temp_case("append-reject");
        let blocks = sealed_blocks(3);
        append_block_for_epoch(&summary, &blocks[0]).expect("append one");

        let duplicate = append_block_for_epoch(&summary, &blocks[0]).expect_err("duplicate");
        assert!(duplicate.contains("want prev height 0"), "{duplicate}");
        let gap = append_block_for_epoch(&summary, &blocks[2]).expect_err("gap");
        assert!(gap.contains("want prev height 2"), "{gap}");
        assert_eq!(
            read_jsonl_lines(&epoch_file_path(&summary, 0))
                .unwrap()
                .len(),
            1
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn legacy_arrays_read() {
        let (dir, summary) = temp_case("legacy-arrays");
        let block = sealed_blocks(1).remove(0);
        let mut value = serde_json::to_value(&block).expect("value");
        let hdr = value["hdr"].as_object_mut().expect("header object");
        hdr.insert("prev_hash".into(), serde_json::json!(block.hdr.prev_hash));
        hdr.insert("tx_root".into(), serde_json::json!(block.hdr.tx_root));
        hdr.insert("state_root".into(), serde_json::json!(block.hdr.state_root));
        hdr.insert("sig".into(), serde_json::json!(block.hdr.sig.as_slice()));
        let epoch = epoch_file_path(&summary, 0);
        std::fs::create_dir_all(epoch.parent().expect("epoch parent")).expect("mkdir epochs");
        std::fs::write(
            &epoch,
            format!("{}\n", serde_json::to_string(&value).unwrap()),
        )
        .expect("write legacy");

        assert_eq!(
            read_last_block_height(&epoch).expect("legacy tail"),
            Some(1)
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn tail_window_bounds() {
        let (dir, summary) = temp_case("tail-window");
        let block = sealed_blocks(1).remove(0);
        let epoch = epoch_file_path(&summary, 0);
        std::fs::create_dir_all(epoch.parent().expect("epoch parent")).expect("mkdir epochs");
        let line = serde_json::to_string(&block).expect("encode");
        std::fs::write(&epoch, format!("{line}\n\n  \r\n")).expect("write tail");
        assert_eq!(
            read_last_block_height(&epoch).expect("spaced tail"),
            Some(1)
        );

        std::fs::write(&epoch, vec![b'x'; TAIL_WINDOW as usize + 1]).expect("write long");
        let err = read_last_block_height(&epoch).expect_err("bounded tail");
        assert!(err.contains("starts before 131072 byte window"), "{err}");
        let _ = std::fs::remove_dir_all(dir);
    }

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
    fn mono_save_sync_disk_lag() {
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
    fn runtime_persist_disk_lag() {
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

    /// `json_file_seal_persist` must converge epoch tail + summary whenever lifecycle calls it.
    #[test]
    fn seal_persist_syncs_tail() {
        let sfx = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("pwm-seal-persist-{sfx}"));
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
            json_file_seal_persist(&pb, &g).expect("seal persist");
        }

        let got = load_snapshot(&pb, &cfg).expect("load").expect("snap");
        assert_eq!(got.blocks.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn epoch_trust_respects_tail_cap() {
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
                anchor_sk: None,
                anchor_idx: 0,
            },
        )
        .expect("timed")
        .0
        .expect("snap");
        assert_eq!(trust.blocks.len(), pwm_core::TAIL_BLOCK_CAP);
        assert_eq!(trust.blocks.last().expect("tip").hdr.height, N);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn epoch_man_v1_tail_ok() {
        let sfx = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("pwm-epoch-man-v1-{sfx}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let pb = dir.join("pwm-data.json");
        let app = app_from_dev_net();
        {
            let mut g = app.inner.try_write().expect("inner");
            g.chain.seal(vec![]).expect("seal");
            append_tip_block(&pb, &g).expect("append");
            save_checkpoint_summary(&pb, &g).expect("summary");
        }
        let got = load_tail_blocks(&pb, 8).expect("v1 accepted");
        assert_eq!(got.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn epoch_man_v2_tail_err() {
        let sfx = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("pwm-epoch-man-v2-{sfx}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let pb = dir.join("pwm-data.json");
        let app = app_from_dev_net();
        {
            let mut g = app.inner.try_write().expect("inner");
            g.chain.seal(vec![]).expect("seal");
            append_tip_block(&pb, &g).expect("append");
            save_checkpoint_summary(&pb, &g).expect("summary");
        }
        let mut man = read_epoch_manifest(&pb)
            .expect("read manifest")
            .expect("manifest exists");
        man.schema_v = EPOCH_MAN_SCHEMA_CUR.saturating_add(1);
        let mp = manifest_file_path(&pb);
        let body = serde_json::to_string_pretty(&man).expect("encode manifest");
        std::fs::write(&mp, body).expect("overwrite manifest");
        let err = load_tail_blocks(&pb, 8).expect_err("must reject");
        assert!(err.contains("unsupported epoch manifest schema"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn trust_load_skips_old_replay() {
        use crate::snapshot::io::{load_snapshot_timed, SnapshotLoadOpts};

        let sfx = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("pwm-trust-skip-replay-{sfx}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("mkdir");
        let pb = dir.join("pwm-data.json");

        let app = app_from_dev_net();
        let (cfg, _) = pwm_core::dev_net();
        const N: u64 = 1105;
        for _ in 0..N {
            let mut g = app.inner.try_write().expect("inner");
            g.chain.seal(vec![]).expect("seal");
            append_tip_block(&pb, &g).expect("append");
        }
        {
            let g = app.inner.try_write().expect("inner");
            save_checkpoint_summary(&pb, &g).expect("tip-aligned summary");
        }

        let ep0 = epoch_file_path(&pb, 0);
        let mut lines = read_jsonl_lines(&ep0).expect("epoch lines");
        assert!(lines.len() >= 2, "need block height 2");
        let mut blk2: Block = serde_json::from_str(&lines[1]).expect("decode block2");
        blk2.hdr.prod_idx = blk2.hdr.prod_idx.saturating_add(1);
        lines[1] = serde_json::to_string(&blk2).expect("encode block2");
        let mut body = lines.join("\n");
        body.push('\n');
        std::fs::write(&ep0, body).expect("tamper block2");

        let (snap, timing) = load_snapshot_timed(
            &pb,
            &cfg,
            SnapshotLoadOpts {
                verify_chain: false,
                anchor_sk: None,
                anchor_idx: 0,
            },
        )
        .expect("trust load");
        let snap = snap.expect("snapshot");
        eprintln!(
            "trust_load_skips_old_replay validate_ms={}",
            timing.validate_ms
        );
        assert_eq!(snap.blocks.len(), pwm_core::TAIL_BLOCK_CAP);
        assert!(!timing.used_full_verify, "trust mode must stay trust");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
