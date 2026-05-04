//! `/v1/accounts`, `/v1/account/:id`.

use super::common::{
    acct_out_for_runtime, ensure_ready, foreign_home_lookup_state, parse_id, HomeLookupState,
};
use super::types::{AcctListOut, AcctOut};
use crate::App;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use pwm_core::hd::domain_of_account_id;

pub(super) async fn v1_accounts(
    State(a): State<App>,
) -> Result<Json<AcctListOut>, (StatusCode, String)> {
    ensure_ready(&a).await?;
    let now_ms = crate::current_time_ms().unwrap_or(0);
    let g = a.inner.read().await;
    let mut accounts = Vec::new();
    for (id, ac) in g.chain.st.accounts.iter() {
        let home_hi = domain_of_account_id(id).to_be_bytes()[0];
        let peer_view = g.peer_account_views.get(id);
        let home_lookup_state = if home_hi == a.identity.cluster_domain_hi {
            HomeLookupState::Ok
        } else {
            foreign_home_lookup_state(
                &a,
                home_hi,
                peer_view.is_some(),
                peer_view.map(|x| x.source_node_id.as_str()),
                now_ms,
            )
            .await
        };
        accounts.push(acct_out_for_runtime(
            id,
            ac,
            a.identity.cluster_domain_hi,
            peer_view,
            home_lookup_state,
        ));
    }
    Ok(Json(AcctListOut { accounts }))
}

pub(super) async fn v1_account(
    State(a): State<App>,
    Path(id): Path<String>,
) -> Result<Json<AcctOut>, (StatusCode, String)> {
    ensure_ready(&a).await?;
    let key =
        parse_id(&id).map_err(|_| (StatusCode::BAD_REQUEST, "invalid account id".to_string()))?;
    let home_hi = domain_of_account_id(&key).to_be_bytes()[0];
    let now_ms = crate::current_time_ms().unwrap_or(0);
    let g = a.inner.read().await;
    let ac = g
        .chain
        .st
        .get(&key)
        .ok_or((StatusCode::NOT_FOUND, "account not found".to_string()))?;
    let peer_view = g.peer_account_views.get(&key);
    let home_lookup_state = if home_hi == a.identity.cluster_domain_hi {
        HomeLookupState::Ok
    } else {
        foreign_home_lookup_state(
            &a,
            home_hi,
            peer_view.is_some(),
            peer_view.map(|x| x.source_node_id.as_str()),
            now_ms,
        )
        .await
    };
    Ok(Json(acct_out_for_runtime(
        &key,
        ac,
        a.identity.cluster_domain_hi,
        peer_view,
        home_lookup_state,
    )))
}
