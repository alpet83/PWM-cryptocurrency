//! Operator-only runtime log override RPC with TTL auto-restore.

use crate::state::LogOvrState;
use crate::App;
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tracing::{info, warn};

const OVR_TTL_MIN: u64 = 1;
const OVR_TTL_MAX: u64 = 3600;
const LOG_LEVELS: &[&str] = &["trace", "debug", "info", "warn", "error"];
const LOG_FOCUS: &[&str] = &[
    "transport:peers",
    "sync:live",
    "seal:loop",
    "snapshot",
    "api",
    "all",
];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LogOvrIn {
    level: String,
    focus: String,
    ttl_seconds: u64,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct LogOvrOut {
    active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    focus: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at_ms: Option<u64>,
    baseline: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

pub(super) async fn v1_log_ovr_get(
    State(a): State<App>,
    conn: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
) -> Result<Json<LogOvrOut>, (StatusCode, String)> {
    let remote = conn.map(|v| v.0);
    let _auth_mode = ensure_op_log_auth(&a, remote, &headers)?;
    clear_if_expired(&a).await?;
    let ctl = a.log_ctl.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "runtime log control is unavailable".to_string(),
    ))?;
    let baseline = ctl.baseline_spec();
    let state = a.log_ovr.read().await.clone();
    Ok(Json(match state {
        Some(row) => LogOvrOut {
            active: true,
            level: Some(row.level),
            focus: Some(row.focus),
            expires_at_ms: Some(row.expires_at_ms),
            baseline,
            reason: row.reason,
        },
        None => LogOvrOut {
            active: false,
            level: None,
            focus: None,
            expires_at_ms: None,
            baseline,
            reason: None,
        },
    }))
}

pub(super) async fn v1_log_ovr_set(
    State(a): State<App>,
    conn: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    Json(input): Json<LogOvrIn>,
) -> Result<Json<LogOvrOut>, (StatusCode, String)> {
    let remote = conn.map(|v| v.0);
    let auth_mode = ensure_op_log_auth(&a, remote, &headers)?;
    let ctl = a.log_ctl.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "runtime log control is unavailable".to_string(),
    ))?;
    let level = norm_level(&input.level)?;
    let focus = norm_focus(&input.focus)?;
    if !(OVR_TTL_MIN..=OVR_TTL_MAX).contains(&input.ttl_seconds) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "invalid ttl_seconds: expected {}..={}",
                OVR_TTL_MIN, OVR_TTL_MAX
            ),
        ));
    }
    let now_ms = crate::current_time_ms()?;
    let expires_at_ms = now_ms.saturating_add(input.ttl_seconds.saturating_mul(1000));
    let filter_spec = crate::logging::ovr_filter_spec(&ctl.baseline_spec(), &level, &focus);
    let reason = trim_reason(input.reason);
    ctl.apply_spec(&filter_spec)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let rev = a
        .log_ovr_rev
        .fetch_add(1, Ordering::SeqCst)
        .saturating_add(1);
    {
        let mut slot = a.log_ovr.write().await;
        *slot = Some(LogOvrState {
            level: level.clone(),
            focus: focus.clone(),
            reason: reason.clone(),
            set_at_ms: now_ms,
            expires_at_ms,
            auth_mode: auth_mode.to_string(),
            rev,
        });
    }
    info!(
        target: "pwmd::operator",
        event = "log_override_set",
        auth_mode,
        remote = %remote_label(remote),
        level = %level,
        focus = %focus,
        ttl_seconds = input.ttl_seconds,
        expires_at_ms
    );
    spawn_ovr_ttl(a.clone(), rev, expires_at_ms);
    Ok(Json(LogOvrOut {
        active: true,
        level: Some(level),
        focus: Some(focus),
        expires_at_ms: Some(expires_at_ms),
        baseline: ctl.baseline_spec(),
        reason,
    }))
}

pub(super) async fn v1_log_ovr_del(
    State(a): State<App>,
    conn: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
) -> Result<StatusCode, (StatusCode, String)> {
    let remote = conn.map(|v| v.0);
    let auth_mode = ensure_op_log_auth(&a, remote, &headers)?;
    let _had = clear_ovr(&a, "delete", auth_mode, remote).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn ensure_op_log_auth(
    app: &App,
    remote: Option<SocketAddr>,
    headers: &HeaderMap,
) -> Result<&'static str, (StatusCode, String)> {
    if remote.is_some_and(|addr| addr.ip().is_loopback()) {
        return Ok("loopback");
    }
    if let Some(expected) = app.op_token.as_ref() {
        if bearer_token(headers).is_some_and(|got| got == expected.as_ref()) {
            return Ok("token");
        }
    }
    warn!(
        target: "pwmd::operator",
        event = "log_override_rejected",
        remote = %remote_label(remote),
        has_token_cfg = app.op_token.is_some(),
        reason = "auth_gate"
    );
    Err((
        StatusCode::FORBIDDEN,
        "operator log override requires loopback or valid bearer token".to_string(),
    ))
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let raw = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    raw.strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|v| !v.is_empty())
}

fn norm_level(raw: &str) -> Result<String, (StatusCode, String)> {
    let val = raw.trim().to_ascii_lowercase();
    if LOG_LEVELS.iter().any(|item| *item == val) {
        Ok(val)
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            "invalid level: expected trace|debug|info|warn|error".to_string(),
        ))
    }
}

fn norm_focus(raw: &str) -> Result<String, (StatusCode, String)> {
    let val = raw.trim().to_ascii_lowercase();
    if LOG_FOCUS.iter().any(|item| *item == val) {
        Ok(val)
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            "invalid focus: expected transport:peers|sync:live|seal:loop|snapshot|api|all"
                .to_string(),
        ))
    }
}

fn trim_reason(raw: Option<String>) -> Option<String> {
    raw.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

fn remote_label(remote: Option<SocketAddr>) -> String {
    remote
        .map(|addr| addr.to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

async fn clear_if_expired(app: &App) -> Result<(), (StatusCode, String)> {
    let row = app.log_ovr.read().await.clone();
    let Some(row) = row else {
        return Ok(());
    };
    let now_ms = crate::current_time_ms()?;
    if now_ms < row.expires_at_ms {
        return Ok(());
    }
    let _ = clear_ovr(app, "expire", "ttl", None).await?;
    Ok(())
}

async fn clear_ovr(
    app: &App,
    event: &'static str,
    auth_mode: &str,
    remote: Option<SocketAddr>,
) -> Result<bool, (StatusCode, String)> {
    let ctl = app.log_ctl.as_ref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "runtime log control is unavailable".to_string(),
    ))?;
    let old = app.log_ovr.read().await.clone();
    let Some(old) = old else {
        return Ok(false);
    };
    ctl.apply_baseline()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    {
        let mut slot = app.log_ovr.write().await;
        *slot = None;
    }
    app.log_ovr_rev.fetch_add(1, Ordering::SeqCst);
    let evt = match event {
        "expire" => "log_override_expired",
        _ => "log_override_cleared",
    };
    info!(
        target: "pwmd::operator",
        event = evt,
        auth_mode,
        remote = %remote_label(remote),
        level = %old.level,
        focus = %old.focus,
        expires_at_ms = old.expires_at_ms
    );
    Ok(true)
}

fn spawn_ovr_ttl(app: App, rev: u64, expires_at_ms: u64) {
    tokio::spawn(async move {
        let now_ms = crate::current_time_ms().unwrap_or(0);
        let sleep_ms = expires_at_ms.saturating_sub(now_ms);
        if sleep_ms > 0 {
            tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
        }
        let row = app.log_ovr.read().await.clone();
        let Some(row) = row else {
            return;
        };
        if row.rev != rev {
            return;
        }
        let now_ms = crate::current_time_ms().unwrap_or(0);
        if now_ms < row.expires_at_ms {
            return;
        }
        if let Err(err) = clear_ovr(&app, "expire", "ttl", None).await {
            warn!(
                target: "pwmd::operator",
                event = "log_override_rejected",
                reason = "expire_restore_failed",
                detail = %err.1
            );
        }
    });
}
