//! Federation shard snapshot.

use crate::federation::{federation_http_snapshot, FederationShardsOut};
use crate::App;
use axum::{extract::State, http::StatusCode, Json};

use super::common::ensure_ready;

pub(super) async fn v1_federation_shards(
    State(app): State<App>,
) -> Result<Json<FederationShardsOut>, (StatusCode, String)> {
    ensure_ready(&app).await?;
    let now_ms = crate::current_time_ms()?;
    let out = federation_http_snapshot(&app, now_ms).await;
    Ok(Json(out))
}
