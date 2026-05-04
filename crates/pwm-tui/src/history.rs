//! Operation history rows for the send panel.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::OP_HISTORY_MAX_ITEMS;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpStatus {
    Pending,
    Ok,
    Error,
}

impl OpStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            OpStatus::Pending => "pending",
            OpStatus::Ok => "ok",
            OpStatus::Error => "error",
        }
    }
}

#[derive(Clone, Debug)]
pub struct OperationHistoryEntry {
    pub req_id: u64,
    pub created_unix_secs: u64,
    pub from_human: String,
    pub to_human: String,
    pub amount_units: u128,
    pub fee_units: u128,
    pub status: OpStatus,
    pub note: String,
}

pub fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn format_hms_utc(ts: u64) -> String {
    let sec = ts % 60;
    let min = (ts / 60) % 60;
    let hour = (ts / 3600) % 24;
    format!("{hour:02}:{min:02}:{sec:02}Z")
}

pub fn push_op_history(hist: &mut Vec<OperationHistoryEntry>, entry: OperationHistoryEntry) {
    hist.insert(0, entry);
    if hist.len() > OP_HISTORY_MAX_ITEMS {
        hist.truncate(OP_HISTORY_MAX_ITEMS);
    }
}

pub fn set_op_history_status(
    hist: &mut [OperationHistoryEntry],
    req_id: u64,
    status: OpStatus,
    note: String,
) -> bool {
    if let Some(item) = hist.iter_mut().find(|x| x.req_id == req_id) {
        item.status = status;
        item.note = note;
        true
    } else {
        false
    }
}

pub fn handle_submit_done_history(
    inflight_send_req_id: &mut Option<u64>,
    op_history: &mut [OperationHistoryEntry],
    req_id: u64,
    result: &Result<String, String>,
) -> bool {
    if *inflight_send_req_id != Some(req_id) {
        return false;
    }
    *inflight_send_req_id = None;
    match result {
        Ok(msg) => {
            let _ = set_op_history_status(op_history, req_id, OpStatus::Ok, msg.clone());
        }
        Err(err) => {
            let _ = set_op_history_status(op_history, req_id, OpStatus::Error, err.clone());
        }
    }
    true
}
