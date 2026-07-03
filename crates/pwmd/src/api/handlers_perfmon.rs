//! `/v1/perfmon` returns process-local performance counter snapshots.

use crate::perfmon::{self, PerfSnapshot};
use axum::Json;

// Smoke: curl http://127.0.0.1:<rpc>/v1/perfmon
pub(super) async fn v1_perfmon() -> Json<Vec<PerfSnapshot>> {
    Json(
        perfmon::REGISTRY
            .iter()
            .map(|entity| entity.snapshot())
            .collect(),
    )
}
