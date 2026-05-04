//! Serde helpers for fixed byte arrays (JSON-friendly).

use serde::de::{Error as DeError, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serializer};
use std::fmt;

pub mod hex32 {
    //! JSON: lowercase hex (64 chars). Binary codecs (`bincode`): raw `[u8;32]` unchanged.

    use super::*;
    use serde::Serialize;

    pub fn serialize<S>(bytes: &[u8; 32], ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if ser.is_human_readable() {
            ser.serialize_str(&hex::encode(bytes))
        } else {
            bytes.serialize(ser)
        }
    }

    struct Hex32Visitor;

    impl<'de> Visitor<'de> for Hex32Visitor {
        type Value = [u8; 32];

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("32-byte value: 64 hex chars or legacy JSON byte array")
        }

        fn visit_str<E: DeError>(self, s: &str) -> Result<Self::Value, E> {
            parse_hex32(s)
        }

        fn visit_string<E: DeError>(self, s: String) -> Result<Self::Value, E> {
            parse_hex32(&s)
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut v = Vec::with_capacity(32);
            while let Some(b) = seq.next_element::<u8>()? {
                v.push(b);
                if v.len() > 32 {
                    return Err(DeError::custom("hex32 legacy array: more than 32 elements"));
                }
            }
            if v.len() != 32 {
                return Err(DeError::custom("hex32 legacy array: need 32 u8 elements"));
            }
            let mut o = [0u8; 32];
            o.copy_from_slice(&v);
            Ok(o)
        }
    }

    fn parse_hex32<E: DeError>(s: &str) -> Result<[u8; 32], E> {
        let t = s.trim().strip_prefix("0x").unwrap_or(s).trim();
        if t.len() != 64 || !t.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(E::custom("hex32: expected 64 hex digits"));
        }
        let v = hex::decode(t).map_err(|e| E::custom(format!("hex32 decode: {e}")))?;
        let mut o = [0u8; 32];
        o.copy_from_slice(&v);
        Ok(o)
    }

    pub fn deserialize<'de, D>(de: D) -> Result<[u8; 32], D::Error>
    where
        D: Deserializer<'de>,
    {
        if de.is_human_readable() {
            de.deserialize_any(Hex32Visitor)
        } else {
            <[u8; 32]>::deserialize(de)
        }
    }
}

pub mod sig64 {
    use super::*;

    pub fn serialize<S>(bytes: &[u8; 64], ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ser.serialize_bytes(bytes.as_slice())
    }

    pub fn deserialize<'de, D>(de: D) -> Result<[u8; 64], D::Error>
    where
        D: Deserializer<'de>,
    {
        let v = <Vec<u8>>::deserialize(de)?;
        if v.len() != 64 {
            return Err(serde::de::Error::custom("need 64 bytes"));
        }
        let mut o = [0u8; 64];
        o.copy_from_slice(&v);
        Ok(o)
    }
}
