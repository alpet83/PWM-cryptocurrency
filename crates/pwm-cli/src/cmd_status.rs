//! Human-readable `/v1/status` summary.

use crate::rpc_helpers::map_reqwest_err;
use crate::{exit_user_error, http_client_for_rpc};
use serde_json::Value;

fn v_u64(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn v_bool(v: &Value, key: &str) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn v_str<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("unknown")
}

pub(crate) fn run_status(rpc_base: &str) {
    let c = http_client_for_rpc();
    let url = format!("{}/v1/status", rpc_base.trim_end_matches('/'));
    let r = c
        .get(&url)
        .send()
        .map_err(|e| map_reqwest_err(&e, "GET /v1/status"))
        .unwrap_or_else(|e| exit_user_error(&e));
    let status = r.status();
    let body = r
        .text()
        .unwrap_or_else(|e| exit_user_error(&format!("GET /v1/status body: {e}")));
    if !status.is_success() {
        exit_user_error(&format!("GET /v1/status HTTP {status}: {body}"));
    }
    let v: Value = serde_json::from_str(&body)
        .unwrap_or_else(|e| exit_user_error(&format!("GET /v1/status invalid JSON: {e}")));
    println!(
        "node phase={} ready={} shard={} role={} lease_state={}",
        v_str(&v, "phase"),
        v_bool(&v, "ready"),
        v_str(&v, "shard"),
        v_str(&v, "seal_role"),
        v_str(&v, "lease_state")
    );
    let prep = v.get("cluster_prep").unwrap_or(&Value::Null);
    println!(
        "cluster_prep phase={} ready_for_seal={} sync_n={} live_n={} peer_tip_max={} local_tip={} blocks_behind_max={} waiting_since_ms={} waiting_sec={} blocked_reason={}",
        v_str(prep, "phase"),
        v_bool(prep, "ready_for_seal"),
        v_u64(prep, "sync_n"),
        v_u64(prep, "live_n"),
        v_u64(prep, "peer_tip_max"),
        v_u64(prep, "local_tip"),
        v_u64(prep, "blocks_behind_max"),
        prep.get("waiting_since_ms")
            .and_then(Value::as_u64)
            .map(|v| v.to_string())
            .unwrap_or_else(|| "none".to_string()),
        prep.get("waiting_sec")
            .and_then(Value::as_u64)
            .map(|v| v.to_string())
            .unwrap_or_else(|| "none".to_string()),
        prep.get("blocked_reason")
            .and_then(Value::as_str)
            .unwrap_or("none")
    );
}
