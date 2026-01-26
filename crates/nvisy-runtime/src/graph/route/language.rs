//! Language evaluator for routing by detected content language.

use nvisy_dal::datatype::AnyDataValue;

/// Evaluates language based on metadata.
#[derive(Debug, Clone)]
pub struct LanguageEvaluator {
    /// Language codes to match.
    codes: Vec<String>,
    /// Minimum confidence threshold.
    min_confidence: f32,
}

impl LanguageEvaluator {
    /// Creates a new language evaluator.
    pub fn new(codes: Vec<String>, min_confidence: f32) -> Self {
        Self {
            codes,
            min_confidence,
        }
    }

    /// Evaluates whether the data matches any of the language codes.
    pub fn evaluate(&self, data: &AnyDataValue) -> bool {
        let detected_lang = self.get_metadata_string(data, "language");
        let confidence = self.get_metadata_f32(data, "language_confidence");

        match (detected_lang, confidence) {
            (Some(lang), Some(conf)) => {
                self.codes
                    .iter()
                    .any(|code| lang.eq_ignore_ascii_case(code))
                    && conf >= self.min_confidence
            }
            (Some(lang), None) => self
                .codes
                .iter()
                .any(|code| lang.eq_ignore_ascii_case(code)),
            _ => false,
        }
    }

    /// Gets a string metadata value.
    fn get_metadata_string(&self, data: &AnyDataValue, key: &str) -> Option<String> {
        match data {
            AnyDataValue::Object(obj) => obj.metadata.get(key).and_then(json_to_string),
            AnyDataValue::Record(record) => record.columns.get(key).and_then(json_to_string),
            AnyDataValue::Document(doc) => doc.metadata.get(key).and_then(json_to_string),
            _ => None,
        }
    }

    /// Gets an f32 metadata value.
    fn get_metadata_f32(&self, data: &AnyDataValue, key: &str) -> Option<f32> {
        match data {
            AnyDataValue::Object(obj) => obj
                .metadata
                .get(key)
                .and_then(|v| v.as_f64())
                .map(|v| v as f32),
            AnyDataValue::Record(record) => record
                .columns
                .get(key)
                .and_then(|v| v.as_f64())
                .map(|v| v as f32),
            AnyDataValue::Document(doc) => doc
                .metadata
                .get(key)
                .and_then(|v| v.as_f64())
                .map(|v| v as f32),
            _ => None,
        }
    }
}

/// Converts a JSON value to a string.
fn json_to_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}
