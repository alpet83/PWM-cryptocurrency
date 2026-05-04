//! Tracing target and millisecond breakdown for primary snapshot startup (`spawn_snapshot_loader`).

/// Single filterable tracing target for startup snapshot load (see `pwmd::lifecycle`).
pub const SNAP_STARTUP_TARGET: &str = "pwmd::startup::snapshot";

/// Timings for JSON-file snapshot load (`io::load_snapshot_timed`).
#[derive(Clone, Debug, Default)]
pub(crate) struct JsonSnapTiming {
    pub(crate) summary_read_ms: u64,
    pub(crate) epochs_ms: u64,
    pub(crate) validate_ms: u64,
}

/// Timings for ClickHouse snapshot load (`ch_http::SnapChCfg::ch_load`).
#[cfg(feature = "clickhouse-snapshot")]
#[derive(Clone, Debug)]
pub(crate) struct ChSnapTiming {
    pub(crate) http_ms: u64,
    pub(crate) parse_ms: u64,
    pub(crate) branch: &'static str,
}

#[cfg(feature = "clickhouse-snapshot")]
impl Default for ChSnapTiming {
    fn default() -> Self {
        Self {
            http_ms: 0,
            parse_ms: 0,
            branch: "none",
        }
    }
}

/// Backend-specific I/O phase timings until [`crate::snapshot::SnapshotData`] is ready for `into_runtime`.
#[derive(Clone, Debug)]
pub(crate) enum SnapIoTiming {
    Json(JsonSnapTiming),
    #[cfg(feature = "clickhouse-snapshot")]
    Ch(ChSnapTiming),
}
