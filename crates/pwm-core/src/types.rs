//! Human-facing account ID codecs (BECH32DX/legacy) and display helpers.

use bech32::{self, FromBase32, ToBase32, Variant};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

use crate::domain_index;
use crate::tx::PolicyKind;
use crate::MARKS_CAP;

/// 32-byte account identifier (see `WHITE_SPEC_v0`).
pub type AccountId = [u8; 32];
pub const LEGACY_HUMAN_ACCOUNT_PREFIX: &str = "PWMv0-";
pub const BECH32DX_HRP: &str = "pwm";
pub const COSIGN_NON_DISABLEABLE: u32 = 1;
pub const CONSERVATION: u32 = 1 << 1;
const BECH32DX_VERSION: u8 = 1;
const BECH32DX_PAYLOAD_LEN: usize = 38;

pub fn account_id_to_human(id: &AccountId) -> String {
    render_acct_id_ui(id)
}

pub fn address_flags(id: &AccountId) -> u32 {
    u32::from_be_bytes([id[2], id[3], id[4], id[5]])
}

pub fn cosign_non_dis(id: &AccountId) -> bool {
    address_flags(id) & COSIGN_NON_DISABLEABLE != 0
}

pub fn conservation_flag(id: &AccountId) -> bool {
    address_flags(id) & CONSERVATION != 0
}

pub fn parse_account_id(input: &str) -> Result<AccountId, String> {
    let s = input.trim();
    if let Ok(id) = parse_bech32dx_account_id(s) {
        return Ok(id);
    }
    let pretty_parse = parse_pretty_account_id(s);
    if let Ok(id) = pretty_parse {
        return Ok(id);
    }
    // Avoid misleading legacy hex errors (like "Odd number of digits") for malformed pretty input.
    if s.starts_with("pwm1-") {
        return Err(pretty_parse.unwrap_err());
    }
    let legacy_bech32_part = s.split_once('$').map(|(head, _)| head).unwrap_or(s);
    if let Ok(id) = parse_bech32dx_account_id(legacy_bech32_part) {
        return Ok(id);
    }
    let hex_part = if let Some(rest) = legacy_bech32_part.strip_prefix(LEGACY_HUMAN_ACCOUNT_PREFIX)
    {
        rest
    } else {
        legacy_bech32_part
    };
    let bytes = hex::decode(hex_part).map_err(|e| e.to_string())?;
    if bytes.len() != 32 {
        return Err("need 32 bytes hex account id".into());
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

/// Parse account id in runtime user-input paths (CLI/TUI flags, prompts, commands).
///
/// Rejects ambiguous legacy pretty addresses where a regulatory label is used
/// without explicit `/LO` suffix (for example `pwm1-CY-f...`).
///
/// Previously: `parse_account_id_for_user_input`.
/// Parses an account id from user-supplied input (pretty or canonical hex).
pub fn parse_acct_id_ui(input: &str) -> Result<AccountId, String> {
    let s = input.trim();
    if let Some(domain_part) = pretty_domain_part(s) {
        reject_ambig_dom(domain_part, true)?;
    }
    parse_account_id(s)
}

/// Parse account id for wallet/address-book migration flows.
///
/// Unlike [`parse_account_id`], this rejects ambiguous legacy pretty addresses where
/// a regulatory label is used without an explicit `/LO` suffix (for example `pwm1-CY-f...`).
/// Parses an account id in migration context (accepts legacy formats).
pub fn parse_acct_id_mig(input: &str) -> Result<AccountId, String> {
    let s = input.trim();
    if let Some(domain_part) = pretty_domain_part(s) {
        reject_ambig_dom(domain_part, false)?;
    }
    parse_account_id(s)
}

fn pretty_domain_part(s: &str) -> Option<&str> {
    let rest = s.strip_prefix("pwm1-")?;
    let (domain_part, _) = rest.split_once("-f")?;
    Some(domain_part)
}

/// Rejects account ids that are ambiguous under the legacy pretty-domain encoding.
fn reject_ambig_dom(domain_part: &str, runtime_user_input: bool) -> Result<(), String> {
    if domain_part.contains('/') {
        return Ok(());
    }
    let Some(entry) = domain_index::lookup_by_label(domain_part) else {
        return Ok(());
    };
    if matches!(
        domain_index::category_for_raw(entry.raw),
        Some(domain_index::DomainCategory::Regulatory)
    ) {
        if runtime_user_input {
            return Err(format!(
                "ambiguous legacy pretty domain '{domain_part}': missing '/LO' suffix; use strict pretty 'pwm1-LABEL/XX-f...-t...' or canonical bech32dx 'pwm1...'"
            ));
        }
        return Err(format!(
            "ambiguous legacy pretty domain '{domain_part}': missing '/LO' suffix; use canonical/bech32dx, account_id_hex, or strict pretty LABEL/LO"
        ));
    }
    Ok(())
}

pub fn account_id_to_bech32dx(id: &AccountId) -> String {
    encode_bech32dx(BECH32DX_VERSION, 0, dom_raw_from_acct(id), id)
}

pub fn encode_bech32dx(version: u8, flags: u8, domain_raw: u32, id: &AccountId) -> String {
    let mut payload = [0u8; BECH32DX_PAYLOAD_LEN];
    payload[0] = version;
    payload[1] = flags;
    payload[2..6].copy_from_slice(&domain_raw.to_be_bytes());
    payload[6..].copy_from_slice(id);
    bech32::encode(BECH32DX_HRP, payload.to_base32(), Variant::Bech32m).expect("bech32dx encode")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bech32DxDecoded {
    pub version: u8,
    pub flags: u8,
    pub domain_raw: u32,
    pub account_id: AccountId,
}

pub fn decode_bech32dx(s: &str) -> Result<Bech32DxDecoded, String> {
    let (hrp, data, variant) = bech32::decode(s).map_err(|e| e.to_string())?;
    if hrp != BECH32DX_HRP {
        return Err("wrong bech32dx hrp".into());
    }
    if variant != Variant::Bech32m {
        return Err("wrong bech32dx variant".into());
    }
    let payload = Vec::<u8>::from_base32(&data).map_err(|e| e.to_string())?;
    if payload.len() != BECH32DX_PAYLOAD_LEN {
        return Err("wrong bech32dx payload length".into());
    }
    if payload[0] != BECH32DX_VERSION {
        return Err("unsupported bech32dx version".into());
    }
    let domain_raw = u32::from_be_bytes([payload[2], payload[3], payload[4], payload[5]]);
    let mut account_id = [0u8; 32];
    account_id.copy_from_slice(&payload[6..]);
    Ok(Bech32DxDecoded {
        version: payload[0],
        flags: payload[1],
        domain_raw,
        account_id,
    })
}

fn parse_bech32dx_account_id(s: &str) -> Result<AccountId, String> {
    Ok(decode_bech32dx(s)?.account_id)
}

fn parse_pretty_account_id(s: &str) -> Result<AccountId, String> {
    let rest = s
        .strip_prefix("pwm1-")
        .ok_or_else(|| "missing pretty prefix".to_string())?;
    let (domain_part, after_domain) = rest
        .split_once("-f")
        .ok_or_else(|| "missing pretty flags separator".to_string())?;
    let (flags_hex, tail_part) = after_domain
        .split_once("-t")
        .ok_or_else(|| "missing pretty tail separator".to_string())?;
    if flags_hex.len() != 8 || !flags_hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("invalid pretty flags".into());
    }
    if tail_part.len() != 52 || !tail_part.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("invalid pretty tail".into());
    }
    let domain_raw = parse_pretty_domain(domain_part)?;
    if domain_raw > u16::MAX as u32 {
        return Err("pretty domain does not fit u16".into());
    }
    let flags = u32::from_str_radix(flags_hex, 16).map_err(|e| e.to_string())?;
    let mut id = [0u8; 32];
    let dom_bytes = (domain_raw as u16).to_be_bytes();
    id[0] = dom_bytes[0];
    id[1] = dom_bytes[1];
    id[2..6].copy_from_slice(&flags.to_be_bytes());
    let tail = hex::decode(tail_part).map_err(|e| e.to_string())?;
    id[6..].copy_from_slice(&tail);
    Ok(id)
}

fn parse_pretty_domain(domain_part: &str) -> Result<u32, String> {
    if let Some((label, lo_hex)) = domain_part.split_once('/') {
        if lo_hex.len() != 2 || !lo_hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err("invalid pretty domain lo-byte".into());
        }
        let entry = domain_index::lookup_by_label(label)
            .ok_or_else(|| "invalid pretty domain label".to_string())?;
        let lo = u8::from_str_radix(lo_hex, 16).map_err(|e| e.to_string())?;
        let raw = (entry.raw & 0xFF00) | (lo as u32);
        return Ok(raw);
    }
    if let Some(entry) = domain_index::lookup_by_label(domain_part) {
        return Ok(entry.raw);
    }
    let hex = domain_part
        .strip_suffix('!')
        .unwrap_or(domain_part)
        .strip_prefix('$')
        .ok_or_else(|| "invalid pretty domain".to_string())?;
    if !(hex.len() == 4 || hex.len() == 5) || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("invalid pretty domain".into());
    }
    u32::from_str_radix(hex, 16).map_err(|e| e.to_string())
}

/// Extracts the raw domain-hi byte from an account id.
fn dom_raw_from_acct(id: &AccountId) -> u32 {
    ((id[0] as u32) << 8) | (id[1] as u32)
}

pub fn format_domain_pascal_hex(domain_raw: u32) -> String {
    fmt_dom_pascal_hex(domain_raw, if domain_raw <= 0xFFFF { 4 } else { 5 })
}

/// Formats a domain-hi value as a fixed-width PascalCase hex string.
pub fn fmt_dom_pascal_hex(domain_raw: u32, width: usize) -> String {
    format!("${:0width$X}", domain_raw, width = width)
}

pub fn format_domain_for_display(domain_raw: u32) -> (String, bool) {
    if let Some(entry) = domain_index::lookup_for_display(domain_raw) {
        return (entry.label.to_string(), true);
    }
    (format_domain_pascal_hex(domain_raw), false)
}

/// Renders an account id in user-readable form (pretty or canonical depending on domain).
pub fn render_acct_id_ui(id: &AccountId) -> String {
    let domain_raw = dom_raw_from_acct(id);
    let (domain_display, known_for_display) = format_domain_for_display(domain_raw);
    let known_domain_with_lo = if known_for_display
        && matches!(
            domain_index::category_for_raw(domain_raw),
            Some(domain_index::DomainCategory::Regulatory)
        ) {
        format!("{}/{:02X}", domain_display, id[1])
    } else {
        domain_display
    };
    let flags = address_flags(id);
    let tail = hex::encode(&id[6..]);
    let domain_hint = if known_for_display {
        known_domain_with_lo
    } else {
        format!("{known_domain_with_lo}!")
    };
    format!("pwm1-{domain_hint}-f{flags:08X}-t{tail}")
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DeferredPolicyEntry {
    pub policy: PolicyKind,
    pub activate_at_height: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Account {
    pub signing_pubkey: [u8; 32],
    pub derivation_index: u32,
    pub balance_pwm: u128,
    #[serde(default, alias = "staked")]
    pub staked_pwm_raw: u128,
    #[serde(default, alias = "marks", deserialize_with = "de_marks_compat")]
    pub stored_marks: u32,
    #[serde(default)]
    pub marks_last_block: u64,
    pub initialized: bool,
    pub index: u32,
    pub flags: u32,
    pub nonce: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rescue_address: Option<AccountId>,
    #[serde(default, skip_serializing_if = "is_zero_u16")]
    pub active_policies: u16,
    #[serde(default, skip_serializing_if = "is_zero_u16")]
    pub dormant_policies: u16,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deferred_policies: Vec<DeferredPolicyEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ipv4_claimed_phase: Option<u8>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub finalized: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub owner_kind: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub owner_display_name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub owner_country_hint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub company_metadata_commitment: Option<[u8; 32]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_verification_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_domain_lo: Option<u8>,
}

fn is_zero_u16(v: &u16) -> bool {
    *v == 0
}

fn is_false(v: &bool) -> bool {
    !*v
}

fn migrate_marks_legacy(raw: u128) -> u32 {
    if raw <= MARKS_CAP as u128 {
        return raw as u32;
    }
    let scaled = raw / crate::display::PWM_RAW_SCALE;
    scaled.min(MARKS_CAP as u128) as u32
}

fn de_marks_compat<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum MarksWire {
        U32(u32),
        U64(u64),
        U128(u128),
        Str(String),
    }

    let wire = MarksWire::deserialize(deserializer)?;
    let raw = match wire {
        MarksWire::U32(v) => v as u128,
        MarksWire::U64(v) => v as u128,
        MarksWire::U128(v) => v,
        MarksWire::Str(s) => s
            .parse::<u128>()
            .map_err(|e| D::Error::custom(format!("invalid marks decimal string: {e}")))?,
    };
    Ok(migrate_marks_legacy(raw))
}

impl Account {
    pub fn genesis_funded(pubkey: [u8; 32], derivation_index: u32, balance: u128) -> Self {
        Self {
            signing_pubkey: pubkey,
            derivation_index,
            balance_pwm: balance,
            staked_pwm_raw: 0,
            stored_marks: 0,
            marks_last_block: 0,
            initialized: true,
            index: 0,
            flags: 0,
            nonce: 0,
            rescue_address: None,
            active_policies: 0,
            dormant_policies: 0,
            deferred_policies: Vec::new(),
            ipv4_claimed_phase: None,
            finalized: false,
            owner_kind: String::new(),
            owner_display_name: String::new(),
            owner_country_hint: String::new(),
            company_metadata_commitment: None,
            external_verification_ref: None,
            requested_domain_lo: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        account_id_to_bech32dx, account_id_to_human, decode_bech32dx, fmt_dom_pascal_hex,
        format_domain_for_display, format_domain_pascal_hex, parse_account_id, parse_acct_id_mig,
        parse_acct_id_ui, BECH32DX_HRP, LEGACY_HUMAN_ACCOUNT_PREFIX,
    };

    /// Parses raw hex plus legacy `pwm1` prefixed hex (formerly `parse_accepts_hex_and_human_prefix`).
    #[test]
    fn parse_hex_or_pwm1_hex() {
        let id = [7u8; 32];
        let hex = hex::encode(id);
        assert_eq!(parse_account_id(&hex).unwrap(), id);
        assert_eq!(
            parse_account_id(&format!("{LEGACY_HUMAN_ACCOUNT_PREFIX}{hex}")).unwrap(),
            id
        );
    }

    #[test]
    fn human_roundtrip() {
        let mut id = [9u8; 32];
        id[0] = 0xBF;
        id[1] = 0x10;
        let human = account_id_to_human(&id);
        assert_eq!(parse_account_id(&human).unwrap(), id);
    }

    /// Pretty account string embeds `-f`/`-t` hints without piping canonical form (formerly `pretty_render_has_hints_and_no_canonical_embedding`).
    #[test]
    fn pretty_hints_no_pipe_emb() {
        let mut id = [9u8; 32];
        id[0] = 0x2C;
        id[1] = 0x7E;
        let human = account_id_to_human(&id);
        assert!(human.contains("CY/7E-f"));
        assert!(human.contains("-f"));
        assert!(human.contains("-t"));
        assert!(!human.contains('|'));
        assert!(human.starts_with("pwm1-"));
        let parsed = parse_account_id(&human).unwrap();
        assert_eq!(parsed[0], 0x2C);
        assert_eq!(parsed[1], 0x7E);
    }

    /// Full tail bytes rendered after `-t` (formerly `pretty_render_includes_full_tail_bytes`).
    #[test]
    fn pretty_tail_hex_full() {
        let mut id = [0u8; 32];
        for (i, b) in id.iter_mut().enumerate() {
            *b = i as u8;
        }
        let human = account_id_to_human(&id);
        let expected_tail = hex::encode(&id[6..]);
        assert!(human.ends_with(&format!("-t{expected_tail}")));
    }

    #[test]
    fn bech32dx_roundtrip() {
        let id = [11u8; 32];
        let addr = account_id_to_bech32dx(&id);
        assert!(addr.starts_with(BECH32DX_HRP));
        assert_eq!(parse_account_id(&addr).unwrap(), id);
    }

    /// `decode_bech32dx` recovers domain/flags/metadata (formerly `decode_bech32dx_reads_domain_and_flags`).
    #[test]
    fn decode_bdx_domain_flags_id() {
        let mut id = [0u8; 32];
        id[0] = 0x00;
        id[1] = 0x7E;
        let addr = super::encode_bech32dx(1, 0x05, 0x0000_007E, &id);
        let decoded = decode_bech32dx(&addr).unwrap();
        assert_eq!(decoded.version, 1);
        assert_eq!(decoded.flags, 0x05);
        assert_eq!(decoded.domain_raw, 0x0000_007E);
        assert_eq!(decoded.account_id, id);
    }

    #[test]
    fn domain_hit_uses_label() {
        let (display, ok) = format_domain_for_display(0x2A00);
        assert!(ok);
        assert_eq!(display, "CU");
    }

    /// Unknown domain uses Pascal-width hex fallback (formerly `domain_miss_uses_pascal_hex_fallback`).
    #[test]
    fn domain_miss_pascal_hex_fb() {
        // Below COUNTRY_RANGE.lo — category_for_raw is None; no regulatory hi fallback.
        let (display, ok) = format_domain_for_display(0x0200);
        assert!(!ok);
        assert_eq!(display, "$0200");
    }

    /// Human render appends `$####!` fallback for unknown domains (formerly `human_render_appends_fallback_for_unknown_domain`).
    #[test]
    fn human_fb_unknown_dom() {
        let mut id = [0u8; 32];
        id[0] = 0x02;
        id[1] = 0x00;
        let human = account_id_to_human(&id);
        assert!(human.contains("$0200!-f"));
        assert_eq!(parse_account_id(&human).unwrap(), id);
    }

    /// Regulatory high-byte lookup keeps fractional low byte labeling (formerly `regulatory_hi_hit_does_not_fallback_on_random_low_byte`).
    #[test]
    fn cylabel_stable_lo_human() {
        let mut id = [0u8; 32];
        id[0] = 0x2C;
        id[1] = 0x7F;
        let (display, ok) = format_domain_for_display(0x2C7F);
        assert!(ok);
        assert_eq!(display, "CY");
        let human = account_id_to_human(&id);
        assert!(human.contains("CY/7F-f"));
    }

    /// Permissive parser accepts deprecated pretty without `/LO` (formerly `parse_accepts_legacy_pretty_domain_without_lo_suffix`).
    #[test]
    fn parse_legacy_plain_no_slolo() {
        let mut id = [0u8; 32];
        id[0] = 0x2C;
        id[1] = 0x00;
        id[2] = 0xBB;
        id[3] = 0x92;
        id[4] = 0x18;
        id[5] = 0x00;
        id[6] = 0x25;
        id[7] = 0xCB;
        let tail = hex::encode(&id[6..]);
        let legacy = format!("pwm1-CY-fBB921800-t{tail}");
        assert_eq!(parse_account_id(&legacy).unwrap(), id);
    }

    /// Migration rejects regulatory legacy pretty missing `/LO` (formerly `parse_for_migration_rejects_legacy_pretty_without_lo_for_regulatory`).
    #[test]
    fn migr_rejects_miss_lo() {
        let legacy = "pwm1-CY-fBB921800-t25cb00000000000000000000000000000000000000000000000000";
        let err = parse_acct_id_mig(legacy).expect_err("must reject");
        assert!(err.contains("missing '/LO'"));
    }

    /// User-input parser rejects ambiguous legacy pretty without `/LO` (formerly `parse_for_user_input_rejects_ambiguous_legacy_pretty_without_lo`).
    #[test]
    fn user_in_reject_ambig_legacy() {
        let legacy = "pwm1-CY-fBB921800-t25cb00000000000000000000000000000000000000000000000000";
        let err = parse_acct_id_ui(legacy).expect_err("must reject");
        assert!(err.contains("missing '/LO'"));
        assert!(err.contains("strict pretty"));
        assert!(err.contains("canonical bech32dx"));
    }

    /// User-input path still allows raw hex when policy permits (formerly `parse_for_user_input_still_accepts_hex_legacy_when_policy_allows`).
    #[test]
    fn user_hex_ok_via_policy() {
        let id = [0xAA; 32];
        let hex = hex::encode(id);
        assert_eq!(parse_acct_id_ui(&hex).unwrap(), id);
    }

    /// Pretty `CY/LO` suffix round-trips (formerly `parse_accepts_pretty_label_with_lo_suffix`).
    #[test]
    fn parse_pretty_label_with_lo() {
        let mut id = [0u8; 32];
        id[0] = 0x2C;
        id[1] = 0x4B;
        id[2] = 0xBB;
        id[3] = 0x92;
        id[4] = 0x18;
        id[5] = 0x00;
        id[6] = 0x25;
        id[7] = 0xCB;
        let pretty = account_id_to_human(&id);
        assert!(pretty.starts_with("pwm1-CY/4B-fBB921800-t25cb"));
        assert_eq!(parse_account_id(&pretty).unwrap(), id);
    }

    /// Unknown-domain pretty without canonical column still parses (formerly `parse_accepts_pretty_without_canonical_part`).
    #[test]
    fn parse_pretty_no_canon_col() {
        let id = [3u8; 32];
        let pretty = "pwm1-$0303!-f03030303-t0303030303030303030303030303030303030303030303030303";
        assert_eq!(parse_account_id(&pretty).unwrap(), id);
    }

    #[test]
    fn parse_accepts_canonical_bech32dx_input() {
        let id = [3u8; 32];
        let canonical = account_id_to_bech32dx(&id);
        assert_eq!(parse_account_id(&canonical).unwrap(), id);
    }

    /// Bad bech32dx checksum is rejected (formerly `parse_rejects_canonical_bech32dx_with_bad_checksum`).
    #[test]
    fn parse_bdx_bad_checksum() {
        let canonical = account_id_to_bech32dx(&[4u8; 32]);
        let mut bad = canonical.clone();
        let last = bad.pop().expect("non-empty canonical");
        let replacement = if last == 'q' { 'p' } else { 'q' };
        bad.push(replacement);
        let err = parse_account_id(&bad).expect_err("must reject bad checksum");
        assert!(!err.is_empty());
    }

    /// Pascal hex width helper widens sparse domain codes (formerly `pascal_hex_uses_20bit_width_when_needed`).
    #[test]
    fn pascal_hex_width_sparse() {
        assert_eq!(fmt_dom_pascal_hex(0x0A3F2, 5), "$0A3F2");
        assert_eq!(format_domain_pascal_hex(0x00AF), "$00AF");
    }
}
