//! Snapshot persistence backend selector (`JsonFile` default; optional ClickHouse prototype).

#[cfg(feature = "clickhouse-snapshot")]
use super::ch_http::SnapChCfg;
use super::io::{self, SnapshotLoadOpts};
use super::telemetry::SnapIoTiming;
use super::types::SnapshotData;
use crate::state::Inner;
use pwm_core::genesis::GenCfg;
use std::path::{Path, PathBuf};
#[cfg(feature = "clickhouse-snapshot")]
use tracing::{debug, warn};

/// Seal-time durability mode for autosnapshot hooks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SealPersistMode {
    /// Background cadence hit (every `SNAP_CHK_BLK_IV` blocks in lifecycle).
    Periodic,
    /// Explicit full flush (graceful shutdown / safe fallback).
    ShutdownFull,
}

#[cfg(feature = "clickhouse-snapshot")]
fn ch_save_seal_fallback(
    c: &SnapChCfg,
    inner: &Inner,
    _mode: SealPersistMode,
) -> Result<(), String> {
    match c.ch_save_seal(inner) {
        Ok(()) => Ok(()),
        Err(e) => {
            if let Some(ref p) = c.json_fallback {
                warn!(
                    target: "pwmd::snapshot",
                    "clickhouse seal failed; json-file fallback path={}: {}",
                    p.display(),
                    e
                );
                // Json fallback must converge disk state to the current tip.
                io::json_file_seal_persist(p.as_path(), inner).map_err(|e2| {
                    format!(
                        "clickhouse seal: {e}; json fallback ({p}): {e2}",
                        p = p.display()
                    )
                })
            } else {
                Err(e)
            }
        }
    }
}

#[cfg(feature = "clickhouse-snapshot")]
fn ch_save_tip_fallback(c: &SnapChCfg, inner: &Inner) -> Result<(), String> {
    match c.ch_save_tip_summary(inner) {
        Ok(()) => Ok(()),
        Err(e) => {
            if let Some(ref p) = c.json_fallback {
                warn!(
                    target: "pwmd::snapshot",
                    "clickhouse tip summary failed; json-file fallback path={}: {}",
                    p.display(),
                    e
                );
                io::save_epochs_sum_tip(p.as_path(), inner).map_err(|e2| {
                    format!(
                        "clickhouse tip: {e}; json fallback ({p}): {e2}",
                        p = p.display()
                    )
                })
            } else {
                Err(e)
            }
        }
    }
}

/// Where chain snapshots are loaded from / flushed to at runtime boundaries.
#[derive(Clone, Debug)]
pub(crate) enum SnapshotBackend {
    JsonFile {
        path: PathBuf,
    },
    #[cfg(feature = "clickhouse-snapshot")]
    ClickHouse(SnapChCfg),
}

impl SnapshotBackend {
    /// Seal-time persistence: JsonFile tip-sync + summary flush; ClickHouse keeps native behavior.
    pub(crate) fn save_seal_persist(
        &self,
        inner: &Inner,
        mode: SealPersistMode,
    ) -> Result<(), String> {
        match self {
            Self::JsonFile { path } => {
                let _ = mode;
                io::json_file_seal_persist(Path::new(path), inner)
            }
            #[cfg(feature = "clickhouse-snapshot")]
            Self::ClickHouse(c) => ch_save_seal_fallback(c, inner, mode),
        }
    }

    /// Tip summary without new block (e.g. relay roaming); JsonFile rewrites `pwm-data.json` only.
    pub(crate) fn save_tip_summary(&self, inner: &Inner) -> Result<(), String> {
        match self {
            Self::JsonFile { path } => io::save_epochs_sum_tip(Path::new(path), inner),
            #[cfg(feature = "clickhouse-snapshot")]
            Self::ClickHouse(c) => c.ch_save_tip_summary(inner),
        }
    }

    /// Builds JSON backend from `Some(path)` (`None` disables autosnapshot hooks).
    pub(crate) fn from_data_file(df: Option<&PathBuf>) -> Option<Self> {
        df.map(|p| Self::JsonFile { path: p.clone() })
    }

    pub(crate) fn load(
        &self,
        cfg: &GenCfg,
        opts: SnapshotLoadOpts,
    ) -> Result<(Option<SnapshotData>, SnapIoTiming), String> {
        match self {
            Self::JsonFile { path } => {
                let (s, j) = io::load_snapshot_timed(Path::new(path), cfg, opts)?;
                Ok((s, SnapIoTiming::Json(j)))
            }
            #[cfg(feature = "clickhouse-snapshot")]
            Self::ClickHouse(c) => {
                // ClickHouse currently reconstructs from stored blocks and performs full replay;
                // JsonFile trust-load options do not weaken CH validation.
                if opts.verify_chain {
                    debug!(
                        target: "pwmd::snapshot",
                        "snapshot verify-chain requested; ClickHouse load already performs full replay"
                    );
                }
                let (s, ch) = c.ch_load(cfg)?;
                Ok((s, SnapIoTiming::Ch(ch)))
            }
        }
    }

    pub(crate) fn save(&self, inner: &Inner) -> Result<(), String> {
        match self {
            Self::JsonFile { path } => io::json_file_runtime_persist(Path::new(path), inner),
            #[cfg(feature = "clickhouse-snapshot")]
            Self::ClickHouse(c) => ch_save_tip_fallback(c, inner),
        }
    }

    pub(crate) fn init_state_path(&self) -> Option<PathBuf> {
        match self {
            Self::JsonFile { path } => Some(path.clone()),
            #[cfg(feature = "clickhouse-snapshot")]
            Self::ClickHouse(c) => c.json_fallback.clone(),
        }
    }

    pub(crate) fn diag_label(&self) -> String {
        match self {
            Self::JsonFile { path } => path.display().to_string(),
            #[cfg(feature = "clickhouse-snapshot")]
            Self::ClickHouse(c) => {
                let mut s = format!(
                    "clickhouse://{}/{}.blocks={} chk={} val_acc={} legacy={} rk={}",
                    c.http_base,
                    c.database,
                    c.table_blocks,
                    c.table_checkpoints,
                    c.table_validators_accept,
                    c.legacy_snapshot_table,
                    c.row_key
                );
                if let Some(ref p) = c.json_fallback {
                    s.push_str(&format!(" json_fallback={}", p.display()));
                }
                s
            }
        }
    }
}
