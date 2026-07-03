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

    pub(super) struct Hex32Visitor;

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

pub mod opt_hex32 {
    //! JSON: null or lowercase hex. Binary codecs keep `Option<[u8;32]>` unchanged.

    use super::*;
    use serde::Serialize;

    pub fn serialize<S>(bytes: &Option<[u8; 32]>, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if ser.is_human_readable() {
            match bytes {
                Some(bytes) => ser.serialize_some(&hex::encode(bytes)),
                None => ser.serialize_none(),
            }
        } else {
            bytes.serialize(ser)
        }
    }

    struct OptHex32Visitor;

    impl<'de> Visitor<'de> for OptHex32Visitor {
        type Value = Option<[u8; 32]>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("null, 64 hex chars, or legacy JSON byte array")
        }

        fn visit_none<E: DeError>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_unit<E: DeError>(self) -> Result<Self::Value, E> {
            Ok(None)
        }

        fn visit_some<D>(self, de: D) -> Result<Self::Value, D::Error>
        where
            D: Deserializer<'de>,
        {
            de.deserialize_any(hex32::Hex32Visitor).map(Some)
        }
    }

    pub fn deserialize<'de, D>(de: D) -> Result<Option<[u8; 32]>, D::Error>
    where
        D: Deserializer<'de>,
    {
        if de.is_human_readable() {
            de.deserialize_option(OptHex32Visitor)
        } else {
            Option::<[u8; 32]>::deserialize(de)
        }
    }
}

pub mod sig64 {
    //! JSON: lowercase hex (128 chars). Binary codec representation is unchanged.

    use super::*;

    pub fn serialize<S>(bytes: &[u8; 64], ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        if ser.is_human_readable() {
            ser.serialize_str(&hex::encode(bytes))
        } else {
            ser.serialize_bytes(bytes.as_slice())
        }
    }

    struct Sig64Visitor;

    impl<'de> Visitor<'de> for Sig64Visitor {
        type Value = [u8; 64];

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("64-byte signature: 128 hex chars or legacy JSON byte array")
        }

        fn visit_str<E: DeError>(self, s: &str) -> Result<Self::Value, E> {
            parse_sig64(s)
        }

        fn visit_string<E: DeError>(self, s: String) -> Result<Self::Value, E> {
            parse_sig64(&s)
        }

        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let mut out = [0u8; 64];
            for byte in &mut out {
                *byte = seq
                    .next_element::<u8>()?
                    .ok_or_else(|| DeError::custom("sig64 legacy array: need 64 u8 elements"))?;
            }
            if seq.next_element::<serde::de::IgnoredAny>()?.is_some() {
                return Err(DeError::custom("sig64 legacy array: more than 64 elements"));
            }
            Ok(out)
        }
    }

    fn parse_sig64<E: DeError>(s: &str) -> Result<[u8; 64], E> {
        let trimmed = s.trim();
        let text = trimmed.strip_prefix("0x").unwrap_or(trimmed);
        if text.len() != 128 || !text.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(E::custom("sig64: expected 128 hex digits"));
        }
        let decoded = hex::decode(text).map_err(|e| E::custom(format!("sig64 decode: {e}")))?;
        let mut out = [0u8; 64];
        out.copy_from_slice(&decoded);
        Ok(out)
    }

    pub fn deserialize<'de, D>(de: D) -> Result<[u8; 64], D::Error>
    where
        D: Deserializer<'de>,
    {
        if de.is_human_readable() {
            return de.deserialize_any(Sig64Visitor);
        }
        let v = <Vec<u8>>::deserialize(de)?;
        if v.len() != 64 {
            return Err(serde::de::Error::custom("need 64 bytes"));
        }
        let mut o = [0u8; 64];
        o.copy_from_slice(&v);
        Ok(o)
    }
}

#[cfg(test)]
mod tests {
    use super::{opt_hex32, sig64};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct SigWire {
        #[serde(with = "sig64")]
        value: [u8; 64],
    }

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct OptWire {
        #[serde(with = "opt_hex32")]
        value: Option<[u8; 32]>,
    }

    #[test]
    fn sig64_json_hex_roundtrip() {
        let wire = SigWire { value: [0xab; 64] };
        let json = serde_json::to_string(&wire).expect("serialize sig64");
        assert_eq!(json, format!(r#"{{"value":"{}"}}"#, "ab".repeat(64)));
        assert_eq!(serde_json::from_str::<SigWire>(&json).unwrap(), wire);
        let prefixed = format!(r#"{{"value":"0x{}"}}"#, "ab".repeat(64));
        assert_eq!(serde_json::from_str::<SigWire>(&prefixed).unwrap(), wire);
    }

    #[test]
    fn sig64_legacy_array_works() {
        let json = format!(
            r#"{{"value":{}}}"#,
            serde_json::to_string(&vec![7u8; 64]).unwrap()
        );
        assert_eq!(
            serde_json::from_str::<SigWire>(&json).unwrap(),
            SigWire { value: [7u8; 64] }
        );
    }

    #[test]
    fn opt_hex32_json_roundtrip() {
        let some = OptWire {
            value: Some([0xcd; 32]),
        };
        let json = serde_json::to_string(&some).expect("serialize optional hex32");
        assert_eq!(json, format!(r#"{{"value":"{}"}}"#, "cd".repeat(32)));
        assert_eq!(serde_json::from_str::<OptWire>(&json).unwrap(), some);

        let none = OptWire { value: None };
        let json = serde_json::to_string(&none).expect("serialize optional null");
        assert_eq!(json, r#"{"value":null}"#);
        assert_eq!(serde_json::from_str::<OptWire>(&json).unwrap(), none);
    }

    #[test]
    fn opt_hex32_legacy_array_works() {
        let json = format!(
            r#"{{"value":{}}}"#,
            serde_json::to_string(&[9u8; 32]).unwrap()
        );
        assert_eq!(
            serde_json::from_str::<OptWire>(&json).unwrap(),
            OptWire {
                value: Some([9u8; 32])
            }
        );
    }
}
