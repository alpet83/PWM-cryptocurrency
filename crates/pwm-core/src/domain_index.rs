//! Phase 1B domain index for bech32DX formatting.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DomainCategory {
    Regulatory,
    Tnc,
    Reserve,
    Witness,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DomainEntry {
    pub raw: u32,
    pub label: &'static str,
    pub category: DomainCategory,
}

pub const STEP: u32 = 3;
pub const DOMAIN_MAX_20BIT: u32 = 0x0F_FFFF;

pub const COUNTRY_RANGE: core::ops::RangeInclusive<u32> = 0x0000..=0xBFFF;
pub const TNC_RANGE: core::ops::RangeInclusive<u32> = 0xC000..=0xDFFF;
pub const RESERVE_RANGE: core::ops::RangeInclusive<u32> = 0xE000..=0xEFFF;
pub const WITNESS_RANGE: core::ops::RangeInclusive<u32> = 0xF000..=0xFFFF;

const REGULATORY: &[DomainEntry] = &[
    DomainEntry {
        raw: 0x0100,
        label: "AR",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x0200,
        label: "AU",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x0300,
        label: "AT",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x0400,
        label: "BE",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x0500,
        label: "BR",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x0600,
        label: "CA",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x0700,
        label: "CL",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x0800,
        label: "CN",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x0900,
        label: "CO",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x0A00,
        label: "CZ",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x0B00,
        label: "DK",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x0C00,
        label: "EG",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x0D00,
        label: "EE",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x0E00,
        label: "FI",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x0F00,
        label: "FR",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1000,
        label: "DE",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1100,
        label: "HK",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1200,
        label: "IN",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1300,
        label: "ID",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1400,
        label: "IE",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1500,
        label: "IL",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1600,
        label: "IT",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1700,
        label: "JP",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1800,
        label: "MX",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1900,
        label: "NL",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1A00,
        label: "NZ",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1B00,
        label: "NG",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1C00,
        label: "NO",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1D00,
        label: "PL",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1E00,
        label: "PT",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1F00,
        label: "SA",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2000,
        label: "SG",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2100,
        label: "ZA",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2200,
        label: "KR",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2300,
        label: "ES",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2400,
        label: "SE",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2500,
        label: "CH",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2600,
        label: "TR",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2700,
        label: "UA",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2800,
        label: "AE",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2900,
        label: "GB",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2A00,
        label: "US",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2B00,
        label: "VN",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2C00,
        label: "CY",
        category: DomainCategory::Regulatory,
    },
];

const TNC: &[DomainEntry] = &[
    DomainEntry {
        raw: 0xC003,
        label: "BABA",
        category: DomainCategory::Tnc,
    },
    DomainEntry {
        raw: 0xC006,
        label: "GOOGL",
        category: DomainCategory::Tnc,
    },
    DomainEntry {
        raw: 0xC009,
        label: "AMZN",
        category: DomainCategory::Tnc,
    },
    DomainEntry {
        raw: 0xC00C,
        label: "AAPL",
        category: DomainCategory::Tnc,
    },
    DomainEntry {
        raw: 0xC00F,
        label: "BRK",
        category: DomainCategory::Tnc,
    },
    DomainEntry {
        raw: 0xC012,
        label: "BYD",
        category: DomainCategory::Tnc,
    },
    DomainEntry {
        raw: 0xC015,
        label: "XOM",
        category: DomainCategory::Tnc,
    },
    DomainEntry {
        raw: 0xC018,
        label: "JPM",
        category: DomainCategory::Tnc,
    },
    DomainEntry {
        raw: 0xC01B,
        label: "META",
        category: DomainCategory::Tnc,
    },
    DomainEntry {
        raw: 0xC01E,
        label: "MSFT",
        category: DomainCategory::Tnc,
    },
    DomainEntry {
        raw: 0xC021,
        label: "NESN",
        category: DomainCategory::Tnc,
    },
    DomainEntry {
        raw: 0xC024,
        label: "NVDA",
        category: DomainCategory::Tnc,
    },
    DomainEntry {
        raw: 0xC027,
        label: "PTR",
        category: DomainCategory::Tnc,
    },
    DomainEntry {
        raw: 0xC02A,
        label: "PG",
        category: DomainCategory::Tnc,
    },
    DomainEntry {
        raw: 0xC02D,
        label: "SMSN",
        category: DomainCategory::Tnc,
    },
    DomainEntry {
        raw: 0xC030,
        label: "SHEL",
        category: DomainCategory::Tnc,
    },
    DomainEntry {
        raw: 0xC033,
        label: "SONY",
        category: DomainCategory::Tnc,
    },
    DomainEntry {
        raw: 0xC036,
        label: "TCEHY",
        category: DomainCategory::Tnc,
    },
    DomainEntry {
        raw: 0xC039,
        label: "TSLA",
        category: DomainCategory::Tnc,
    },
    DomainEntry {
        raw: 0xC03C,
        label: "TM",
        category: DomainCategory::Tnc,
    },
];

const WITNESS: &[DomainEntry] = &[
    DomainEntry {
        raw: 0xF003,
        label: "Witness Fast",
        category: DomainCategory::Witness,
    },
    DomainEntry {
        raw: 0xF006,
        label: "Witness Mobile",
        category: DomainCategory::Witness,
    },
    DomainEntry {
        raw: 0xF009,
        label: "Witness Secure",
        category: DomainCategory::Witness,
    },
];

pub fn all_entries() -> impl Iterator<Item = &'static DomainEntry> {
    REGULATORY.iter().chain(TNC.iter()).chain(WITNESS.iter())
}

pub fn lookup_by_raw(raw: u32) -> Option<&'static DomainEntry> {
    all_entries().find(|entry| entry.raw == raw)
}

pub fn lookup_regulatory_by_hi(domain_hi: u8) -> Option<&'static DomainEntry> {
    REGULATORY
        .iter()
        .find(|entry| ((entry.raw >> 8) as u8) == domain_hi)
}

pub fn lookup_for_display(raw: u32) -> Option<&'static DomainEntry> {
    lookup_by_raw(raw).or_else(|| match category_for_raw(raw) {
        Some(DomainCategory::Regulatory) => lookup_regulatory_by_hi((raw >> 8) as u8),
        _ => None,
    })
}

pub fn lookup_by_label(label: &str) -> Option<&'static DomainEntry> {
    let normalized = label.trim();
    all_entries()
        .find(|entry| entry.label.eq_ignore_ascii_case(normalized))
        .or_else(|| lookup_legacy_label_raw(normalized).and_then(lookup_by_raw))
}

fn lookup_legacy_label_raw(label: &str) -> Option<u32> {
    let normalized = label.trim().to_ascii_lowercase();
    let raw = match normalized.as_str() {
        "argentina" => 0x0100,
        "australia" => 0x0200,
        "austria" => 0x0300,
        "belgium" => 0x0400,
        "brazil" => 0x0500,
        "canada" => 0x0600,
        "chile" => 0x0700,
        "china" => 0x0800,
        "colombia" => 0x0900,
        "czech republic" => 0x0A00,
        "denmark" => 0x0B00,
        "egypt" => 0x0C00,
        "estonia" => 0x0D00,
        "finland" => 0x0E00,
        "france" => 0x0F00,
        "germany" => 0x1000,
        "hong kong" => 0x1100,
        "india" => 0x1200,
        "indonesia" => 0x1300,
        "ireland" => 0x1400,
        "israel" => 0x1500,
        "italy" => 0x1600,
        "japan" => 0x1700,
        "mexico" => 0x1800,
        "netherlands" => 0x1900,
        "new zealand" => 0x1A00,
        "nigeria" => 0x1B00,
        "norway" => 0x1C00,
        "poland" => 0x1D00,
        "portugal" => 0x1E00,
        "saudi arabia" => 0x1F00,
        "singapore" => 0x2000,
        "south africa" => 0x2100,
        "south korea" => 0x2200,
        "spain" => 0x2300,
        "sweden" => 0x2400,
        "switzerland" => 0x2500,
        "turkey" => 0x2600,
        "ukraine" => 0x2700,
        "united arab emirates" => 0x2800,
        "united kingdom" => 0x2900,
        "united states" => 0x2A00,
        "vietnam" => 0x2B00,
        "cyprus" => 0x2C00,
        "alibaba group" => 0xC003,
        "alphabet" => 0xC006,
        "amazon" => 0xC009,
        "apple" => 0xC00C,
        "berkshire hathaway" => 0xC00F,
        "exxonmobil" => 0xC015,
        "jpmorgan chase" => 0xC018,
        "meta platforms" => 0xC01B,
        "microsoft" => 0xC01E,
        "nestle" => 0xC021,
        "nvidia" => 0xC024,
        "petrochina" => 0xC027,
        "procter & gamble" => 0xC02A,
        "samsung electronics" => 0xC02D,
        "shell" => 0xC030,
        "sony group" => 0xC033,
        "tesla" => 0xC039,
        "tencent" => 0xC036,
        "toyota motor" => 0xC03C,
        _ => return None,
    };
    Some(raw)
}

pub fn category_for_raw(raw: u32) -> Option<DomainCategory> {
    if COUNTRY_RANGE.contains(&raw) {
        Some(DomainCategory::Regulatory)
    } else if TNC_RANGE.contains(&raw) {
        Some(DomainCategory::Tnc)
    } else if RESERVE_RANGE.contains(&raw) {
        Some(DomainCategory::Reserve)
    } else if WITNESS_RANGE.contains(&raw) {
        Some(DomainCategory::Witness)
    } else {
        None
    }
}

pub fn is_structurally_valid(raw: u32) -> bool {
    raw <= DOMAIN_MAX_20BIT && category_for_raw(raw).is_some()
}

#[cfg(test)]
mod tests {
    use super::{
        category_for_raw, lookup_by_label, lookup_by_raw, lookup_for_display,
        lookup_regulatory_by_hi, DomainCategory, STEP,
    };

    #[test]
    fn sample_raw_lookup_works() {
        let us = lookup_by_raw(0x2A00).expect("US");
        assert_eq!(us.label, "US");
        assert_eq!(us.category, DomainCategory::Regulatory);
    }

    #[test]
    fn sample_label_lookup_works() {
        let msft = lookup_by_label("MSFT").expect("MSFT");
        assert_eq!(msft.raw, 0xC01E);
        assert_eq!(msft.category, DomainCategory::Tnc);
    }

    #[test]
    fn sample_legacy_label_alias_lookup_works() {
        let msft = lookup_by_label("microsoft").expect("legacy alias");
        assert_eq!(msft.raw, 0xC01E);
        assert_eq!(msft.label, "MSFT");
    }

    #[test]
    fn legacy_tnc_aliases_resolve_to_short_labels() {
        let nvidia = lookup_by_label("nvidia").expect("legacy alias nvidia");
        assert_eq!(nvidia.raw, 0xC024);
        assert_eq!(nvidia.label, "NVDA");

        let tesla = lookup_by_label("tesla").expect("legacy alias tesla");
        assert_eq!(tesla.raw, 0xC039);
        assert_eq!(tesla.label, "TSLA");
    }

    #[test]
    fn sparse_assignments_have_step_more_than_one() {
        assert!(STEP > 1);
        let first = lookup_by_raw(0x0100).unwrap();
        let second = lookup_by_raw(0x0200).unwrap();
        assert_eq!(second.raw - first.raw, 0x0100);
    }

    #[test]
    fn reserve_range_has_category_without_direct_label() {
        assert_eq!(lookup_by_raw(0xE003), None);
        assert_eq!(category_for_raw(0xE003), Some(DomainCategory::Reserve));
    }

    #[test]
    fn regulatory_lookup_by_hi_ignores_low_byte_noise() {
        let cy = lookup_regulatory_by_hi(0x2C).expect("CY by high byte");
        assert_eq!(cy.label, "CY");
        let from_display = lookup_for_display(0x2C7F).expect("CY by display lookup");
        assert_eq!(from_display.label, "CY");
    }
}
