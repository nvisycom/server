//! [`RedactionKey`]: a caller-held key for the keyed redaction operators.

use std::borrow::Cow;
use std::fmt;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use elide_pipeline::provider::KeyConfig;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::Deserialize;

/// The length of a [`RedactionKey`] in bytes: what AES-256 requires, and a
/// sound length for HMAC.
const REDACTION_KEY_BYTES: usize = 32;

/// The key the `encrypt` and `hmac_hash` redaction operators run with, supplied
/// by the caller on a redact request: 32 bytes, base64 on the wire.
///
/// The caller holds it, so only the caller can decrypt encrypted values or match
/// HMAC pseudonyms to its own systems. It is used for one request and never
/// stored: it has no `Serialize`, and its `Debug` prints no key bytes.
pub struct RedactionKey(Vec<u8>);

impl RedactionKey {
    /// The engine key configuration for this key.
    #[must_use]
    pub fn into_key_config(self) -> KeyConfig {
        KeyConfig::Static { key: self.0 }
    }
}

impl fmt::Debug for RedactionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RedactionKey([REDACTED])")
    }
}

impl<'de> Deserialize<'de> for RedactionKey {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let encoded = String::deserialize(deserializer)?;
        let bytes = STANDARD
            .decode(encoded.as_bytes())
            .map_err(|_| serde::de::Error::custom("redaction key must be base64"))?;
        if bytes.len() != REDACTION_KEY_BYTES {
            return Err(serde::de::Error::custom(format!(
                "redaction key must be {REDACTION_KEY_BYTES} bytes, got {}",
                bytes.len()
            )));
        }
        Ok(Self(bytes))
    }
}

impl JsonSchema for RedactionKey {
    fn schema_name() -> Cow<'static, str> {
        "RedactionKey".into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        schemars::json_schema!({
            "type": "string",
            "format": "byte",
            "description": "A 32-byte key, base64-encoded.",
            "minLength": 44,
            "maxLength": 44,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(value: &str) -> serde_json::Result<RedactionKey> {
        serde_json::from_value(serde_json::Value::String(value.to_owned()))
    }

    #[test]
    fn a_key_must_be_32_base64_bytes() {
        assert!(parse(&STANDARD.encode([7u8; 32])).is_ok());
        assert!(parse(&STANDARD.encode([7u8; 16])).is_err());
        assert!(parse("not base64!").is_err());
    }

    #[test]
    fn a_key_never_prints_its_bytes() {
        let key = parse(&STANDARD.encode([7u8; 32])).unwrap();
        assert_eq!(format!("{key:?}"), "RedactionKey([REDACTED])");
    }
}
