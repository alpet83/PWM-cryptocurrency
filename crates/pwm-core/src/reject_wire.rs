//! Optional UX helper: summarize pwmd `/v1/tx` JSON reject bodies (RFC 0014 baseline shape).

use crate::tx::TxError;
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

/// Stable mapping from core tx errors into wire reject code/class.
pub fn tx_err_wire(e: &TxError, tx_kind: &str) -> (&'static str, &'static str) {
    use TxError::*;
    match e {
        InvalidPurposeLength | InvalidPurposeChars if tx_kind == "burn" => {
            ("E_BURN_SCHEMA_INVALID", "VALIDATION_ERROR")
        }
        InsufficientMarks if tx_kind == "burn" => ("E_BURN_OVER_BALANCE", "STATE_CONFLICT"),
        DomainMismatch if tx_kind == "burn" => ("E_BURN_POLICY_REJECT", "POLICY_REJECT"),
        ImportFeeTooLow => ("E_IMPORT_FEE_TOO_LOW", "POLICY_REJECT"),
        ExportLockRefunded => ("E_EXPORT_LOCK_REFUNDED", "STATE_CONFLICT"),
        PolicySchemaInvalid => ("E_POLICY_SCHEMA_INVALID", "VALIDATION_ERROR"),
        PolicyNotInstalled => ("E_POLICY_NOT_INSTALLED", "POLICY_REJECT"),
        PolicyNotActive => ("E_POLICY_NOT_ACTIVE", "POLICY_REJECT"),
        PolicyDenied => ("E_POLICY_DENIED", "POLICY_REJECT"),
        PolicySenderFiltered => ("E_POLICY_SENDER_FILTERED", "POLICY_REJECT"),
        PolicyRoutingDenied => ("E_POLICY_ROUTING_DENIED", "POLICY_REJECT"),
        PolicyMissingCosign => ("E_POLICY_MISSING_COSIGN", "POLICY_REJECT"),
        PolicyRescueRequired => ("E_POLICY_RESCUE_REQUIRED", "POLICY_REJECT"),
        PolicyEmergencyCosignRequired => ("E_POLICY_EMERGENCY_COSIGN_REQUIRED", "POLICY_REJECT"),
        PolicyAccountFinalized => ("E_POLICY_ACCOUNT_FINALIZED", "POLICY_REJECT"),
        PolicyIrreversible => ("E_POLICY_IRREVERSIBLE", "POLICY_REJECT"),
        PolicyFlagNonDisableable => ("E_POLICY_FLAG_NON_DISABLEABLE", "POLICY_REJECT"),
        ConservationDelayRequired => ("E_CONSERVATION_DELAY_REQUIRED", "POLICY_REJECT"),
        ConservationPendingExists => ("E_CONSERVATION_PENDING_EXISTS", "POLICY_REJECT"),
        PolicyActivationFeeMustBeZero => ("E_POLICY_ACTIVATION_FEE_MUST_BE_ZERO", "POLICY_REJECT"),
        PolicyActivationTargetMismatch => ("E_POLICY_ACTIVATION_TARGET_MISMATCH", "POLICY_REJECT"),
        PolicyActivationTargetRequired => ("E_POLICY_ACTIVATION_TARGET_REQUIRED", "POLICY_REJECT"),
        PolicyActivationTargetNotAllowed => {
            ("E_POLICY_ACTIVATION_TARGET_NOT_ALLOWED", "VALIDATION_ERROR")
        }
        DuplicateImport | EvidenceDuplicate => ("E_EVIDENCE_DUPLICATE", "STATE_CONFLICT"),
        _ => ("E_SCHEMA_INVALID", "VALIDATION_ERROR"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tx::TxError;

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

    #[test]
    fn maps_v6_pol_evd_codes() {
        assert_eq!(
            tx_err_wire(&TxError::PolicyFlagNonDisableable, "policy"),
            ("E_POLICY_FLAG_NON_DISABLEABLE", "POLICY_REJECT")
        );
        assert_eq!(
            tx_err_wire(&TxError::ConservationDelayRequired, "policy"),
            ("E_CONSERVATION_DELAY_REQUIRED", "POLICY_REJECT")
        );
        assert_eq!(
            tx_err_wire(&TxError::ConservationPendingExists, "policy"),
            ("E_CONSERVATION_PENDING_EXISTS", "POLICY_REJECT")
        );
        assert_eq!(
            tx_err_wire(&TxError::PolicyActivationFeeMustBeZero, "policy"),
            ("E_POLICY_ACTIVATION_FEE_MUST_BE_ZERO", "POLICY_REJECT")
        );
        assert_eq!(
            tx_err_wire(&TxError::PolicyActivationTargetMismatch, "policy"),
            ("E_POLICY_ACTIVATION_TARGET_MISMATCH", "POLICY_REJECT")
        );
        assert_eq!(
            tx_err_wire(&TxError::PolicyActivationTargetRequired, "policy"),
            ("E_POLICY_ACTIVATION_TARGET_REQUIRED", "POLICY_REJECT")
        );
        assert_eq!(
            tx_err_wire(&TxError::PolicyActivationTargetNotAllowed, "policy"),
            ("E_POLICY_ACTIVATION_TARGET_NOT_ALLOWED", "VALIDATION_ERROR")
        );
        assert_eq!(
            tx_err_wire(&TxError::EvidenceDuplicate, "import"),
            ("E_EVIDENCE_DUPLICATE", "STATE_CONFLICT")
        );
        assert_eq!(
            tx_err_wire(&TxError::DuplicateImport, "import"),
            ("E_EVIDENCE_DUPLICATE", "STATE_CONFLICT")
        );
    }
}
