//! `/v1/accounts`, `/v1/account/:id`.

use super::common::{
    acct_out_for_runtime, ensure_ready, foreign_home_lookup_state, parse_id, HomeLookupState,
};
use super::types::{AcctListOut, AcctOut, PendingConservationOut};
use crate::state::{Inner, PeerAccountView};
use crate::App;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use pwm_core::hd::domain_of_account_id;
use pwm_core::types::Account;
use pwm_core::{AccountId, State as CoreState};

fn pending_conservation_out(st: &CoreState, key: &[u8; 32]) -> Vec<PendingConservationOut> {
    st.pending_conservation
        .iter()
        .filter(|row| row.sender == *key)
        .map(|row| PendingConservationOut {
            recipient: hex::encode(row.recipient),
            amount_pwm: row.amount_pwm.to_string(),
            fee_pwm: row.fee_pwm.to_string(),
            nonce: row.nonce,
            enqueue_height: row.enqueue_height,
            execute_at_height: row.execute_at_height,
        })
        .collect()
}

struct AccountSnapshot {
    id: AccountId,
    account: Account,
    peer_view: Option<PeerAccountView>,
    pending: Vec<PendingConservationOut>,
}

fn account_snapshots(inner: &Inner) -> Vec<AccountSnapshot> {
    inner
        .chain
        .st
        .accounts
        .iter()
        .map(|(id, account)| AccountSnapshot {
            id: *id,
            account: account.clone(),
            peer_view: inner.peer_account_views.get(id).cloned(),
            pending: pending_conservation_out(&inner.chain.st, id),
        })
        .collect()
}

fn account_snapshot(inner: &Inner, id: AccountId) -> Option<AccountSnapshot> {
    let account = inner.chain.st.get(&id)?.clone();
    Some(AccountSnapshot {
        id,
        account,
        peer_view: inner.peer_account_views.get(&id).cloned(),
        pending: pending_conservation_out(&inner.chain.st, &id),
    })
}

pub(super) async fn v1_accounts(
    State(a): State<App>,
) -> Result<Json<AcctListOut>, (StatusCode, String)> {
    ensure_ready(&a).await?;
    let now_ms = crate::current_time_ms().unwrap_or(0);
    let snapshots = {
        let g = a.inner.read().await;
        account_snapshots(&g)
    };
    let mut accounts = Vec::with_capacity(snapshots.len());
    for item in snapshots {
        let home_hi = domain_of_account_id(&item.id).to_be_bytes()[0];
        let peer_view = item.peer_view.as_ref();
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
        let mut out = acct_out_for_runtime(
            &item.id,
            &item.account,
            a.identity.cluster_domain_hi,
            peer_view,
            home_lookup_state,
        );
        out.pending_conservation = item.pending;
        accounts.push(out);
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
    let item = {
        let g = a.inner.read().await;
        account_snapshot(&g, key)
    }
    .ok_or((StatusCode::NOT_FOUND, "account not found".to_string()))?;
    let home_hi = domain_of_account_id(&item.id).to_be_bytes()[0];
    let now_ms = crate::current_time_ms().unwrap_or(0);
    let peer_view = item.peer_view.as_ref();
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
    let mut out = acct_out_for_runtime(
        &item.id,
        &item.account,
        a.identity.cluster_domain_hi,
        peer_view,
        home_lookup_state,
    );
    out.pending_conservation = item.pending;
    Ok(Json(out))
}
