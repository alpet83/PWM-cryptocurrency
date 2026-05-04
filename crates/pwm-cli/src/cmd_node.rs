//! Operator RPC: graceful pwmd shutdown.

use crate::rpc_helpers::map_reqwest_err;
use crate::{cli_config::http_client_for_rpc, exit_user_error};

pub(crate) fn run_node_shutdown(rpc_base: &str) {
    let base = rpc_base.trim_end_matches('/');
    let url = format!("{base}/v1/shutdown");
    let client = http_client_for_rpc();
    let res = client.post(url).send();
    match res {
        Ok(resp) if resp.status() == reqwest::StatusCode::NO_CONTENT => {
            println!("pwmd shutdown acknowledged (snapshot persisted if configured)");
        }
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            exit_user_error(&format!(
                "shutdown failed: HTTP {status} {}",
                crate::rpc_helpers::truncate_rpc_body_hint(&body, 400)
            ));
        }
        Err(e) => exit_user_error(&map_reqwest_err(&e, "POST /v1/shutdown")),
    }
}
