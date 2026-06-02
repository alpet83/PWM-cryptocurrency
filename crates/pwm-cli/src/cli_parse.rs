//! CLI argument parsing helpers (addresses, domains, amounts).

use pwm_core::{parse_acct_id_ui, AccountId};

pub(crate) const ADDRESS_FORMAT_HINT: &str =
    "pretty pwm1-<label_or_$hex!>-f<flags8hex>-t<tail52hex>, canonical pwm1..., legacy PWMv0-... / hex";

pub fn hex32(s: &str) -> Result<[u8; 32], String> {
    let v = hex::decode(s.trim()).map_err(|e| e.to_string())?;
    if v.len() != 32 {
        return Err("need 32 bytes hex".into());
    }
    let mut a = [0u8; 32];
    a.copy_from_slice(&v);
    Ok(a)
}

pub fn parse_domain(s: &str) -> Result<u16, String> {
    let t = s.trim();
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        return u16::from_str_radix(hex, 16).map_err(|e| e.to_string());
    }
    if t.chars()
        .any(|c| c.is_ascii_hexdigit() && c.is_ascii_alphabetic())
    {
        return u16::from_str_radix(t, 16).map_err(|e| e.to_string());
    }
    t.parse::<u16>()
        .or_else(|_| u16::from_str_radix(t, 16))
        .map_err(|e| e.to_string())
}

pub fn master_seed(s: &str) -> Result<[u8; 32], String> {
    hex32(s)
}

fn parse_amount_value(field: &str, value: &str) -> Result<u128, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{field} value must not be empty"));
    }
    trimmed
        .parse::<u128>()
        .map_err(|_| format!("{field} value must be an unsigned integer, got '{value}'"))
}

pub(crate) fn parse_address_arg(field: &str, value: &str) -> Result<AccountId, String> {
    parse_acct_id_ui(value).map_err(|e| {
        format!(
            "Invalid value for {field}: '{value}'. Accepted formats: {ADDRESS_FORMAT_HINT}. Parse details: {e}"
        )
    })
}

/// Accept plain address input and URI form `pwm:<address>?amount=<u128>`.
pub(crate) fn parse_address_input(
    field: &str,
    value: &str,
) -> Result<(AccountId, Option<u128>), String> {
    let trimmed = value.trim();
    if !trimmed.starts_with("pwm:") {
        return parse_address_arg(field, trimmed).map(|id| (id, None));
    }
    let rest = &trimmed["pwm:".len()..];
    if rest.is_empty() {
        return Err(format!(
            "Invalid value for {field}: '{value}'. malformed pwm URI: missing address after 'pwm:'"
        ));
    }
    if rest.starts_with("//") {
        return Err(format!(
            "Invalid value for {field}: '{value}'. malformed pwm URI: authority form is not supported; expected 'pwm:<address>'"
        ));
    }
    if rest.contains('#') {
        return Err(format!(
            "Invalid value for {field}: '{value}'. malformed pwm URI: fragments are not supported"
        ));
    }
    let (address_part, query_part) = match rest.split_once('?') {
        Some(parts) => parts,
        None => (rest, ""),
    };
    if address_part.trim().is_empty() {
        return Err(format!(
            "Invalid value for {field}: '{value}'. malformed pwm URI: missing address before query"
        ));
    }
    let address = parse_address_arg(field, address_part.trim())?;
    if query_part.is_empty() {
        return Ok((address, None));
    }
    let mut amount: Option<u128> = None;
    for pair in query_part.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (raw_key, raw_value) = pair.split_once('=').unwrap_or((pair, ""));
        if raw_key != "amount" {
            return Err(format!(
                "Invalid value for {field}: '{value}'. unsupported pwm URI query parameter '{raw_key}'"
            ));
        }
        if amount.is_some() {
            return Err(format!(
                "Invalid value for {field}: '{value}'. duplicate 'amount' query parameter"
            ));
        }
        amount = Some(parse_amount_value("URI amount", raw_value)?);
    }
    Ok((address, amount))
}

pub(crate) fn resolve_tx_send_amount(
    cli_amount: Option<u128>,
    uri_amount: Option<u128>,
) -> Result<u128, String> {
    match (cli_amount, uri_amount) {
        (Some(cli), Some(uri)) if cli != uri => Err(format!(
            "amount conflict: --amount={cli} differs from URI amount={uri}. Use exactly one source or the same value in both"
        )),
        (Some(cli), Some(_)) => Ok(cli),
        (Some(cli), None) => Ok(cli),
        (None, Some(uri)) => Ok(uri),
        (None, None) => Err("missing amount: provide --amount or use URI query '?amount='".to_string()),
    }
}
