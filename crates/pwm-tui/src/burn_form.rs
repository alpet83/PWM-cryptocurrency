//! F5 burn-mark form state (marks input stays `u32`).

use std::time::Instant;

use pwm_core::{parse_account_id, parse_acct_id_ui, validate_recipient_domain_policy, AccountId};

use crate::config::SEND_FLOW_STEP_TIMEOUT;
use crate::modals::TextInput;
use crate::send_form::SendStepFlow;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BurnField {
    MarkAmount,
    Beneficiary,
    Purpose,
    Confirm,
}

pub struct BurnForm {
    pub from: String,
    pub marks_available: u32,
    pub mark_amount: TextInput,
    pub beneficiary: TextInput,
    pub beneficiary_editable: bool,
    pub purpose: TextInput,
    pub confirm: TextInput,
    pub active: BurnField,
    pub status: String,
    pub status_is_error: bool,
    pub flow: Option<SendStepFlow>,
}

impl BurnForm {
    pub fn new(
        from: String,
        marks_available: u32,
        beneficiary: String,
        beneficiary_editable: bool,
    ) -> Self {
        Self {
            from,
            marks_available,
            mark_amount: TextInput::new(),
            beneficiary: TextInput::from_end(beneficiary),
            beneficiary_editable,
            purpose: TextInput::from_end("default".into()),
            confirm: TextInput::new(),
            active: BurnField::MarkAmount,
            status:
                "V5 marks: stake PWM with S, wait for blocks, then burn materialized marks with F5."
                    .into(),
            status_is_error: false,
            flow: None,
        }
    }

    pub fn flow_is_active(&self) -> bool {
        self.flow.as_ref().is_some_and(SendStepFlow::is_active)
    }

    pub fn failed_flow_lock(&self) -> bool {
        self.flow.as_ref().is_some_and(|flow| flow.failed)
    }

    pub fn try_advance_flow(&mut self, now: Instant) -> bool {
        if let Some(flow) = self.flow.as_mut() {
            if flow.next_step(now) {
                self.status = flow.status_text(now, SEND_FLOW_STEP_TIMEOUT);
                self.status_is_error = flow.failed;
                return true;
            }
        }
        false
    }

    pub fn auto_advance_flow(&mut self, now: Instant) -> bool {
        if let Some(flow) = self.flow.as_mut() {
            if flow.auto_advance_if_due(now, SEND_FLOW_STEP_TIMEOUT) {
                self.status = flow.status_text(now, SEND_FLOW_STEP_TIMEOUT);
                self.status_is_error = flow.failed;
                return true;
            }
        }
        false
    }

    pub fn apply_submit_result(&mut self, result: &Result<String, String>) {
        let now = Instant::now();
        let (msg, is_error) = match result {
            Ok(msg) => (msg.as_str(), false),
            Err(err) => (err.as_str(), true),
        };
        let flow = SendStepFlow::from_submit_message(msg, is_error, now);
        self.status = flow.status_text(now, SEND_FLOW_STEP_TIMEOUT);
        self.status_is_error = flow.failed;
        self.flow = Some(flow);
    }

    pub fn next_field(&mut self) {
        self.active = if self.beneficiary_editable {
            match self.active {
                BurnField::MarkAmount => BurnField::Beneficiary,
                BurnField::Beneficiary => BurnField::Purpose,
                BurnField::Purpose => BurnField::Confirm,
                BurnField::Confirm => BurnField::MarkAmount,
            }
        } else {
            match self.active {
                BurnField::MarkAmount => BurnField::Purpose,
                BurnField::Beneficiary => BurnField::Purpose,
                BurnField::Purpose => BurnField::Confirm,
                BurnField::Confirm => BurnField::MarkAmount,
            }
        };
    }

    pub fn prev_field(&mut self) {
        self.active = if self.beneficiary_editable {
            match self.active {
                BurnField::MarkAmount => BurnField::Confirm,
                BurnField::Beneficiary => BurnField::MarkAmount,
                BurnField::Purpose => BurnField::Beneficiary,
                BurnField::Confirm => BurnField::Purpose,
            }
        } else {
            match self.active {
                BurnField::MarkAmount => BurnField::Confirm,
                BurnField::Beneficiary => BurnField::MarkAmount,
                BurnField::Purpose => BurnField::MarkAmount,
                BurnField::Confirm => BurnField::Purpose,
            }
        };
    }

    fn active_input_mut(&mut self) -> Option<&mut TextInput> {
        match self.active {
            BurnField::MarkAmount => Some(&mut self.mark_amount),
            BurnField::Beneficiary if self.beneficiary_editable => Some(&mut self.beneficiary),
            BurnField::Beneficiary => None,
            BurnField::Purpose => Some(&mut self.purpose),
            BurnField::Confirm => Some(&mut self.confirm),
        }
    }

    pub fn clamp_active_cursor(&mut self) {
        if let Some(inp) = self.active_input_mut() {
            inp.clamp_cursor();
        }
    }

    pub fn move_left(&mut self) {
        if let Some(inp) = self.active_input_mut() {
            inp.move_left();
        }
    }

    pub fn move_right(&mut self) {
        if let Some(inp) = self.active_input_mut() {
            inp.move_right();
        }
    }

    pub fn move_home(&mut self) {
        if let Some(inp) = self.active_input_mut() {
            inp.move_home();
        }
    }

    pub fn move_end(&mut self) {
        if let Some(inp) = self.active_input_mut() {
            inp.move_end();
        }
    }

    pub fn insert_char(&mut self, c: char) {
        if let Some(inp) = self.active_input_mut() {
            inp.insert_char(c);
        }
    }

    pub fn backspace(&mut self) {
        if let Some(inp) = self.active_input_mut() {
            inp.backspace();
        }
    }

    pub fn delete(&mut self) {
        if let Some(inp) = self.active_input_mut() {
            inp.delete();
        }
    }
}

pub fn burn_replay_guard_status(
    form: &BurnForm,
    inflight_burn_req_id: Option<u64>,
) -> Option<&'static str> {
    if inflight_burn_req_id.is_some() {
        Some("submit already in progress")
    } else if form.failed_flow_lock() {
        Some("burn failed: press Esc to close form")
    } else if form.flow_is_active() {
        Some("step flow is active: press Enter to continue")
    } else {
        None
    }
}

pub fn validate_burn_form(
    form: &BurnForm,
) -> Result<(AccountId, u32, Option<AccountId>, String), String> {
    let from = parse_account_id(&form.from).map_err(|e| format!("from: {e}"))?;
    let raw_amt = form.mark_amount.as_str().trim();
    let mark_amount: u32 = raw_amt.parse().map_err(|_| {
        format!("mark_amount: expected unsigned integer mark units, got {raw_amt:?}")
    })?;
    if mark_amount == 0 {
        return Err("mark_amount must be > 0".into());
    }
    let ben_trim = form.beneficiary.as_str().trim();
    let beneficiary = if ben_trim.is_empty() {
        None
    } else {
        let b = parse_acct_id_ui(ben_trim).map_err(|e| format!("beneficiary: {e}"))?;
        validate_recipient_domain_policy(&b, Some("--beneficiary"))
            .map_err(|e| format!("beneficiary policy: {e}"))?;
        Some(b)
    };
    let purpose = form.purpose.as_str().trim().to_string();
    if purpose.is_empty() {
        return Err("purpose must not be empty after trim (use 'default' or custom text)".into());
    }
    if form.confirm.as_str().trim().to_lowercase() != "yes" {
        return Err("confirm must be 'yes'".into());
    }
    Ok((from, mark_amount, beneficiary, purpose))
}

#[cfg(test)]
mod tests {
    use super::BurnForm;

    #[test]
    fn default_purpose_is_default() {
        let form = BurnForm::new("pwm1-test-from".into(), 7, "aabbcc".into(), true);
        assert_eq!(form.marks_available, 7);
        assert_eq!(form.beneficiary.as_str(), "aabbcc");
        assert_eq!(form.purpose.as_str(), "default");
        assert!(!form.status.contains("Claim"));
    }
}
