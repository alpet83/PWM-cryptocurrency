//! Epoch file naming and manifest scaffolding for Json snapshot storage.
//! `EPOCH_SPAN` sizes each on-disk JSONL shard (~fewer files). `SNAP_CHK_BLK_IV` rewrites the
//! summary and (with ClickHouse) inserts checkpoint rows **~10×** more often than a file-epoch
//! boundary (1000/100) because CH has no epoch files and needs dense state anchors.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Checkpoint interval for JsonFile `pwm-data.json` summary and CH `checkpoints__*` rows (seal path).
pub(crate) const SNAP_CHK_BLK_IV: u64 = 100;

/// Max block heights per `epochs/block_e*.json` JSONL file (wider files ⇒ fewer files on disk).
pub(crate) const EPOCH_SPAN: u64 = 1_000;
pub(crate) const EPOCH_MANIFEST_FILE: &str = "pwm-epochs-manifest.json";
pub(crate) const EPOCH_MAN_SCHEMA_CUR: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EpochRange {
    pub(crate) idx: u64,
    pub(crate) first_h: u64,
    pub(crate) last_h: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct EpochManifest {
    pub(crate) schema_v: u32,
    pub(crate) epoch_span: u64,
    pub(crate) canonical_h: u64,
    pub(crate) tip_hash: String,
    pub(crate) epochs: Vec<EpochMeta>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct EpochMeta {
    pub(crate) idx: u64,
    pub(crate) first_h: u64,
    pub(crate) last_h: u64,
    pub(crate) file_name: String,
}

pub(crate) fn epoch_idx(height: u64) -> Result<u64, String> {
    if height == 0 {
        return Err("epoch idx expects height >= 1".into());
    }
    Ok((height - 1) / EPOCH_SPAN)
}

pub(crate) fn epoch_range(idx: u64) -> EpochRange {
    let first_h = idx.saturating_mul(EPOCH_SPAN).saturating_add(1);
    let last_h = first_h.saturating_add(EPOCH_SPAN.saturating_sub(1));
    EpochRange {
        idx,
        first_h,
        last_h,
    }
}

pub(crate) fn epoch_file_name(idx: u64) -> String {
    format!("block_e{idx}.json")
}

pub(crate) fn manifest_file_path(summary_path: &Path) -> PathBuf {
    epochs_dir(summary_path).join(EPOCH_MANIFEST_FILE)
}

pub(crate) fn epoch_file_path(summary_path: &Path, idx: u64) -> PathBuf {
    epochs_dir(summary_path).join(epoch_file_name(idx))
}

pub(crate) fn mk_manifest(
    canonical_h: u64,
    tip_hash: String,
    epochs: Vec<EpochMeta>,
) -> EpochManifest {
    EpochManifest {
        schema_v: EPOCH_MAN_SCHEMA_CUR,
        epoch_span: EPOCH_SPAN,
        canonical_h,
        tip_hash,
        epochs,
    }
}

pub(crate) fn epoch_man_schema_ok(schema_v: u32) -> bool {
    schema_v == EPOCH_MAN_SCHEMA_CUR
}

pub(crate) fn ensure_epoch_man_schema(schema_v: u32) -> Result<(), String> {
    if epoch_man_schema_ok(schema_v) {
        return Ok(());
    }
    Err(format!(
        "unsupported epoch manifest schema {schema_v}; supported schema {}",
        EPOCH_MAN_SCHEMA_CUR
    ))
}

fn epochs_dir(summary_path: &Path) -> PathBuf {
    summary_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("epochs")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn epoch_idx_bound_ok() {
        assert_eq!(epoch_idx(1).expect("h1"), 0);
        assert_eq!(epoch_idx(1_000).expect("h1000"), 0);
        assert_eq!(epoch_idx(1_001).expect("h1001"), 1);
        assert_eq!(epoch_idx(2_000).expect("h2000"), 1);
        assert_eq!(epoch_idx(2_001).expect("h2001"), 2);
    }

    #[test]
    fn epoch_idx_zero_err() {
        assert!(epoch_idx(0).is_err());
    }

    #[test]
    fn epoch_range_map_ok() {
        let e0 = epoch_range(0);
        assert_eq!(e0.first_h, 1);
        assert_eq!(e0.last_h, 1_000);
        let e2 = epoch_range(2);
        assert_eq!(e2.first_h, 2_001);
        assert_eq!(e2.last_h, 3_000);
    }

    #[test]
    fn epoch_name_map_ok() {
        assert_eq!(epoch_file_name(0), "block_e0.json");
        assert_eq!(epoch_file_name(17), "block_e17.json");
    }

    #[test]
    fn epoch_path_map_ok() {
        let base = Path::new("/tmp/pwm-data.json");
        assert_eq!(
            epoch_file_path(base, 3),
            Path::new("/tmp/epochs/block_e3.json")
        );
        assert_eq!(
            manifest_file_path(base),
            Path::new("/tmp/epochs/pwm-epochs-manifest.json")
        );
    }

    #[test]
    fn epoch_man_v1_ok() {
        assert!(epoch_man_schema_ok(EPOCH_MAN_SCHEMA_CUR));
        assert!(ensure_epoch_man_schema(EPOCH_MAN_SCHEMA_CUR).is_ok());
    }

    #[test]
    fn epoch_man_v2_err() {
        let err =
            ensure_epoch_man_schema(EPOCH_MAN_SCHEMA_CUR.saturating_add(1)).expect_err("reject");
        assert!(err.contains("unsupported epoch manifest schema"), "{err}");
        assert!(err.contains(&EPOCH_MAN_SCHEMA_CUR.to_string()), "{err}");
    }
}
