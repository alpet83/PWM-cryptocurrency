//! Percentage hints for modal amount inputs.

use pwm_core::parse_decimal_pwm_units;

/// Fixed display width for amount/marks input cell so the pct label stays put.
pub(crate) const MODAL_AMOUNT_INPUT_WIDTH: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AmountPctHint {
    pub label: String,
    pub over_limit: bool,
}

pub(crate) fn format_pct_of_limit(parsed: u128, limit: u128) -> Option<String> {
    if limit == 0 {
        return None;
    }
    let pct = (parsed as f64) * 100.0 / (limit as f64);
    if pct >= 100.0 || pct >= 10.0 {
        Some(format!("{pct:.1}%"))
    } else {
        Some(format!("{pct:.2}%"))
    }
}

pub(crate) fn amount_pct_hint(parsed: u128, limit: u128, unit: &str) -> Option<AmountPctHint> {
    let pct = format_pct_of_limit(parsed, limit)?;
    Some(AmountPctHint {
        over_limit: parsed > limit,
        label: format!("{pct} of {unit}"),
    })
}

pub(crate) fn mark_pct_hint(raw: &str, limit: u32) -> Option<AmountPctHint> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parsed = trimmed.parse::<u128>().ok()?;
    amount_pct_hint(parsed, u128::from(limit), "marks")
}

pub(crate) fn pwm_pct_hint(raw: &str, limit: u128) -> Option<AmountPctHint> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parsed = parse_decimal_pwm_units(trimmed).ok()?;
    amount_pct_hint(parsed, limit, "balance")
}

pub(crate) fn pad_input_field(display: &str, width: usize) -> String {
    let char_count = display.chars().count();
    if char_count >= width {
        display.chars().take(width).collect()
    } else {
        format!("{display}{}", " ".repeat(width - char_count))
    }
}

#[cfg(test)]
mod tests {
    use super::{format_pct_of_limit, mark_pct_hint, pad_input_field, pwm_pct_hint, AmountPctHint};

    #[test]
    fn pct_hint_empty_input() {
        assert_eq!(mark_pct_hint("", 10), None);
        assert_eq!(pwm_pct_hint("  ", 1_000_000), None);
    }

    #[test]
    fn pct_hint_parse_fail() {
        assert_eq!(mark_pct_hint("abc", 10), None);
        assert_eq!(pwm_pct_hint("1.x", 1_000_000), None);
    }

    #[test]
    fn pct_hint_zero_limit() {
        assert_eq!(format_pct_of_limit(1, 0), None);
        assert_eq!(mark_pct_hint("1", 0), None);
    }

    #[test]
    fn pct_hint_exact_limit() {
        assert_eq!(format_pct_of_limit(10, 10).as_deref(), Some("100.0%"));
        assert_eq!(
            mark_pct_hint("10", 10),
            Some(AmountPctHint {
                label: "100.0% of marks".into(),
                over_limit: false,
            })
        );
    }

    #[test]
    fn pct_hint_over_limit_pct() {
        assert_eq!(format_pct_of_limit(11, 10).as_deref(), Some("110.0%"));
        assert_eq!(
            pwm_pct_hint("2", 1_000_000),
            Some(AmountPctHint {
                label: "200.0% of balance".into(),
                over_limit: true,
            })
        );
    }

    #[test]
    fn pct_hint_precision() {
        assert_eq!(format_pct_of_limit(1, 20).as_deref(), Some("5.00%"));
        assert_eq!(format_pct_of_limit(1, 5).as_deref(), Some("20.0%"));
    }

    #[test]
    fn pad_input_field_fixed_width() {
        assert_eq!(pad_input_field("1", 5), "1    ");
        assert_eq!(pad_input_field("123456", 4), "1234");
    }
}
