//! Account-info command: fetch account/head and print marks detail snapshot.

use crate::rpc_helpers::{map_reqwest_err, truncate_rpc_body_hint};
use crate::wallet::load_wallet_yaml_upgrade;
use crate::{exit_user_error, http_client_for_rpc};
use pwm_core::compute_lazy_marks;
use pwm_core::genesis::{
    GenCfg, DEF_BASE_EMIT, DEF_BLOCKS_PER_HOUR, DEF_MARKS_HOUR, DEF_MARKS_STAKE_MIN,
    DEF_PWM_STAKE_MIN, DEF_SEASON_COEFF_PPM,
};
use pwm_core::types::Account;
use pwm_core::MARKS_CAP;
use pwm_core::{account_id_to_human, parse_account_id, AccountId, FundingCfg, RewPol, ValCfg};
use serde_json::Value;
use std::path::PathBuf;

pub(crate) struct AcctInfo {
    pub(crate) marks: u32,
    pub(crate) marks_last_block: u64,
    pub(crate) staked: u128,
}

pub(crate) fn run_account_info(
    rpc_base: &str,
    account: Option<String>,
    wallet: Option<PathBuf>,
    upgrade_wallet: bool,
) {
    let acct_id = resolve_acct_id(account, wallet, upgrade_wallet).unwrap_or_else(|e| {
        exit_user_error(&e);
    });
    let c = http_client_for_rpc();
    let head = fetch_head_h(&c, rpc_base).unwrap_or_else(|e| exit_user_error(&e));
    let acct = fetch_acct(&c, rpc_base, acct_id).unwrap_or_else(|e| exit_user_error(&e));
    let effective = calc_eff_marks(acct.marks, acct.marks_last_block, acct.staked, head);
    let sat_pct = calc_sat_pct(effective);

    println!("account={}", account_id_to_human(&acct_id));
    println!("head_height={head}");
    println!("marks_stored={}", acct.marks);
    println!("marks_effective={effective}");
    println!("marks_sat_pct={sat_pct}");
    println!("marks_last_block={}", acct.marks_last_block);
    println!("staked={}", acct.staked);
}

fn resolve_acct_id(
    account: Option<String>,
    wallet: Option<PathBuf>,
    upgrade_wallet: bool,
) -> Result<AccountId, String> {
    if let Some(raw) = account {
        return parse_account_id(raw.trim()).map_err(|e| format!("invalid --account: {e}"));
    }
    let wallet = wallet.ok_or_else(|| "set --account or --wallet".to_string())?;
    let id_hex = read_wallet_id(wallet, upgrade_wallet)?;
    parse_account_id(id_hex.trim()).map_err(|e| format!("invalid wallet account id: {e}"))
}

fn read_wallet_id(wallet: PathBuf, upgrade_wallet: bool) -> Result<String, String> {
    let doc = load_wallet_yaml_upgrade(&wallet, upgrade_wallet)
        .map_err(|e| format!("failed to read wallet '{}': {e}", wallet.display()))?;
    Ok(doc.account_id_hex)
}

fn fetch_head_h(c: &reqwest::blocking::Client, rpc_base: &str) -> Result<u64, String> {
    let url = format!("{rpc_base}/v1/head");
    let r = c
        .get(&url)
        .send()
        .map_err(|e| map_reqwest_err(&e, "head fetch"))?;
    let status = r.status();
    let body = r.text().unwrap_or_default();
    if !status.is_success() {
        return Err(http_err("head fetch", status, &url, &body));
    }
    parse_head_h(&body)
}

fn fetch_acct(
    c: &reqwest::blocking::Client,
    rpc_base: &str,
    acct_id: AccountId,
) -> Result<AcctInfo, String> {
    let acct_hex = hex::encode(acct_id);
    let url = format!("{rpc_base}/v1/account/{acct_hex}");
    let r = c
        .get(&url)
        .send()
        .map_err(|e| map_reqwest_err(&e, "account fetch"))?;
    let status = r.status();
    let body = r.text().unwrap_or_default();
    if !status.is_success() {
        return Err(http_err("account fetch", status, &url, &body));
    }
    parse_acct_body(&body)
}

pub(crate) fn parse_head_h(body: &str) -> Result<u64, String> {
    let v: Value =
        serde_json::from_str(body).map_err(|e| format!("head fetch: invalid JSON: {e}"))?;
    parse_u64_field(&v, "height")
        .ok_or_else(|| format!("head fetch: missing/invalid `height`. {}", body_hint(body)))
}

pub(crate) fn parse_acct_body(body: &str) -> Result<AcctInfo, String> {
    let v: Value = serde_json::from_str(body)
        .map_err(|e| format!("account fetch: invalid /v1/account JSON: {e}"))?;
    let marks = parse_u32_field(&v, "marks").ok_or_else(|| {
        format!(
            "account fetch: missing/invalid `marks`. {}",
            body_hint(body)
        )
    })?;
    let marks_last_block = parse_u64_field(&v, "marks_last_block").ok_or_else(|| {
        format!(
            "account fetch: missing/invalid `marks_last_block`. {}",
            body_hint(body)
        )
    })?;
    let staked = parse_u128_field(&v, "staked").ok_or_else(|| {
        format!(
            "account fetch: missing/invalid `staked`. {}",
            body_hint(body)
        )
    })?;
    Ok(AcctInfo {
        marks,
        marks_last_block,
        staked,
    })
}

fn parse_u64_field(v: &Value, field: &str) -> Option<u64> {
    v.get(field).and_then(|n| match n {
        Value::Number(num) => num.as_u64(),
        Value::String(s) => s.parse::<u64>().ok(),
        _ => None,
    })
}

fn parse_u32_field(v: &Value, field: &str) -> Option<u32> {
    parse_u64_field(v, field).and_then(|x| u32::try_from(x).ok())
}

fn parse_u128_field(v: &Value, field: &str) -> Option<u128> {
    v.get(field).and_then(|n| match n {
        Value::Number(num) => num.as_u64().map(u128::from),
        Value::String(s) => s.parse::<u128>().ok(),
        _ => None,
    })
}

fn body_hint(body: &str) -> String {
    let hint = truncate_rpc_body_hint(body, 160);
    if hint.is_empty() {
        "(empty body)".to_string()
    } else {
        hint
    }
}

fn http_err(ctx: &str, status: reqwest::StatusCode, url: &str, body: &str) -> String {
    let hint = truncate_rpc_body_hint(body, 240);
    if hint.is_empty() {
        format!("{ctx}: HTTP {status} from {url}")
    } else {
        format!("{ctx}: HTTP {status} from {url}: {hint}")
    }
}

fn mk_gen_cfg() -> GenCfg {
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
        pwm_stake_min: DEF_PWM_STAKE_MIN,
        marks_stake_min: DEF_MARKS_STAKE_MIN,
        season_enabled: false,
        season_coeff_ppm: DEF_SEASON_COEFF_PPM,
    }
}

fn mk_eff_acct(marks: u32, marks_last_block: u64, staked: u128) -> Account {
    Account {
        stored_marks: marks,
        marks_last_block,
        staked_pwm_raw: staked,
        ..Account::default()
    }
}

pub(crate) fn calc_eff_marks(marks: u32, marks_last_block: u64, staked: u128, head: u64) -> u32 {
    let acct = mk_eff_acct(marks, marks_last_block, staked);
    let cfg = mk_gen_cfg();
    compute_lazy_marks(&acct, head, &cfg)
}

pub(crate) fn calc_sat_pct(effective: u32) -> u8 {
    if effective == 0 {
        return 0;
    }
    let pct = (u128::from(effective) * 100) / u128::from(MARKS_CAP);
    u8::try_from(pct).unwrap_or(100)
}
