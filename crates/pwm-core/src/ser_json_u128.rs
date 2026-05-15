//! JSON serde helpers for u128 wire compatibility.

use serde::de::{self, Visitor};
use serde::{Deserializer, Serializer};
use std::fmt;

pub fn serialize<S: Serializer>(v: &u128, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(&v.to_string())
}

pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<u128, D::Error> {
    d.deserialize_any(U128Visitor)
}

pub mod opt {
    use super::{deserialize as de_u128, serialize as ser_u128};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(v: &Option<u128>, s: S) -> Result<S::Ok, S::Error> {
        match v {
            Some(inner) => ser_u128(inner, s),
            None => s.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u128>, D::Error> {
        Option::<CompatU128>::deserialize(d).map(|v| v.map(|x| x.0))
    }

    struct CompatU128(pub u128);

    impl<'de> Deserialize<'de> for CompatU128 {
        fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
            de_u128(d).map(Self)
        }
    }
}

struct U128Visitor;

impl<'de> Visitor<'de> for U128Visitor {
    type Value = u128;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a decimal u128 string or u64 number")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
        v.parse::<u128>()
            .map_err(|_| E::custom("invalid decimal string for u128"))
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
        self.visit_str(&v)
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
        Ok(v as u128)
    }
}
