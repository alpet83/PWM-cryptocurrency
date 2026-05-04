//! RPC fetch helpers, health enums, and footer status rendering.

use crate::config::rpc_context_label;
use ratatui::prelude::{Color, Line, Span, Style};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonFetchFailure {
    Timeout,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpcHealth {
    Online,
    Timeout,
    Offline,
}

impl RpcHealth {
    fn severity(self) -> u8 {
        match self {
            RpcHealth::Online => 0,
            RpcHealth::Timeout => 1,
            RpcHealth::Offline => 2,
        }
    }
}

pub fn merge_rpc_health(lhs: RpcHealth, rhs: RpcHealth) -> RpcHealth {
    if rhs.severity() > lhs.severity() {
        rhs
    } else {
        lhs
    }
}

pub fn rpc_health_from_failure(f: JsonFetchFailure) -> RpcHealth {
    match f {
        JsonFetchFailure::Timeout => RpcHealth::Timeout,
        JsonFetchFailure::Other => RpcHealth::Offline,
    }
}

/// Max `tip=` payload length before middle-ellipsis (hex hashes are long).
const FOOTER_TIP_FULL_MAX: usize = 24;
const FOOTER_TIP_PREFIX_KEEP: usize = 8;
const FOOTER_TIP_SUFFIX_KEEP: usize = 8;

/// ASCII-only middle ellipsis for footer one-liners (chain tip hex, etc.).
pub fn ellipsis_middle_ascii(value: &str, keep_prefix: usize, keep_suffix: usize) -> String {
    let max_plain = keep_prefix + keep_suffix;
    if value.len() <= max_plain {
        return value.to_string();
    }
    if keep_prefix + 3 + keep_suffix >= value.len() {
        return value.to_string();
    }
    let mut out = String::new();
    out.push_str(&value[..keep_prefix]);
    out.push_str("...");
    out.push_str(&value[value.len() - keep_suffix..]);
    out
}

/// Shortens `height=… tip=<long>` style head strings for the status footer.
pub fn format_footer_head_line(head: &str) -> String {
    const SEP: &str = " tip=";
    let Some(pos) = head.find(SEP) else {
        return head.to_string();
    };
    let tip_start = pos + SEP.len();
    let tip_val = &head[tip_start..];
    if tip_val.len() <= FOOTER_TIP_FULL_MAX {
        return head.to_string();
    }
    let short = ellipsis_middle_ascii(tip_val, FOOTER_TIP_PREFIX_KEEP, FOOTER_TIP_SUFFIX_KEEP);
    format!("{}{SEP}{short}", &head[..pos])
}

pub fn rpc_bad_label(rpc_health: RpcHealth) -> Option<&'static str> {
    match rpc_health {
        RpcHealth::Online => None,
        RpcHealth::Timeout => Some("RPC timeout"),
        RpcHealth::Offline => Some("RPC offline"),
    }
}

/// Builds the bottom status `Line`: RPC health and poll errors first so they stay visible on narrow terminals.
pub fn status_footer_line(
    head: &str,
    err: &str,
    identity_note: &str,
    f3_action: &str,
    rpc_health: RpcHealth,
    dbg: bool,
    rpc_url: &str,
    status_shard_label: Option<&str>,
) -> Line<'static> {
    let head_shown = format_footer_head_line(head);
    let rpc_context = rpc_context_label(rpc_url, status_shard_label);
    let mut tail = format!(
        "{} | {} | Tab switch panel | Arrows move active panel | H history | F3 {} | F4 encrypt | F5 burn->CLI | F6 send | F10 quit",
        head_shown, rpc_context, f3_action
    );
    if dbg {
        tail.push_str(" | PWM_TUI_DEBUG=1");
    }
    if !identity_note.is_empty() {
        tail.push_str(" | ");
        tail.push_str(identity_note);
    }

    let bad = rpc_bad_label(rpc_health);
    let mut spans: Vec<Span<'static>> = Vec::new();
    if let Some(label) = bad {
        spans.push(Span::styled(label, Style::default().fg(Color::Red)));
    }
    if !err.is_empty() {
        if !spans.is_empty() {
            spans.push(Span::raw(" | "));
        }
        // Own the poll error text so the returned `Line` does not borrow caller locals.
        spans.push(Span::raw(err.to_string()));
    }
    if !spans.is_empty() {
        spans.push(Span::raw(" | "));
    }
    spans.push(Span::raw(tail));
    Line::from(spans)
}

pub fn debug_json() -> bool {
    matches!(
        std::env::var("PWM_TUI_DEBUG").as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    )
}

pub fn fetch_json(
    client: &reqwest::blocking::Client,
    url: &str,
) -> Result<Value, JsonFetchFailure> {
    let r = client.get(url).send().map_err(|e| {
        if e.is_timeout() {
            JsonFetchFailure::Timeout
        } else {
            JsonFetchFailure::Other
        }
    })?;
    if !r.status().is_success() {
        return Err(JsonFetchFailure::Other);
    }
    r.json().map_err(|_| JsonFetchFailure::Other)
}
