//! Debug helpers for bounded divergence dumps and seal midpoint timing.

use crate::handshake::PWM_PROTOCOL_VERSION;
use crate::state::App;
use pwm_core::block::{hdr_hash, Block};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

const MID_MS: u64 = 500;
const MID_WAIT_CAP_MS: u64 = 750;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DumpWrite {
    Off,
    CapReached,
    Wrote(PathBuf),
}

#[derive(Serialize)]
struct DumpRow<'a> {
    height: u64,
    hash: String,
    source: &'a str,
    node_id: &'a str,
    protocol_version: &'a str,
    block: &'a Block,
}

pub(crate) fn align_mid_on(debug_align_mid: bool, debug_det_seal_time: bool) -> bool {
    debug_align_mid && !debug_det_seal_time
}

pub(crate) fn mid_wait_ms(now_ms: u64) -> u64 {
    let sub = now_ms % 1_000;
    let wait = if sub <= MID_MS {
        MID_MS - sub
    } else {
        1_000 - (sub - MID_MS)
    };
    if wait > MID_WAIT_CAP_MS {
        0
    } else {
        wait
    }
}

pub(crate) fn dump_path(base_dir: &Path, height: u64) -> PathBuf {
    base_dir.join(format!("b{height}.json"))
}

fn dump_dir(app: &App) -> PathBuf {
    if let Some(dir) = app.debug_dump.dir.clone() {
        return dir;
    }
    if let Some(path) = app.data_file.as_ref() {
        if let Some(parent) = path.parent() {
            return parent.join("blocks");
        }
    }
    PathBuf::from("state").join("blocks")
}

pub(crate) fn dump_blk_json(
    app: &App,
    blk: &Block,
    source: &str,
    node_id: &str,
) -> Result<DumpWrite, String> {
    if !app.debug_dump.on_divergence {
        return Ok(DumpWrite::Off);
    }
    let max_files = app.debug_dump.max_files.max(1);
    if app.dump_count.load(Ordering::Relaxed) >= max_files {
        return Ok(DumpWrite::CapReached);
    }
    let dir = dump_dir(app);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("debug dump: create dir {}: {e}", dir.display()))?;
    let final_path = dump_path(&dir, blk.hdr.height);
    let tmp_path = final_path.with_extension("json.tmp");
    let row = DumpRow {
        height: blk.hdr.height,
        hash: hex::encode(hdr_hash(&blk.hdr)),
        source,
        node_id,
        protocol_version: PWM_PROTOCOL_VERSION,
        block: blk,
    };
    let mut file = std::fs::File::create(&tmp_path)
        .map_err(|e| format!("debug dump: create file {}: {e}", tmp_path.display()))?;
    serde_json::to_writer_pretty(&mut file, &row)
        .map_err(|e| format!("debug dump: write json {}: {e}", tmp_path.display()))?;
    std::fs::rename(&tmp_path, &final_path).map_err(|e| {
        format!(
            "debug dump: move {} -> {}: {e}",
            tmp_path.display(),
            final_path.display()
        )
    })?;
    app.dump_count.fetch_add(1, Ordering::Relaxed);
    Ok(DumpWrite::Wrote(final_path))
}

#[cfg(test)]
mod tests {
    use super::{align_mid_on, dump_blk_json, dump_path, mid_wait_ms, DumpWrite};
    use crate::bootstrap::app_from_genesis_id;
    use crate::config::{DebugDumpCfg, GenesisSource};
    use crate::default_runtime_identity_neutral;
    use crate::DevLane;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn dump_path_uses_b_height() {
        let base = PathBuf::from("state").join("blocks");
        assert_eq!(dump_path(&base, 42), base.join("b42.json"));
    }

    #[test]
    fn mid_wait_stays_bounded() {
        assert_eq!(mid_wait_ms(1_000), 500);
        assert_eq!(mid_wait_ms(1_490), 10);
        assert_eq!(mid_wait_ms(1_800), 700);
    }

    #[test]
    fn align_det_wins_over_mid() {
        assert!(align_mid_on(true, false));
        assert!(!align_mid_on(true, true));
        assert!(!align_mid_on(false, false));
    }

    #[test]
    fn div_dump_writes_block_file() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dump_root = std::env::temp_dir().join(format!("pwmd-div-dump-{suffix}"));
        let mut app = app_from_genesis_id(
            &GenesisSource::DevNet,
            DevLane::Lane0,
            Some(dump_root.join("pwm-data.json")),
            Some(default_runtime_identity_neutral()),
        )
        .expect("app");
        app.debug_dump = DebugDumpCfg {
            on_divergence: true,
            dir: Some(dump_root.clone()),
            max_files: 4,
            trigger_streak: 2,
        };
        let blk = {
            let mut g = app.inner.blocking_write();
            g.chain.seal(vec![]).expect("seal");
            g.chain.blocks.back().expect("tip").clone()
        };
        let out = dump_blk_json(&app, &blk, "divergence_probe", "peer-a").expect("dump");
        let path = match out {
            DumpWrite::Wrote(path) => path,
            other => panic!("unexpected dump outcome: {other:?}"),
        };
        let body = std::fs::read_to_string(&path).expect("dump body");
        assert!(body.contains("\"source\": \"divergence_probe\""));
        assert!(body.contains("\"height\": 1"));
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir_all(dump_root);
    }
}
