//! Background RPC worker plus UI snapshot assembly for account panels.

use crate::config::{base_url, http_client, parse_status_shard_label, DEBUG_FETCH_INTERVAL};
use crate::models::{
    parse_hex_account_id, parse_u128, AcctRow, UNKNOWN_BALANCE_SENTINEL,
    UNKNOWN_INIT_NONCE_SENTINEL,
};
use crate::roaming::submit_roaming_intent;
use crate::status::{
    fetch_json, merge_rpc_health, rpc_health_from_failure, JsonFetchFailure, RpcHealth,
};
use crate::tx_submit::submit_transfer;
use crate::wallet::IdentitySource;
use pwm_core::{account_id_to_human, AccountId};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Instant;

pub struct Ui {
    pub head: String,
    pub rows: Vec<AcctRow>,
    pub detail_line: String,
    pub debug_detail: String,
    pub err: String,
    pub rpc_health: RpcHealth,
    pub rpc_shard_label: Option<String>,
    pub identity_note: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Owner,
    Receivers,
}

impl Default for Ui {
    fn default() -> Self {
        Self {
            head: "…".into(),
            rows: vec![],
            detail_line: String::new(),
            debug_detail: String::new(),
            err: String::new(),
            rpc_health: RpcHealth::Online,
            rpc_shard_label: None,
            identity_note: String::new(),
        }
    }
}

#[derive(Clone)]
pub struct PollSnapshot {
    pub head: String,
    pub rows: Vec<AcctRow>,
    pub err: String,
    pub rpc_health: RpcHealth,
    pub rpc_shard_label: Option<String>,
}

pub enum RpcTask {
    Poll,
    DebugAccount {
        id_hex: String,
    },
    SubmitTransfer {
        req_id: u64,
        from: AccountId,
        to: AccountId,
        amount: u128,
        fee: u128,
        identity: IdentitySource,
    },
    SubmitRoamingIntent {
        req_id: u64,
        from: AccountId,
        to: AccountId,
        amount: u128,
        fee: u128,
        identity: IdentitySource,
    },
}

pub enum RpcEvent {
    PollDone(PollSnapshot),
    DebugAccountDone {
        id_hex: String,
        detail: String,
        rpc_health: RpcHealth,
    },
    SubmitDone {
        req_id: u64,
        to_id: AccountId,
        result: Result<String, String>,
    },
}

pub struct DebugCache {
    pub selected_id_hex: Option<String>,
    pub inflight_id_hex: Option<String>,
    pub cached_detail: String,
    pub last_fetch_at: Instant,
}

impl DebugCache {
    pub fn new() -> Self {
        Self {
            selected_id_hex: None,
            inflight_id_hex: None,
            cached_detail: String::new(),
            last_fetch_at: Instant::now() - DEBUG_FETCH_INTERVAL,
        }
    }
}

pub fn acct_row_for_id(rows: &[AcctRow], id: &AccountId, label: Option<String>) -> AcctRow {
    let mut base = rows
        .iter()
        .find(|r| r.id == *id)
        .cloned()
        .unwrap_or_else(|| AcctRow {
            id: *id,
            id_hex: hex::encode(id),
            balance_pwm: 0,
            initialized: false,
            nonce: 0,
            label: None,
        });
    if label.is_some() {
        base.label = label;
    }
    base
}

pub fn owner_and_receivers(
    rows: &[AcctRow],
    identity: &IdentitySource,
) -> (Vec<AcctRow>, usize, Vec<AcctRow>) {
    match identity {
        IdentitySource::Wallet(w) => {
            let owners: Vec<AcctRow> = if w.owned_accounts.is_empty() {
                vec![acct_row_for_id(rows, &w.account_id, None)]
            } else {
                w.owned_accounts
                    .iter()
                    .map(|a| acct_row_for_id(rows, &a.id, None))
                    .collect()
            };
            let active_owner_idx = w
                .owned_accounts
                .iter()
                .position(|a| a.is_active)
                .unwrap_or(0);
            let receivers: Vec<AcctRow> = if !w.address_book.is_empty() {
                w.address_book
                    .iter()
                    .map(|b| acct_row_for_id(rows, &b.id, b.label.clone()))
                    .collect()
            } else if !owners.is_empty() {
                rows.iter()
                    .filter(|r| !owners.iter().any(|o| o.id == r.id))
                    .cloned()
                    .collect()
            } else {
                rows.to_vec()
            };
            (owners, active_owner_idx, receivers)
        }
        IdentitySource::SeedFallback => {
            let owners = rows.first().cloned().into_iter().collect();
            let receivers = rows.iter().skip(1).cloned().collect();
            (owners, 0, receivers)
        }
    }
}

pub fn poll_snapshot(client: &reqwest::blocking::Client) -> PollSnapshot {
    let b = base_url();
    let mut head = "…".to_string();
    let mut rows = Vec::new();
    let mut rpc_health = RpcHealth::Online;
    let mut rpc_shard_label = None;
    let mut parts: Vec<&'static str> = Vec::new();
    match fetch_json(client, &format!("{}/v1/head", b)) {
        Ok(v) => {
            head = format!(
                "height={} tip={}",
                v["height"].as_u64().unwrap_or(0),
                v["tip"].as_str().unwrap_or("?")
            );
        }
        Err(e) => {
            parts.push(match e {
                JsonFetchFailure::Timeout => "head: timeout",
                JsonFetchFailure::Other => "head: offline",
            });
            rpc_health = merge_rpc_health(rpc_health, rpc_health_from_failure(e));
        }
    }
    match fetch_json(client, &format!("{}/v1/accounts", b)) {
        Ok(v) => {
            rows = v["accounts"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| {
                            let id_hex = x["id"].as_str()?.to_string();
                            let id = parse_hex_account_id(&id_hex)?;
                            Some(AcctRow {
                                id,
                                id_hex,
                                balance_pwm: {
                                    let local_view_only =
                                        x["local_view_only"].as_bool().unwrap_or(false);
                                    let lookup = x["home_lookup_status"].as_str().unwrap_or("");
                                    if local_view_only && lookup != "ok" {
                                        UNKNOWN_BALANCE_SENTINEL
                                    } else if local_view_only {
                                        parse_u128(&x["authoritative_home_balance"])
                                    } else {
                                        parse_u128(&x["balance_pwm"])
                                    }
                                },
                                initialized: {
                                    let local_view_only =
                                        x["local_view_only"].as_bool().unwrap_or(false);
                                    let lookup = x["home_lookup_status"].as_str().unwrap_or("");
                                    if local_view_only && lookup == "ok" {
                                        x["authoritative_home_initialized"]
                                            .as_bool()
                                            .or_else(|| x["initialized"].as_bool())
                                            .unwrap_or(false)
                                    } else if local_view_only {
                                        true
                                    } else {
                                        x["initialized"].as_bool().unwrap_or(false)
                                    }
                                },
                                nonce: {
                                    let local_view_only =
                                        x["local_view_only"].as_bool().unwrap_or(false);
                                    let lookup = x["home_lookup_status"].as_str().unwrap_or("");
                                    if local_view_only && lookup != "ok" {
                                        UNKNOWN_INIT_NONCE_SENTINEL
                                    } else {
                                        x["nonce"].as_u64().unwrap_or(0)
                                    }
                                },
                                label: None,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
        }
        Err(e) => {
            parts.push(match e {
                JsonFetchFailure::Timeout => "accounts: timeout",
                JsonFetchFailure::Other => "accounts: offline",
            });
            rpc_health = merge_rpc_health(rpc_health, rpc_health_from_failure(e));
        }
    }
    match fetch_json(client, &format!("{}/v1/status", b)) {
        Ok(v) => {
            rpc_shard_label = parse_status_shard_label(&v);
        }
        Err(e) => {
            rpc_health = merge_rpc_health(rpc_health, rpc_health_from_failure(e));
        }
    }
    PollSnapshot {
        head,
        rows,
        err: parts.join("; "),
        rpc_health,
        rpc_shard_label,
    }
}

pub fn fetch_debug_account(
    client: &reqwest::blocking::Client,
    id_hex: &str,
) -> (String, RpcHealth) {
    let b = base_url();
    match fetch_json(client, &format!("{}/v1/account/{}", b, id_hex)) {
        Ok(v) => (
            serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".into()),
            RpcHealth::Online,
        ),
        Err(JsonFetchFailure::Timeout) => {
            ("debug: account json timeout".into(), RpcHealth::Timeout)
        }
        Err(JsonFetchFailure::Other) => ("debug: account rpc offline".into(), RpcHealth::Offline),
    }
}

pub fn start_rpc_worker() -> (Sender<RpcTask>, Receiver<RpcEvent>) {
    let (task_tx, task_rx) = mpsc::channel::<RpcTask>();
    let (evt_tx, evt_rx) = mpsc::channel::<RpcEvent>();
    thread::spawn(move || {
        let client = http_client();
        while let Ok(task) = task_rx.recv() {
            match task {
                RpcTask::Poll => {
                    let _ = evt_tx.send(RpcEvent::PollDone(poll_snapshot(&client)));
                }
                RpcTask::DebugAccount { id_hex } => {
                    let (detail, rpc_health) = fetch_debug_account(&client, &id_hex);
                    let _ = evt_tx.send(RpcEvent::DebugAccountDone {
                        id_hex,
                        detail,
                        rpc_health,
                    });
                }
                RpcTask::SubmitTransfer {
                    req_id,
                    from,
                    to,
                    amount,
                    fee,
                    identity,
                } => {
                    let result = submit_transfer(&from, &to, amount, fee, &identity);
                    let _ = evt_tx.send(RpcEvent::SubmitDone {
                        req_id,
                        to_id: to,
                        result,
                    });
                }
                RpcTask::SubmitRoamingIntent {
                    req_id,
                    from,
                    to,
                    amount,
                    fee,
                    identity,
                } => {
                    let result = submit_roaming_intent(&from, &to, amount, fee, &identity);
                    let _ = evt_tx.send(RpcEvent::SubmitDone {
                        req_id,
                        to_id: to,
                        result,
                    });
                }
            }
        }
    });
    (task_tx, evt_rx)
}

pub fn format_acct_cell(r: &AcctRow) -> String {
    if let Some(l) = r
        .label
        .as_ref()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        format!("{l} | {}", account_id_to_human(&r.id))
    } else {
        account_id_to_human(&r.id)
    }
}
