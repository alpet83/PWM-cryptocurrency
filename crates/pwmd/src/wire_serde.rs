//! serde compatibility helpers for numeric fields on HTTP/wire payloads.

use serde::de::{self, Visitor};
use serde::{Deserializer, Serializer};
use std::fmt;

fn parse_u128_compat_str(raw: &str) -> Result<u128, &'static str> {
    if let Some(hex) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
        if hex.is_empty() {
            return Err("invalid u128 hex string");
        }
        return u128::from_str_radix(hex, 16).map_err(|_| "invalid u128 hex string");
    }
    raw.parse::<u128>()
        .map_err(|_| "invalid u128 decimal string")
}

fn to_hex_u128(value: u128) -> String {
    format!("0x{value:x}")
}

pub(crate) fn ser_u128_hex<S>(value: &u128, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&to_hex_u128(*value))
}

pub(crate) fn de_u128_compat<'de, D>(deserializer: D) -> Result<u128, D::Error>
where
    D: Deserializer<'de>,
{
    struct U128CompatVisitor;

    impl<'de> Visitor<'de> for U128CompatVisitor {
        type Value = u128;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("u128 as hex/decimal string or non-negative integer")
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(u128::from(value))
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            u128::try_from(value).map_err(|_| E::custom("u128 must be non-negative"))
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            parse_u128_compat_str(value).map_err(E::custom)
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            self.visit_str(&value)
        }
    }

    deserializer.deserialize_any(U128CompatVisitor)
}
