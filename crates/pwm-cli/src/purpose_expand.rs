//! Expand supported placeholders in burn purpose text.

use std::time::{SystemTime, UNIX_EPOCH};

/// Expand `{utc_time}` and `{utc_timestamp}` placeholders in `purpose`.
/// Unknown `{...}` tokens are left unchanged.
pub fn expand_purpose(raw: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let ts = now.as_secs();
    let utc_time = fmt_utc_time(ts);
    raw.replace("{utc_timestamp}", &ts.to_string())
        .replace("{utc_time}", &utc_time)
}

fn fmt_utc_time(ts: u64) -> String {
    let days = (ts / 86_400) as i64;
    let sec_day = ts % 86_400;
    let (year, month, day) = civil_from_days(days);
    let hh = sec_day / 3_600;
    let mm = (sec_day % 3_600) / 60;
    let ss = sec_day % 60;
    format!(
        "{:02}-{:02}-{:02} {:02}:{:02}:{:02}Z",
        day,
        month,
        (year % 100).rem_euclid(100),
        hh,
        mm,
        ss
    )
}

fn civil_from_days(days: i64) -> (i64, i64, i64) {
    // Gregorian conversion from days since Unix epoch (1970-01-01).
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::expand_purpose;

    fn is_digits(raw: &str) -> bool {
        !raw.is_empty() && raw.chars().all(|ch| ch.is_ascii_digit())
    }

    fn is_time_fmt(raw: &str) -> bool {
        if raw.len() != 18 {
            return false;
        }
        let bytes = raw.as_bytes();
        bytes[2] == b'-'
            && bytes[5] == b'-'
            && bytes[8] == b' '
            && bytes[11] == b':'
            && bytes[14] == b':'
            && bytes[17] == b'Z'
            && bytes
                .iter()
                .enumerate()
                .filter(|(idx, _)| ![2, 5, 8, 11, 14, 17].contains(idx))
                .all(|(_, b)| b.is_ascii_digit())
    }

    #[test]
    fn expands_utc_timestamp() {
        let expanded = expand_purpose("{utc_timestamp}");
        assert!(is_digits(&expanded));
    }

    #[test]
    fn expands_utc_time() {
        let expanded = expand_purpose("{utc_time}");
        assert!(is_time_fmt(&expanded));
    }

    #[test]
    fn keeps_unknown_placeholder() {
        let expanded = expand_purpose("burn {foo}");
        assert_eq!(expanded, "burn {foo}");
    }

    #[test]
    fn expands_combined_template() {
        let expanded = expand_purpose("burn {utc_timestamp} at {utc_time}");
        let parts: Vec<&str> = expanded.split(" at ").collect();
        assert_eq!(parts.len(), 2);
        assert!(parts[0].starts_with("burn "));
        assert!(is_digits(parts[0].trim_start_matches("burn ")));
        assert!(is_time_fmt(parts[1]));
    }
}
