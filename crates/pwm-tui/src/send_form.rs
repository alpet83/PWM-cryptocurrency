//! Send modal form state and validation.

use std::time::{Duration, Instant};

use pwm_core::{parse_account_id, parse_acct_id_ui, parse_decimal_pwm_units, AccountId};

use crate::config::SEND_FLOW_STEP_TIMEOUT;
use crate::modals::TextInput;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendField {
    To,
    Amount,
    Fee,
    Confirm,
}

pub struct SendForm {
    pub from: String,
    pub to: TextInput,
    pub to_editable: bool,
    pub amount: TextInput,
    pub fee: TextInput,
    pub confirm: TextInput,
    pub active: SendField,
    pub status: String,
    pub status_is_error: bool,
    pub flow: Option<SendStepFlow>,
    pub pending_book_prompt_to: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SendStepFlow {
    title: String,
    steps: Vec<String>,
    pub shown_steps: usize,
    last_step_at: Instant,
    pub failed: bool,
}

impl SendStepFlow {
    pub fn from_submit_message(message: &str, is_error: bool, now: Instant) -> Self {
        let mut lines: Vec<String> = message
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        let title = if lines.len() > 1 && lines[0].ends_with(':') {
            lines.remove(0)
        } else if is_error {
            "Send failed".into()
        } else {
            "Send completed".into()
        };
        if lines.is_empty() {
            lines.push(message.trim().to_string());
        }
        let shown_steps = 1.min(lines.len());
        let failed = is_error || lines.iter().any(|line| line.contains("FAIL"));
        Self {
            title,
            steps: lines,
            shown_steps,
            last_step_at: now,
            failed,
        }
    }

    pub fn is_active(&self) -> bool {
        self.shown_steps < self.steps.len()
    }

    pub(crate) fn next_step(&mut self, now: Instant) -> bool {
        if !self.is_active() {
            return false;
        }
        self.shown_steps += 1;
        self.last_step_at = now;
        true
    }

    pub fn auto_advance_if_due(&mut self, now: Instant, timeout: Duration) -> bool {
        if self.is_active() && now.duration_since(self.last_step_at) >= timeout {
            return self.next_step(now);
        }
        false
    }

    pub(crate) fn status_text(&self, now: Instant, timeout: Duration) -> String {
        let mut out = Vec::with_capacity(self.shown_steps + 3);
        out.push(self.title.clone());
        out.extend(self.steps.iter().take(self.shown_steps).cloned());
        if self.is_active() {
            let elapsed = now.duration_since(self.last_step_at);
            let left = timeout.saturating_sub(elapsed).as_secs().saturating_add(1);
            out.push(format!("Next step: Enter or auto in {left}s"));
        } else if self.failed {
            out.push("Flow stopped on failure. Press Esc to close form.".into());
        } else {
            out.push("Flow completed. Enter starts a new send, Esc closes form.".into());
        }
        out.join("\n")
    }
}

impl SendForm {
    pub fn new(from: String, to: String, to_editable: bool) -> Self {
        let active = if to_editable {
            SendField::To
        } else {
            SendField::Amount
        };
        Self {
            from,
            to: TextInput::from_end(to),
            to_editable,
            amount: TextInput::new(),
            fee: TextInput::from_end("0".into()),
            confirm: TextInput::new(),
            active,
            status: "Fill fields and press Enter on confirm".into(),
            status_is_error: false,
            flow: None,
            pending_book_prompt_to: None,
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

    pub fn apply_submit_result(
        &mut self,
        result: &Result<String, String>,
        book_prompt_to: Option<String>,
    ) {
        let now = Instant::now();
        let (msg, is_error) = match result {
            Ok(msg) => (msg.as_str(), false),
            Err(err) => (err.as_str(), true),
        };
        let flow = SendStepFlow::from_submit_message(msg, is_error, now);
        self.status = flow.status_text(now, SEND_FLOW_STEP_TIMEOUT);
        self.status_is_error = flow.failed;
        self.flow = Some(flow);
        if self.pending_book_prompt_to.is_none() {
            self.pending_book_prompt_to = book_prompt_to;
        }
    }

    pub fn take_book_prompt(&mut self) -> Option<String> {
        self.pending_book_prompt_to.take()
    }

    pub fn next_field(&mut self) {
        self.active = if self.to_editable {
            match self.active {
                SendField::To => SendField::Amount,
                SendField::Amount => SendField::Fee,
                SendField::Fee => SendField::Confirm,
                SendField::Confirm => SendField::To,
            }
        } else {
            match self.active {
                SendField::To => SendField::Amount,
                SendField::Amount => SendField::Fee,
                SendField::Fee => SendField::Confirm,
                SendField::Confirm => SendField::Amount,
            }
        };
    }

    pub fn prev_field(&mut self) {
        self.active = if self.to_editable {
            match self.active {
                SendField::To => SendField::Confirm,
                SendField::Amount => SendField::To,
                SendField::Fee => SendField::Amount,
                SendField::Confirm => SendField::Fee,
            }
        } else {
            match self.active {
                SendField::To => SendField::Confirm,
                SendField::Amount => SendField::Confirm,
                SendField::Fee => SendField::Amount,
                SendField::Confirm => SendField::Fee,
            }
        };
    }

    pub fn active_input_mut(&mut self) -> Option<&mut TextInput> {
        match self.active {
            SendField::To if self.to_editable => Some(&mut self.to),
            SendField::To => None,
            SendField::Amount => Some(&mut self.amount),
            SendField::Fee => Some(&mut self.fee),
            SendField::Confirm => Some(&mut self.confirm),
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

pub fn send_replay_guard_status(
    form: &SendForm,
    inflight_send_req_id: Option<u64>,
) -> Option<&'static str> {
    if inflight_send_req_id.is_some() {
        Some("submit already in progress")
    } else if form.failed_flow_lock() {
        Some("send failed: press Esc to close form")
    } else if form.flow_is_active() {
        Some("step flow is active: press Enter to continue")
    } else {
        None
    }
}

pub fn value_with_caret(value: &str, cursor: usize, active: bool) -> String {
    if !active {
        return value.to_string();
    }
    let i = cursor.min(value.len());
    let mut shown = String::with_capacity(value.len() + 1);
    shown.push_str(&value[..i]);
    shown.push('|');
    shown.push_str(&value[i..]);
    shown
}

pub fn validate_send_form(form: &SendForm) -> Result<(AccountId, AccountId, u128, u128), String> {
    let from = parse_account_id(&form.from).map_err(|e| format!("from: {e}"))?;
    let to = parse_acct_id_ui(form.to.as_str()).map_err(|e| format!("to: {e}"))?;
    let amount =
        parse_decimal_pwm_units(form.amount.as_str().trim()).map_err(|e| format!("amount: {e}"))?;
    if amount == 0 {
        return Err("amount must be > 0".into());
    }
    let fee = parse_decimal_pwm_units(form.fee.as_str().trim()).map_err(|e| format!("fee: {e}"))?;
    if form.confirm.as_str().trim().to_lowercase() != "yes" {
        return Err("confirm must be 'yes'".into());
    }
    Ok((from, to, amount, fee))
}
