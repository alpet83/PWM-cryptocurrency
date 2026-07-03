//! Effective marks and saturation formatting for account table rows.

use crate::AcctRow;
use pwm_core::compute_lazy_marks;
use pwm_core::genesis::{
    GenCfg, DEF_BASE_EMIT, DEF_BLOCKS_PER_HOUR, DEF_CONSERV_DELAY_BLOCKS, DEF_EPOCH_LEN_BLOCKS,
    DEF_MARKS_HOUR, DEF_MARKS_STAKE_MIN, DEF_PWM_STAKE_MIN, DEF_SEASON_COEFF_PPM,
    DEF_XSHARD_LOCK_TO,
};
use pwm_core::types::Account;
use pwm_core::MARKS_CAP;
use pwm_core::{FundingCfg, RewPol, ValCfg};

fn mk_marks_cfg() -> GenCfg {
    GenCfg {
        funding: FundingCfg {
            accounts: Vec::new(),
        },
        vals: ValCfg { set: Vec::new() },
        rew: RewPol::ToProducerAccount,
        accounts: Vec::new(),
        blocks_per_hour: DEF_BLOCKS_PER_HOUR,
        marks_per_hour: DEF_MARKS_HOUR,
        ipv4_claim_phases: Vec::new(),
        block_reward: 0,
        marks_coeff: 0,
        policy_ver: pwm_core::genesis::LEGACY_POLICY_VER,
        base_emission_per_block: DEF_BASE_EMIT,
        min_validator_stake: DEF_PWM_STAKE_MIN,
        epoch_length_blocks: DEF_EPOCH_LEN_BLOCKS,
        conservation_delay_blocks: DEF_CONSERV_DELAY_BLOCKS,
        xshard_lock_to_blocks: DEF_XSHARD_LOCK_TO,
        pwm_stake_min: DEF_PWM_STAKE_MIN,
        marks_stake_min: DEF_MARKS_STAKE_MIN,
        season_enabled: false,
        season_coeff_ppm: DEF_SEASON_COEFF_PPM,
    }
}

fn mk_marks_acct(row: &AcctRow) -> Account {
    Account {
        stored_marks: row.marks,
        marks_last_block: row.marks_last_block,
        staked_pwm_raw: row.staked,
        ..Account::default()
    }
}

pub(crate) fn effective_marks_at_height(row: &AcctRow, head: u64) -> u32 {
    let acct = mk_marks_acct(row);
    let cfg = mk_marks_cfg();
    compute_lazy_marks(&acct, head, &cfg)
}

pub(crate) fn marks_sat_pct(effective: u32) -> u8 {
    if effective == 0 {
        return 0;
    }
    let pct = (u128::from(effective) * 100) / u128::from(MARKS_CAP);
    u8::try_from(pct).unwrap_or(100)
}

pub(crate) fn format_amount_compact(amount: u128) -> String {
    const K: f64 = 1_000.0;
    const M: f64 = 1_000_000.0;
    const B: f64 = 1_000_000_000.0;

    let value = amount as f64;
    if amount < 1_000 {
        amount.to_string()
    } else if amount < 1_000_000 {
        format!("{:.2}K", value / K)
    } else if amount < 1_000_000_000 {
        format!("{:.2}M", value / M)
    } else {
        format!("{:.2}B", value / B)
    }
}

pub(crate) fn format_marks_compact(effective: u32) -> String {
    format_amount_compact(u128::from(effective))
}

#[cfg(test)]
mod tests {
    use super::{
        effective_marks_at_height, format_amount_compact, format_marks_compact, marks_sat_pct,
    };
    use crate::AcctRow;
    use pwm_core::MARKS_CAP;

    fn mk_row() -> AcctRow {
        AcctRow {
            id: [0u8; 32],
            id_hex: String::new(),
            balance_pwm: 0,
            initialized: true,
            nonce: 0,
            marks: 10,
            marks_last_block: 0,
            effective_marks: None,
            marks_sat_pct: None,
            pending_conservation: Vec::new(),
            staked: 0,
            rescue_address: None,
            active_policies: 0,
            dormant_policies: 0,
            finalized: false,
            owner_kind: String::new(),
            owner_name: String::new(),
            owner_country: String::new(),
            label: None,
        }
    }

    #[test]
    fn marks_display_zero_stake() {
        let row = mk_row();
        let eff = effective_marks_at_height(&row, 9999);
        assert_eq!(eff, 10);
        assert_eq!(marks_sat_pct(eff), 0);
    }

    #[test]
    fn marks_display_sat_cap() {
        let mut row = mk_row();
        row.marks = MARKS_CAP;
        let eff = effective_marks_at_height(&row, 9999);
        assert_eq!(eff, MARKS_CAP);
        assert_eq!(marks_sat_pct(eff), 100);
    }

    #[test]
    fn marks_display_lazy_delta() {
        let mut row = mk_row();
        row.staked = 5_000_000;
        row.marks_last_block = 0;
        let eff = effective_marks_at_height(&row, 3600);
        assert_eq!(eff, 15);
        assert_eq!(format_marks_compact(eff), "15");
    }

    #[test]
    fn marks_compact_scale() {
        assert_eq!(format_marks_compact(999), "999");
        assert_eq!(format_marks_compact(1_500), "1.50K");
        assert_eq!(format_marks_compact(2_500_000), "2.50M");
        assert_eq!(format_marks_compact(3_000_000_000), "3.00B");
        assert_eq!(format_marks_compact(MARKS_CAP), "4.29B");
    }

    #[test]
    fn amount_compact_scale() {
        assert_eq!(format_amount_compact(999), "999");
        assert_eq!(format_amount_compact(1_500), "1.50K");
        assert_eq!(format_amount_compact(2_500_000), "2.50M");
        assert_eq!(format_amount_compact(3_000_000_000), "3.00B");
    }
}
