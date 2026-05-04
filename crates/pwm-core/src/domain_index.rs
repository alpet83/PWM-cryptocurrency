//! Phase 1B domain index for bech32DX formatting.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DomainCategory {
    Regulatory,
    Sector,
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

pub const COUNTRY_RANGE: core::ops::RangeInclusive<u32> = 0x0300..=0xC5FF;
pub const SECTOR_RANGE: core::ops::RangeInclusive<u32> = 0xD000..=0xDFFF;
pub const RESERVE_RANGE: core::ops::RangeInclusive<u32> = 0xE000..=0xEFFF;
pub const WITNESS_RANGE: core::ops::RangeInclusive<u32> = 0xF000..=0xFFFF;

const REGULATORY: &[DomainEntry] = &[
    DomainEntry {
        raw: 0x0300,
        label: "AD",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x0400,
        label: "AE",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x0500,
        label: "AF",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x0600,
        label: "AG",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x0700,
        label: "AL",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x0800,
        label: "AM",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x0900,
        label: "AO",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x0A00,
        label: "AR",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x0B00,
        label: "AT",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x0C00,
        label: "AU",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x0D00,
        label: "AZ",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x0E00,
        label: "BA",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x0F00,
        label: "BB",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1000,
        label: "BD",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1100,
        label: "BE",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1200,
        label: "BF",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1300,
        label: "BG",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1400,
        label: "BH",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1500,
        label: "BI",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1600,
        label: "BJ",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1700,
        label: "BN",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1800,
        label: "BO",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1900,
        label: "BR",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1A00,
        label: "BS",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1B00,
        label: "BT",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1C00,
        label: "BW",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1D00,
        label: "BY",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1E00,
        label: "BZ",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x1F00,
        label: "CA",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2000,
        label: "CD",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2100,
        label: "CF",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2200,
        label: "CG",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2300,
        label: "CH",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2400,
        label: "CI",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2500,
        label: "CL",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2600,
        label: "CM",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2700,
        label: "CN",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2800,
        label: "CO",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2900,
        label: "CR",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2A00,
        label: "CU",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2B00,
        label: "CV",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2C00,
        label: "CY",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2D00,
        label: "CZ",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2E00,
        label: "DE",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x2F00,
        label: "DJ",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x3000,
        label: "DK",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x3100,
        label: "DM",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x3200,
        label: "DO",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x3300,
        label: "DZ",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x3400,
        label: "EC",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x3500,
        label: "EE",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x3600,
        label: "EG",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x3700,
        label: "ER",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x3800,
        label: "ES",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x3900,
        label: "ET",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x3A00,
        label: "FI",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x3B00,
        label: "FJ",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x3C00,
        label: "FM",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x3D00,
        label: "FR",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x3E00,
        label: "GA",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x3F00,
        label: "GB",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x4000,
        label: "GD",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x4100,
        label: "GE",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x4200,
        label: "GH",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x4300,
        label: "GM",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x4400,
        label: "GN",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x4500,
        label: "GQ",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x4600,
        label: "GR",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x4700,
        label: "GT",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x4800,
        label: "GW",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x4900,
        label: "GY",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x4A00,
        label: "HN",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x4B00,
        label: "HR",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x4C00,
        label: "HT",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x4D00,
        label: "HU",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x4E00,
        label: "ID",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x4F00,
        label: "IE",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x5000,
        label: "IL",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x5100,
        label: "IN",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x5200,
        label: "IQ",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x5300,
        label: "IR",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x5400,
        label: "IS",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x5500,
        label: "IT",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x5600,
        label: "JM",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x5700,
        label: "JO",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x5800,
        label: "JP",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x5900,
        label: "KE",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x5A00,
        label: "KG",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x5B00,
        label: "KH",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x5C00,
        label: "KI",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x5D00,
        label: "KM",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x5E00,
        label: "KN",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x5F00,
        label: "KP",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x6000,
        label: "KR",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x6100,
        label: "KW",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x6200,
        label: "KZ",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x6300,
        label: "LA",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x6400,
        label: "LB",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x6500,
        label: "LC",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x6600,
        label: "LI",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x6700,
        label: "LK",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x6800,
        label: "LR",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x6900,
        label: "LS",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x6A00,
        label: "LT",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x6B00,
        label: "LU",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x6C00,
        label: "LV",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x6D00,
        label: "LY",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x6E00,
        label: "MA",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x6F00,
        label: "MC",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x7000,
        label: "MD",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x7100,
        label: "ME",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x7200,
        label: "MG",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x7300,
        label: "MH",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x7400,
        label: "MK",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x7500,
        label: "ML",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x7600,
        label: "MM",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x7700,
        label: "MN",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x7800,
        label: "MR",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x7900,
        label: "MT",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x7A00,
        label: "MU",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x7B00,
        label: "MV",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x7C00,
        label: "MW",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x7D00,
        label: "MX",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x7E00,
        label: "MY",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x7F00,
        label: "MZ",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x8000,
        label: "NA",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x8100,
        label: "NE",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x8200,
        label: "NG",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x8300,
        label: "NI",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x8400,
        label: "NL",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x8500,
        label: "NO",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x8600,
        label: "NP",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x8700,
        label: "NR",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x8800,
        label: "NZ",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x8900,
        label: "OM",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x8A00,
        label: "PA",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x8B00,
        label: "PE",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x8C00,
        label: "PG",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x8D00,
        label: "PH",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x8E00,
        label: "PK",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x8F00,
        label: "PL",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x9000,
        label: "PS",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x9100,
        label: "PT",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x9200,
        label: "PW",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x9300,
        label: "PY",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x9400,
        label: "QA",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x9500,
        label: "RO",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x9600,
        label: "RS",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x9700,
        label: "RU",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x9800,
        label: "RW",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x9900,
        label: "SA",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x9A00,
        label: "SB",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x9B00,
        label: "SC",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x9C00,
        label: "SD",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x9D00,
        label: "SE",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x9E00,
        label: "SG",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0x9F00,
        label: "SI",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xA000,
        label: "SK",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xA100,
        label: "SL",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xA200,
        label: "SM",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xA300,
        label: "SN",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xA400,
        label: "SO",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xA500,
        label: "SR",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xA600,
        label: "SS",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xA700,
        label: "ST",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xA800,
        label: "SV",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xA900,
        label: "SY",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xAA00,
        label: "SZ",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xAB00,
        label: "TD",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xAC00,
        label: "TG",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xAD00,
        label: "TH",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xAE00,
        label: "TJ",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xAF00,
        label: "TL",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xB000,
        label: "TM",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xB100,
        label: "TN",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xB200,
        label: "TO",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xB300,
        label: "TR",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xB400,
        label: "TT",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xB500,
        label: "TV",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xB600,
        label: "TZ",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xB700,
        label: "UA",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xB800,
        label: "UG",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xB900,
        label: "US",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xBA00,
        label: "UY",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xBB00,
        label: "UZ",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xBC00,
        label: "VA",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xBD00,
        label: "VC",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xBE00,
        label: "VE",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xBF00,
        label: "VN",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xC000,
        label: "VU",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xC100,
        label: "WS",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xC200,
        label: "YE",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xC300,
        label: "ZA",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xC400,
        label: "ZM",
        category: DomainCategory::Regulatory,
    },
    DomainEntry {
        raw: 0xC500,
        label: "ZW",
        category: DomainCategory::Regulatory,
    },
];

const SECTORS: &[DomainEntry] = &[
    DomainEntry {
        raw: 0xD003,
        label: "CDS",
        category: DomainCategory::Sector,
    },
    DomainEntry {
        raw: 0xD006,
        label: "CST",
        category: DomainCategory::Sector,
    },
    DomainEntry {
        raw: 0xD009,
        label: "ENG",
        category: DomainCategory::Sector,
    },
    DomainEntry {
        raw: 0xD00C,
        label: "FIN",
        category: DomainCategory::Sector,
    },
    DomainEntry {
        raw: 0xD00F,
        label: "HLC",
        category: DomainCategory::Sector,
    },
    DomainEntry {
        raw: 0xD012,
        label: "IND",
        category: DomainCategory::Sector,
    },
    DomainEntry {
        raw: 0xD015,
        label: "ITC",
        category: DomainCategory::Sector,
    },
    DomainEntry {
        raw: 0xD018,
        label: "MAT",
        category: DomainCategory::Sector,
    },
    DomainEntry {
        raw: 0xD01B,
        label: "REA",
        category: DomainCategory::Sector,
    },
    DomainEntry {
        raw: 0xD01E,
        label: "TLS",
        category: DomainCategory::Sector,
    },
    DomainEntry {
        raw: 0xD021,
        label: "UTL",
        category: DomainCategory::Sector,
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
    REGULATORY
        .iter()
        .chain(SECTORS.iter())
        .chain(WITNESS.iter())
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
        "argentina" => 0x0A00,
        "australia" => 0x0C00,
        "austria" => 0x0B00,
        "belgium" => 0x1100,
        "brazil" => 0x1900,
        "canada" => 0x1F00,
        "chile" => 0x2500,
        "china" => 0x2700,
        "colombia" => 0x2800,
        "cyprus" => 0x2C00,
        "czech republic" => 0x2D00,
        "denmark" => 0x3000,
        "egypt" => 0x3600,
        "estonia" => 0x3500,
        "finland" => 0x3A00,
        "france" => 0x3D00,
        "germany" => 0x2E00,
        "india" => 0x5100,
        "indonesia" => 0x4E00,
        "ireland" => 0x4F00,
        "israel" => 0x5000,
        "italy" => 0x5500,
        "japan" => 0x5800,
        "mexico" => 0x7D00,
        "netherlands" => 0x8400,
        "new zealand" => 0x8800,
        "nigeria" => 0x8200,
        "norway" => 0x8500,
        "poland" => 0x8F00,
        "portugal" => 0x9100,
        "saudi arabia" => 0x9900,
        "singapore" => 0x9E00,
        "south africa" => 0xC300,
        "south korea" => 0x6000,
        "spain" => 0x3800,
        "sweden" => 0x9D00,
        "switzerland" => 0x2300,
        "turkey" => 0xB300,
        "ukraine" => 0xB700,
        "united arab emirates" => 0x0400,
        "united kingdom" => 0x3F00,
        "united states" => 0xB900,
        "vietnam" => 0xBF00,
        _ => return None,
    };
    Some(raw)
}

pub fn category_for_raw(raw: u32) -> Option<DomainCategory> {
    if COUNTRY_RANGE.contains(&raw) {
        Some(DomainCategory::Regulatory)
    } else if SECTOR_RANGE.contains(&raw) {
        Some(DomainCategory::Sector)
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
        lookup_regulatory_by_hi, DomainCategory, REGULATORY,
    };

    /// Canonical regulatory table size + stable sort order (formerly `country_list_has_195_entries_sorted`).
    #[test]
    fn reg195_country_table_sorted() {
        assert_eq!(REGULATORY.len(), 195);
        let mut labels: Vec<&str> = REGULATORY.iter().map(|e| e.label).collect();
        let mut sorted = labels.clone();
        sorted.sort_unstable();
        assert_eq!(labels, sorted);
        labels.dedup();
        assert_eq!(labels.len(), 195);
    }

    #[test]
    fn sample_raw_lookup_works() {
        let us = lookup_by_raw(0xB900).expect("US");
        assert_eq!(us.label, "US");
        assert_eq!(us.category, DomainCategory::Regulatory);
    }

    #[test]
    fn sample_sector_lookup_works() {
        let fin = lookup_by_label("FIN").expect("FIN");
        assert_eq!(fin.raw, 0xD00C);
        assert_eq!(fin.category, DomainCategory::Sector);
    }

    /// Historical label aliases resolve to canonical rows (formerly `sample_legacy_label_alias_lookup_works`).
    #[test]
    fn lookup_legacy_alias_hit() {
        let us = lookup_by_label("united states").expect("legacy alias");
        assert_eq!(us.raw, 0xB900);
        assert_eq!(us.label, "US");
    }

    /// Reserve range has category routing without verbatim label lookup (formerly `reserve_range_has_category_without_direct_label`).
    #[test]
    fn reserve_raw_cat_only() {
        assert_eq!(lookup_by_raw(0xE003), None);
        assert_eq!(category_for_raw(0xE003), Some(DomainCategory::Reserve));
    }

    /// CY regulatory hi lookup collapses low-byte variants (formerly `regulatory_lookup_by_hi_ignores_low_byte_noise`).
    #[test]
    fn reg_hi_ignore_lo_noise() {
        let cy = lookup_regulatory_by_hi(0x2C).expect("CY by high byte");
        assert_eq!(cy.label, "CY");
        let from_display = lookup_for_display(0x2C7F).expect("CY by display lookup");
        assert_eq!(from_display.label, "CY");
    }
}
