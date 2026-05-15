//! Optional UX helper: summarize pwmd `/v1/tx` JSON reject bodies (RFC 0014 baseline shape).

use serde_json::Value;

/// Parse pwmd `/v1/tx` JSON reject body and return a single-line hint for CLI/TUI.
pub fn summarize_tx_reject_json(body: &str) -> Option<String> {
    let v: Value = serde_json::from_str(body.trim()).ok()?;
    if v.get("ok").and_then(|x| x.as_bool()) == Some(true) {
        return None;
    }
    let phase = v.get("phase").and_then(|x| x.as_str()).unwrap_or("?");
    let tx_kind = v.get("tx_kind").and_then(|x| x.as_str()).unwrap_or("?");
    let class = v
        .get("response_class")
        .and_then(|x| x.as_str())
        .unwrap_or("?");
    let err = v.get("error")?;
    let code = err.get("code").and_then(|x| x.as_str()).unwrap_or("?");
    let msg = err.get("message").and_then(|x| x.as_str()).unwrap_or("");
    let trace = err.get("trace_id").and_then(|x| x.as_str()).unwrap_or("");
    let mut s = format!("reject: code={code} class={class} phase={phase} tx_kind={tx_kind}");
    if let Some(claim_mode) = v.get("claim_mode").and_then(|x| x.as_str()) {
        s.push_str(&format!(" claim_mode={claim_mode}"));
    }
    if !trace.is_empty() {
        s.push_str(&format!(" trace_id={trace}"));
    }
    if !msg.is_empty() {
        s.push_str(&format!(" msg={msg}"));
    }
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarizes_rfc_shape() {
        let j = r#"{"ok":false,"phase":"preflight","tx_kind":"burn","response_class":"VALIDATION_ERROR","error":{"code":"E_BURN_SCHEMA_INVALID","message":"bad","trace_id":"abc"}}"#;
        let s = summarize_tx_reject_json(j).expect("hint");
        assert!(s.contains("E_BURN_SCHEMA_INVALID"));
        assert!(s.contains("VALIDATION_ERROR"));
        assert!(s.contains("preflight"));
        assert!(s.contains("trace_id=abc"));
    }

    #[test]
    fn ignores_ok_true() {
        assert!(summarize_tx_reject_json(r#"{"ok":true}"#).is_none());
    }

    #[test]
    fn summarizes_claim_mode_when_present() {
        let j = r#"{"ok":false,"phase":"validate","tx_kind":"claim","claim_mode":"auto","response_class":"VALIDATION_ERROR","error":{"code":"E_CLAIM_SHAPE_INVALID","message":"bad mode","trace_id":"trk1"}}"#;
        let s = summarize_tx_reject_json(j).expect("hint");
        assert!(s.contains("tx_kind=claim"));
        assert!(s.contains("claim_mode=auto"));
    }
}
