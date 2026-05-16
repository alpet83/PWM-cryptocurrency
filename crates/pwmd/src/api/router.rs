//! HTTP route wiring for `/v1/*`.

use crate::App;
use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};
use tower_http::cors::CorsLayer;

use super::handlers_account::{v1_account, v1_accounts};
use super::handlers_backfill::{v1_cross_shard_backfill, v1_cross_shard_facts};
use super::handlers_bridge::v1_bridge_federation_reset;
use super::handlers_federation::v1_federation_shards;
use super::handlers_operator_log::{v1_log_ovr_del, v1_log_ovr_get, v1_log_ovr_set};
use super::handlers_peer::{v1_dev_peers, v1_peer_hello};
use super::handlers_roaming::{
    v1_export_handoff_register, v1_export_readiness, v1_roaming_intent_create,
    v1_roaming_intent_finalize, v1_roaming_intent_status,
};
use super::handlers_shutdown::v1_shutdown;
use super::handlers_status::{v1_flow_recent, v1_head, v1_status};
use super::handlers_tx::v1_tx;
use super::types::V1_TX_BODY_LIMIT;

/// After `with_state(app)` this is `Router<()>` (see axum `with_state` docs).
pub fn router(app: App, cors: CorsLayer) -> Router {
    Router::new()
        .route("/v1/status", get(v1_status))
        .route(
            "/v1/bridge-federation/reset",
            post(v1_bridge_federation_reset),
        )
        .route(
            "/v1/operator/log/override",
            get(v1_log_ovr_get)
                .post(v1_log_ovr_set)
                .delete(v1_log_ovr_del),
        )
        .route("/v1/shutdown", post(v1_shutdown))
        .route("/v1/head", get(v1_head))
        .route("/v1/accounts", get(v1_accounts))
        .route("/v1/account/:id", get(v1_account))
        .route("/v1/tx", post(v1_tx))
        .route("/v1/export-readiness", post(v1_export_readiness))
        .route("/v1/roaming-intents", post(v1_roaming_intent_create))
        .route("/v1/roaming-intents/:id", get(v1_roaming_intent_status))
        .route(
            "/v1/roaming-intents/:id/finalize",
            post(v1_roaming_intent_finalize),
        )
        .route("/v1/export-provenance", post(v1_export_handoff_register))
        .route("/v1/cross-shard/facts", get(v1_cross_shard_facts))
        .route("/v1/cross-shard/backfill", post(v1_cross_shard_backfill))
        .route("/v1/flow/recent", get(v1_flow_recent))
        .route("/v1/peer/hello", post(v1_peer_hello))
        .route("/v1/dev/peers", get(v1_dev_peers))
        .route("/v1/federation/shards", get(v1_federation_shards))
        .layer(DefaultBodyLimit::max(V1_TX_BODY_LIMIT))
        .layer(cors)
        .with_state(app)
}
