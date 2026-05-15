//! Stake/unstake modal form state and validation.

use pwm_core::{parse_account_id, parse_decimal_pwm_units, AccountId};

use crate::modals::TextInput;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StakeMode {
    Stake,
    Unstake,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StakeField {
    Amount,
    Confirm,
}

pub struct StakeForm {
    pub mode: StakeMode,
    pub from: String,
    pub amount: TextInput,
    pub confirm: TextInput,
    pub active: StakeField,
    pub status: String,
    pub status_is_error: bool,
    pub limit_units: u128,
}

impl StakeForm {
    pub fn new(mode: StakeMode, from: String, balance_pwm: u128, staked: u128) -> Self {
        let status = match mode {
            StakeMode::Stake => "Enter amount and type 'yes' to submit stake.",
            StakeMode::Unstake => "Enter amount and type 'yes' to submit unstake.",
        };
        let limit_units = match mode {
            StakeMode::Stake => balance_pwm,
            StakeMode::Unstake => staked,
        };
        Self {
            mode,
            from,
            amount: TextInput::new(),
            confirm: TextInput::new(),
            active: StakeField::Amount,
            status: status.into(),
            status_is_error: false,
            limit_units,
        }
    }

    pub fn title(&self) -> &'static str {
        match self.mode {
            StakeMode::Stake => "F7 Stake PWM",
            StakeMode::Unstake => "Shift+F7 Unstake PWM",
        }
    }

    pub fn limit_label(&self) -> &'static str {
        match self.mode {
            StakeMode::Stake => "Available",
            StakeMode::Unstake => "Staked",
        }
    }

    fn active_input_mut(&mut self) -> &mut TextInput {
        match self.active {
            StakeField::Amount => &mut self.amount,
            StakeField::Confirm => &mut self.confirm,
        }
    }

    pub fn next_field(&mut self) {
        self.active = match self.active {
            StakeField::Amount => StakeField::Confirm,
            StakeField::Confirm => StakeField::Amount,
        };
    }

    pub fn prev_field(&mut self) {
        self.next_field();
    }

    pub fn clamp_active_cursor(&mut self) {
        self.active_input_mut().clamp_cursor();
    }

    pub fn move_left(&mut self) {
        self.active_input_mut().move_left();
    }

    pub fn move_right(&mut self) {
        self.active_input_mut().move_right();
    }

    pub fn move_home(&mut self) {
        self.active_input_mut().move_home();
    }

    pub fn move_end(&mut self) {
        self.active_input_mut().move_end();
    }

    pub fn insert_char(&mut self, c: char) {
        self.active_input_mut().insert_char(c);
    }

    pub fn backspace(&mut self) {
        self.active_input_mut().backspace();
    }

    pub fn delete(&mut self) {
        self.active_input_mut().delete();
    }
}

pub fn validate_stake_form(form: &StakeForm) -> Result<(AccountId, u128), String> {
    let from = parse_account_id(&form.from).map_err(|e| format!("from: {e}"))?;
    let amount =
        parse_decimal_pwm_units(form.amount.as_str().trim()).map_err(|e| format!("amount: {e}"))?;
    if amount == 0 {
        return Err("amount must be > 0".into());
    }
    if amount > form.limit_units {
        let what = match form.mode {
            StakeMode::Stake => "available balance",
            StakeMode::Unstake => "staked amount",
        };
        return Err(format!("amount exceeds {what}"));
    }
    if form.confirm.as_str().trim().to_lowercase() != "yes" {
        return Err("confirm must be 'yes'".into());
    }
    Ok((from, amount))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::f5_burn_hint_needed;
    use pwm_core::account_id_to_human;

    fn fixture_form(
        mode: StakeMode,
        bal: u128,
        staked: u128,
        amount: &str,
        confirm: &str,
    ) -> StakeForm {
        let mut form = StakeForm::new(mode, account_id_to_human(&[11u8; 32]), bal, staked);
        form.amount.set_text(amount.to_string());
        form.confirm.set_text(confirm.to_string());
        form
    }

    #[test]
    fn stake_form_valid_amount() {
        let form = fixture_form(StakeMode::Stake, 10_000_000, 0, "1.5", "yes");
        let (id, units) = validate_stake_form(&form).unwrap();
        assert_eq!(id, [11u8; 32]);
        assert_eq!(units, 1_500_000);
    }

    #[test]
    fn stake_form_zero_rejects() {
        for z in ["0", "0.0"] {
            let form = fixture_form(StakeMode::Stake, 99, 0, z, "yes");
            let err = validate_stake_form(&form).unwrap_err();
            assert!(err.contains("amount"), "z={z} err={err}");
        }
    }

    #[test]
    fn stake_form_bad_input() {
        for raw in ["abc", "-1", ""] {
            let form = fixture_form(StakeMode::Stake, 99, 0, raw, "yes");
            assert!(validate_stake_form(&form).is_err(), "raw={raw:?}");
        }
    }

    #[test]
    fn unstake_form_valid() {
        let lim = 5_000_000u128;
        let form = fixture_form(StakeMode::Unstake, 999, lim, "1.5", "yes");
        let (_, units) = validate_stake_form(&form).unwrap();
        assert_eq!(units, 1_500_000);
    }

    /// F5 burn path shows stake-first hint only when both staked balance and marks are zero.
    #[test]
    fn hint_no_stake_no_marks() {
        assert!(f5_burn_hint_needed(0, 0));
        assert!(!f5_burn_hint_needed(1, 0));
        assert!(!f5_burn_hint_needed(0, 1));
        assert!(!f5_burn_hint_needed(2, 3));
    }
}
