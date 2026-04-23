//! Serde helpers for fixed byte arrays (JSON-friendly).

use serde::{Deserialize, Deserializer, Serializer};

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
