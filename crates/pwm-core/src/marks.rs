use crate::display::PWM_RAW_SCALE;
use crate::genesis::GenCfg;
use crate::types::Account;

pub const MARKS_CAP: u32 = u32::MAX;

const PPM_DENOM: u128 = 1_000_000;

fn ceil_div_u64(numer: u64, denom: u64) -> u64 {
    if numer == 0 {
        return 0;
    }
    1 + ((numer - 1) / denom)
}

/// Computes effective marks at `current_height` without mutating account state.
pub fn compute_lazy_marks(account: &Account, current_height: u64, gen_cfg: &GenCfg) -> u32 {
    let stored = account.stored_marks;
    if stored == MARKS_CAP || current_height <= account.marks_last_block {
        return stored;
    }

    if gen_cfg.blocks_per_hour == 0 {
        return stored;
    }

    let delta_blocks = current_height.saturating_sub(account.marks_last_block);
    let delta_hours = delta_blocks / gen_cfg.blocks_per_hour;
    if delta_hours == 0 {
        return stored;
    }

    let whole_pwm_staked = account.staked_pwm_raw / PWM_RAW_SCALE;
    let rate = u64::from(gen_cfg.marks_per_hour);
    if whole_pwm_staked == 0 || rate == 0 {
        return stored;
    }

    let remaining = u64::from(MARKS_CAP - stored);
    let whole_pwm_staked = u64::try_from(whole_pwm_staked).unwrap_or(u64::MAX);
    let per_hour = whole_pwm_staked.saturating_mul(rate);
    if per_hour == 0 {
        return stored;
    }

    let satur_hours = ceil_div_u64(remaining, per_hour);
    let effective_hours = delta_hours.min(satur_hours);
    let generated = per_hour.saturating_mul(effective_hours);

    u64::from(stored)
        .saturating_add(generated)
        .min(u64::from(MARKS_CAP)) as u32
}

/// Computes deterministic per-block reward from V5 float inflation parameters.
pub fn compute_block_reward(gen_cfg: &GenCfg, _block_height: u64) -> u128 {
    if gen_cfg.season_coeff_ppm == 0 {
        return gen_cfg.block_reward;
    }
    gen_cfg
        .base_emission_per_block
        .saturating_mul(u128::from(gen_cfg.season_coeff_ppm))
        / PPM_DENOM
}

#[cfg(test)]
mod tests {
    use super::{compute_block_reward, compute_lazy_marks};
    use crate::display::PWM_RAW_SCALE;
    use crate::genesis::dev_net;
    use crate::types::Account;
    use crate::MARKS_CAP;

    #[test]
    fn marks_zero_stake_no_generation() {
        let (cfg, _) = dev_net();
        let acc = Account {
            stored_marks: 123,
            marks_last_block: 0,
            staked_pwm_raw: 0,
            ..Account::default()
        };

        assert_eq!(
            compute_lazy_marks(&acc, cfg.blocks_per_hour * 10, &cfg),
            123
        );
    }

    #[test]
    // Ensures ceil-based saturation reaches full cap for large stake.
    fn marks_1m_pwm_ceil_cap() {
        let (mut cfg, _) = dev_net();
        cfg.blocks_per_hour = 3_600;
        cfg.marks_per_hour = 1;

        let acc = Account {
            stored_marks: 0,
            marks_last_block: 0,
            staked_pwm_raw: 1_000_000u128 * PWM_RAW_SCALE,
            ..Account::default()
        };

        let h = 4_295u64 * cfg.blocks_per_hour;
        assert_eq!(compute_lazy_marks(&acc, h, &cfg), MARKS_CAP);
    }

    #[test]
    fn marks_saturated_account_is_noop() {
        let (cfg, _) = dev_net();
        let acc = Account {
            stored_marks: MARKS_CAP,
            marks_last_block: 0,
            staked_pwm_raw: 10 * PWM_RAW_SCALE,
            ..Account::default()
        };

        assert_eq!(
            compute_lazy_marks(&acc, cfg.blocks_per_hour * 100, &cfg),
            MARKS_CAP
        );
    }

    #[test]
    fn inflation_neutral_ppm_base() {
        let (mut cfg, _) = dev_net();
        cfg.base_emission_per_block = 123_456;
        cfg.season_coeff_ppm = 1_000_000;

        assert_eq!(compute_block_reward(&cfg, 0), 123_456);
    }

    #[test]
    fn inflation_zero_ppm_fallback() {
        let (mut cfg, _) = dev_net();
        cfg.base_emission_per_block = 999_999;
        cfg.block_reward = 77;
        cfg.season_coeff_ppm = 0;

        assert_eq!(compute_block_reward(&cfg, 0), 77);
    }

    #[test]
    fn inflation_sat_mul_no_ovf() {
        let (mut cfg, _) = dev_net();
        cfg.base_emission_per_block = u128::MAX;
        cfg.season_coeff_ppm = 2_000_000;

        assert_eq!(compute_block_reward(&cfg, 0), u128::MAX / 1_000_000);
    }
}
