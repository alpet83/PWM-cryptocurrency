//! serde compatibility helpers for numeric fields on HTTP/wire payloads.

use serde::de::{self, Visitor};
use serde::Deserializer;
use std::fmt;

pub(crate) fn de_u128_compat<'de, D>(deserializer: D) -> Result<u128, D::Error>
where
    D: Deserializer<'de>,
{
    struct U128CompatVisitor;

    impl<'de> Visitor<'de> for U128CompatVisitor {
        type Value = u128;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("u128 as decimal string or non-negative integer")
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
            value
                .parse::<u128>()
                .map_err(|_| E::custom("invalid u128 decimal string"))
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
