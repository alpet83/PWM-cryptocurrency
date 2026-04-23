use ed25519_dalek::SigningKey;
use pwm_core::hd::{account_id_from_parts, domain_of_account_id};
use pwm_core::AccountId;
use slip10_ed25519::derive_ed25519_private_key;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BruteforceMatch {
    pub signing_key: [u8; 32],
    pub verifying_key: [u8; 32],
    pub derivation_index: u32,
    pub account_id: AccountId,
    pub domain: u16,
    pub derived_flags: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BruteforceProgress {
    pub checked: u64,
    pub elapsed_sec: f64,
    pub attempts_per_sec: f64,
    pub expected_total: f64,
    pub eta_sec: f64,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DomainMatchMode {
    FullU16,
    HighByteOnly,
}

pub fn flags_from_account_id(id: &AccountId) -> u32 {
    u32::from_be_bytes([id[2], id[3], id[4], id[5]])
}

#[allow(dead_code)]
pub fn matches_flags_mask(flags: u32, mask: u32) -> bool {
    (flags & mask) == mask
}

pub fn matches_flags_expected(flags: u32, mask: u32, expected_flags: u32) -> bool {
    (flags & mask) == expected_flags
}

pub fn domain_matches(derived: u16, expected: u16, mode: DomainMatchMode) -> bool {
    match mode {
        DomainMatchMode::FullU16 => derived == expected,
        DomainMatchMode::HighByteOnly => (derived >> 8) == (expected >> 8),
    }
}

pub fn expected_attempts(flags_mask: u32, domain_mode: DomainMatchMode) -> f64 {
    let domain_bits = match domain_mode {
        DomainMatchMode::FullU16 => 16,
        DomainMatchMode::HighByteOnly => 8,
    };
    let bits = flags_mask.count_ones() as i32;
    2f64.powi(domain_bits + bits)
}

pub fn eta_seconds(checked: u64, attempts_per_sec: f64, expected_total: f64) -> f64 {
    if attempts_per_sec <= 0.0 {
        return f64::INFINITY;
    }
    let remaining = (expected_total - checked as f64).max(0.0);
    remaining / attempts_per_sec
}

pub fn format_eta_human(sec: f64) -> String {
    if !sec.is_finite() {
        return "unknown".to_string();
    }
    if sec < 3600.0 {
        return format!("{:.1} min", sec / 60.0);
    }
    let hours = sec / 3600.0;
    if hours < 24.0 {
        return format!("{hours:.1} h");
    }
    let days = hours / 24.0;
    if days < 7.0 {
        return format!("{days:.1} d");
    }
    let weeks = days / 7.0;
    format!("{weeks:.1} w")
}

#[allow(dead_code)]
pub fn brute_force_domain_flags(
    master_seed: &[u8; 32],
    domain: u16,
    domain_mode: DomainMatchMode,
    flags_mask: u32,
    expected_flags: u32,
    max_try: u32,
) -> Option<BruteforceMatch> {
    brute_force_domain_flags_with_progress(
        master_seed,
        domain,
        domain_mode,
        flags_mask,
        expected_flags,
        max_try,
        0,
        |_| {},
    )
}

pub fn brute_force_domain_flags_with_progress<F: FnMut(BruteforceProgress)>(
    master_seed: &[u8; 32],
    domain: u16,
    domain_mode: DomainMatchMode,
    flags_mask: u32,
    expected_flags: u32,
    max_try: u32,
    progress_interval_sec: u64,
    on_progress: F,
) -> Option<BruteforceMatch> {
    brute_force_domain_flags_with_progress_and_match_policy(
        master_seed,
        domain,
        domain_mode,
        flags_mask,
        expected_flags,
        max_try,
        progress_interval_sec,
        on_progress,
        |_| true,
    )
}

pub fn brute_force_domain_flags_with_progress_and_match_policy<
    F: FnMut(BruteforceProgress),
    P: Fn(u16) -> bool,
>(
    master_seed: &[u8; 32],
    domain: u16,
    domain_mode: DomainMatchMode,
    flags_mask: u32,
    expected_flags: u32,
    max_try: u32,
    progress_interval_sec: u64,
    mut on_progress: F,
    accept_domain: P,
) -> Option<BruteforceMatch> {
    if (expected_flags & !flags_mask) != 0 {
        return None;
    }
    let started = Instant::now();
    let expected_total = expected_attempts(flags_mask, domain_mode);
    let mut next_progress_at = started + Duration::from_secs(progress_interval_sec);

    for i in 0..=max_try {
        let sk_bytes = derive_ed25519_private_key(master_seed, &[0, i]);
        let sk = SigningKey::from_bytes(&sk_bytes);
        let pk = sk.verifying_key().to_bytes();
        let aid = account_id_from_parts(&pk, i);
        let derived_domain = domain_of_account_id(&aid);
        let derived_flags = flags_from_account_id(&aid);

        if progress_interval_sec > 0 {
            let now = Instant::now();
            if now >= next_progress_at {
                let checked = i as u64 + 1;
                let elapsed_sec = started.elapsed().as_secs_f64();
                let attempts_per_sec = if elapsed_sec > 0.0 {
                    checked as f64 / elapsed_sec
                } else {
                    0.0
                };
                on_progress(BruteforceProgress {
                    checked,
                    elapsed_sec,
                    attempts_per_sec,
                    expected_total,
                    eta_sec: eta_seconds(checked, attempts_per_sec, expected_total),
                });
                next_progress_at = now + Duration::from_secs(progress_interval_sec);
            }
        }

        if domain_matches(derived_domain, domain, domain_mode)
            && matches_flags_expected(derived_flags, flags_mask, expected_flags)
            && accept_domain(derived_domain)
        {
            return Some(BruteforceMatch {
                signing_key: sk.to_bytes(),
                verifying_key: pk,
                derivation_index: i,
                account_id: aid,
                domain: derived_domain,
                derived_flags,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use pwm_core::hd::account_id_from_parts;
    use slip10_ed25519::derive_ed25519_private_key;

    #[test]
    fn brute_force_finds_i0_for_exact_mask() {
        let seed = [9u8; 32];
        let sk0 = SigningKey::from_bytes(&derive_ed25519_private_key(&seed, &[0, 0]));
        let pk0 = sk0.verifying_key().to_bytes();
        let aid0 = account_id_from_parts(&pk0, 0);
        let domain = domain_of_account_id(&aid0);
        let mask = flags_from_account_id(&aid0);
        let expected = mask & mask;

        let hit = brute_force_domain_flags(
            &seed,
            domain,
            DomainMatchMode::FullU16,
            mask,
            expected,
            10_000,
        )
        .expect("must find i=0");
        assert_eq!(hit.derivation_index, 0);
        assert_eq!(hit.account_id, aid0);
        assert_eq!(hit.derived_flags, mask);
    }

    #[test]
    fn expected_must_fit_mask() {
        let seed = [1u8; 32];
        let domain = 0x007E;
        let mask = 0x0000_FF00;
        let expected_outside = 0x0001_0000;
        let hit = brute_force_domain_flags(
            &seed,
            domain,
            DomainMatchMode::FullU16,
            mask,
            expected_outside,
            100,
        );
        assert!(hit.is_none());
    }

    #[test]
    fn expected_attempts_grows_by_mask_bits() {
        assert_eq!(expected_attempts(0, DomainMatchMode::FullU16), 65536.0);
        assert_eq!(expected_attempts(0b1, DomainMatchMode::FullU16), 131072.0);
        assert_eq!(
            expected_attempts(0b1011, DomainMatchMode::FullU16),
            524288.0
        );
    }

    #[test]
    fn expected_attempts_user_profile_10bit_is_2pow18() {
        assert_eq!(
            expected_attempts(0x03FF, DomainMatchMode::HighByteOnly),
            262144.0
        );
    }

    #[test]
    fn high_byte_mode_uses_only_domain_hi() {
        let expected = 0x12AB;
        let same_hi = 0x12FF;
        let other_hi = 0x13AB;
        assert!(domain_matches(
            same_hi,
            expected,
            DomainMatchMode::HighByteOnly
        ));
        assert!(!domain_matches(
            other_hi,
            expected,
            DomainMatchMode::HighByteOnly
        ));
        assert!(!domain_matches(same_hi, expected, DomainMatchMode::FullU16));
    }

    #[test]
    fn eta_format_scales_to_weeks() {
        assert_eq!(format_eta_human(7200.0), "2.0 h");
        assert_eq!(format_eta_human(172800.0), "2.0 d");
        assert_eq!(format_eta_human(1209600.0), "2.0 w");
    }

    #[test]
    fn match_policy_can_reject_otherwise_valid_domain_hit() {
        let seed = [12u8; 32];
        let sk0 = SigningKey::from_bytes(&derive_ed25519_private_key(&seed, &[0, 0]));
        let pk0 = sk0.verifying_key().to_bytes();
        let aid0 = account_id_from_parts(&pk0, 0);
        let domain = domain_of_account_id(&aid0);
        let flags = flags_from_account_id(&aid0);

        let allowed = brute_force_domain_flags_with_progress_and_match_policy(
            &seed,
            domain,
            DomainMatchMode::FullU16,
            flags,
            flags,
            1024,
            0,
            |_| {},
            |_| true,
        );
        assert!(allowed.is_some());

        let denied = brute_force_domain_flags_with_progress_and_match_policy(
            &seed,
            domain,
            DomainMatchMode::FullU16,
            flags,
            flags,
            1024,
            0,
            |_| {},
            |_| false,
        );
        assert!(denied.is_none());
    }
}
