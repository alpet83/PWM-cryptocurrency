//! Human-readable PWM amounts (6 fractional decimals; 1 PWM = 1_000_000 raw).

/// Scale: 1 PWM == 1_000_000 raw units.
pub const PWM_RAW_SCALE: u128 = 1_000_000;

/// Pretty-print raw PWM units as decimal coin text (trims fractional zeros).
pub fn format_pwm(raw: u128) -> String {
    let whole = raw / PWM_RAW_SCALE;
    let frac = raw % PWM_RAW_SCALE;
    if frac == 0 {
        return format!("{whole} PWM");
    }
    let mut frac_text = format!("{frac:06}");
    while frac_text.ends_with('0') {
        frac_text.pop();
    }
    format!("{whole}.{frac_text} PWM")
}

/// Parse decimal PWM text into raw units (max 6 fractional digits).
pub fn parse_decimal_pwm_units(raw: &str) -> Result<u128, String> {
    let s = raw.trim();
    if s.is_empty() {
        return Err("value is required".into());
    }
    if s.starts_with('-') {
        return Err("negative values are not allowed".into());
    }
    let (whole, frac) = match s.split_once('.') {
        Some((w, f)) => (w, Some(f)),
        None => (s, None),
    };
    if whole.is_empty() || !whole.chars().all(|c| c.is_ascii_digit()) {
        return Err("must be a decimal number like 12.34".into());
    }
    let whole_units = whole
        .parse::<u128>()
        .map_err(|_| "numeric overflow".to_string())?
        .checked_mul(PWM_RAW_SCALE)
        .ok_or_else(|| "numeric overflow".to_string())?;
    let frac_units = if let Some(frac_raw) = frac {
        if frac_raw.is_empty() || !frac_raw.chars().all(|c| c.is_ascii_digit()) {
            return Err("must be a decimal number like 12.34".into());
        }
        if frac_raw.len() > 6 {
            return Err(
                "supports up to 6 decimal places (scale 1 PWM = 1_000_000 base units)".into(),
            );
        }
        let mut frac_padded = frac_raw.to_string();
        while frac_padded.len() < 6 {
            frac_padded.push('0');
        }
        frac_padded
            .parse::<u128>()
            .map_err(|_| "numeric overflow".to_string())?
    } else {
        0
    };
    whole_units
        .checked_add(frac_units)
        .ok_or_else(|| "numeric overflow".to_string())
}

#[cfg(test)]
mod tests {
    use super::{format_pwm, parse_decimal_pwm_units};

    /// Decimal PWM parser accepts ints and fractions (formerly `parse_decimal_accepts_integer_and_fraction`).
    #[test]
    fn dec_pwm_parse_ok_basic() {
        assert_eq!(parse_decimal_pwm_units("12").unwrap(), 12_000_000);
        assert_eq!(parse_decimal_pwm_units("12.34").unwrap(), 12_340_000);
        assert_eq!(parse_decimal_pwm_units("0.001").unwrap(), 1_000);
        assert_eq!(parse_decimal_pwm_units("0.000001").unwrap(), 1);
    }

    /// Reject junk / overprecision decimals (formerly `parse_decimal_rejects_invalid_and_overprecise_values`).
    #[test]
    fn dec_pwm_parse_reject_bad() {
        let bad = ["", " ", "-1", "abc", "1.", ".1", "1.1234567", "1,23"];
        for v in bad {
            assert!(
                parse_decimal_pwm_units(v).is_err(),
                "expected invalid value: {v}"
            );
        }
    }

    /// `format_pwm` shows micro‑PWM fractions (formerly `format_pwm_shows_decimal_coin_units`).
    #[test]
    fn fmt_pwm_frac_units_ok() {
        assert_eq!(format_pwm(0), "0 PWM");
        assert_eq!(format_pwm(1), "0.000001 PWM");
        assert_eq!(format_pwm(1_000_000), "1 PWM");
        assert_eq!(format_pwm(1_230_000), "1.23 PWM");
    }
}
