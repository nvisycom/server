//! Compiled routing node types.

use jiff::Timestamp;
use nvisy_dal::AnyDataValue;

use crate::definition::{
    ContentTypeCategory, DateField, PatternMatchType, SwitchCondition, SwitchDef,
};

/// Compiled switch node - ready to route data.
///
/// Evaluates a condition against input data and determines
/// which output port to route the data to.
#[derive(Debug, Clone)]
pub struct CompiledSwitch {
    /// The condition to evaluate.
    condition: SwitchCondition,
    /// Output port for data matching the condition.
    match_port: String,
    /// Output port for data not matching the condition.
    else_port: String,
}

impl CompiledSwitch {
    /// Creates a new compiled switch.
    pub fn new(condition: SwitchCondition, match_port: String, else_port: String) -> Self {
        Self {
            condition,
            match_port,
            else_port,
        }
    }

    /// Returns all output port names.
    pub fn output_ports(&self) -> impl Iterator<Item = &str> {
        [self.match_port.as_str(), self.else_port.as_str()].into_iter()
    }

    /// Evaluates the switch condition against input data.
    ///
    /// Returns the appropriate output port name based on whether
    /// the condition matches.
    pub fn evaluate(&self, data: &AnyDataValue) -> &str {
        if self.evaluate_condition(data) {
            &self.match_port
        } else {
            &self.else_port
        }
    }

    /// Evaluates the condition against the data.
    fn evaluate_condition(&self, data: &AnyDataValue) -> bool {
        match &self.condition {
            SwitchCondition::ContentType(c) => self.match_content_type(data, c.category),

            SwitchCondition::FileExtension(c) => {
                match data {
                    AnyDataValue::Blob(blob) => blob.path.rsplit('.').next().is_some_and(|ext| {
                        c.extensions.iter().any(|e| ext.eq_ignore_ascii_case(e))
                    }),
                    _ => false,
                }
            }

            SwitchCondition::FileSize(c) => match data {
                AnyDataValue::Blob(blob) => {
                    let size = blob.data.len() as u64;
                    let above_min = c.min_bytes.is_none_or(|min| size >= min);
                    let below_max = c.max_bytes.is_none_or(|max| size <= max);
                    above_min && below_max
                }
                _ => false,
            },

            SwitchCondition::PageCount(c) => {
                let page_count = self.get_metadata_u32(data, "page_count");
                match page_count {
                    Some(count) => {
                        let above_min = c.min_pages.is_none_or(|min| count >= min);
                        let below_max = c.max_pages.is_none_or(|max| count <= max);
                        above_min && below_max
                    }
                    None => false,
                }
            }

            SwitchCondition::Duration(c) => {
                let duration_secs = self.get_metadata_i64(data, "duration_seconds");
                match duration_secs {
                    Some(secs) => {
                        let above_min = c.min_seconds.is_none_or(|min| secs >= min);
                        let below_max = c.max_seconds.is_none_or(|max| secs <= max);
                        above_min && below_max
                    }
                    None => false,
                }
            }

            SwitchCondition::Language(c) => {
                let detected_lang = self.get_metadata_string(data, "language");
                let confidence = self.get_metadata_f32(data, "language_confidence");
                match (detected_lang, confidence) {
                    (Some(lang), Some(conf)) => {
                        lang.eq_ignore_ascii_case(&c.code) && conf >= c.min_confidence
                    }
                    (Some(lang), None) => lang.eq_ignore_ascii_case(&c.code),
                    _ => false,
                }
            }

            SwitchCondition::FileDate(c) => {
                let timestamp = match c.field {
                    DateField::Created => self.get_metadata_timestamp(data, "created_at"),
                    DateField::Modified => self.get_metadata_timestamp(data, "modified_at"),
                };
                match timestamp {
                    Some(ts) => {
                        let after_ok = c.after.is_none_or(|after| ts >= after);
                        let before_ok = c.before.is_none_or(|before| ts <= before);
                        after_ok && before_ok
                    }
                    None => false,
                }
            }

            SwitchCondition::FileName(c) => match data {
                AnyDataValue::Blob(blob) => {
                    let filename = blob.path.rsplit('/').next().unwrap_or(&blob.path);
                    match c.match_type {
                        PatternMatchType::Glob => glob_match(&c.pattern, filename),
                        PatternMatchType::Regex => {
                            // Fall back to glob matching for now
                            glob_match(&c.pattern, filename)
                        }
                        PatternMatchType::Exact => filename == c.pattern,
                        PatternMatchType::Contains => {
                            filename.to_lowercase().contains(&c.pattern.to_lowercase())
                        }
                    }
                }
                _ => false,
            },
        }
    }

    /// Matches content type category against data.
    fn match_content_type(&self, data: &AnyDataValue, category: ContentTypeCategory) -> bool {
        match data {
            AnyDataValue::Blob(blob) => {
                let mime = blob
                    .content_type
                    .as_deref()
                    .unwrap_or("application/octet-stream");
                match category {
                    ContentTypeCategory::Image => mime.starts_with("image/"),
                    ContentTypeCategory::Document => {
                        mime == "application/pdf"
                            || mime.starts_with("application/vnd.")
                            || mime == "application/msword"
                    }
                    ContentTypeCategory::Text => {
                        mime.starts_with("text/") || mime == "application/json"
                    }
                    ContentTypeCategory::Audio => mime.starts_with("audio/"),
                    ContentTypeCategory::Video => mime.starts_with("video/"),
                    ContentTypeCategory::Spreadsheet => {
                        mime == "application/vnd.ms-excel"
                            || mime.contains("spreadsheet")
                            || mime == "text/csv"
                    }
                    ContentTypeCategory::Presentation => {
                        mime == "application/vnd.ms-powerpoint" || mime.contains("presentation")
                    }
                    ContentTypeCategory::Archive => {
                        mime == "application/zip"
                            || mime == "application/x-tar"
                            || mime == "application/gzip"
                            || mime == "application/x-rar-compressed"
                            || mime == "application/x-7z-compressed"
                    }
                    ContentTypeCategory::Code => {
                        mime.starts_with("text/x-")
                            || mime == "application/javascript"
                            || mime == "application/typescript"
                            || mime == "application/x-python"
                    }
                    ContentTypeCategory::Other => true,
                }
            }
            _ => false,
        }
    }

    /// Gets a string metadata value from JSON.
    fn get_metadata_string(&self, data: &AnyDataValue, key: &str) -> Option<String> {
        match data {
            AnyDataValue::Blob(blob) => blob.metadata.get(key).and_then(json_to_string),
            AnyDataValue::Record(record) => record.columns.get(key).and_then(json_to_string),
            _ => None,
        }
    }

    /// Gets a u32 metadata value.
    fn get_metadata_u32(&self, data: &AnyDataValue, key: &str) -> Option<u32> {
        match data {
            AnyDataValue::Blob(blob) => blob
                .metadata
                .get(key)
                .and_then(json_to_u64)
                .map(|v| v as u32),
            AnyDataValue::Record(record) => record
                .columns
                .get(key)
                .and_then(json_to_u64)
                .map(|v| v as u32),
            _ => None,
        }
    }

    /// Gets an i64 metadata value.
    fn get_metadata_i64(&self, data: &AnyDataValue, key: &str) -> Option<i64> {
        match data {
            AnyDataValue::Blob(blob) => blob.metadata.get(key).and_then(json_to_i64),
            AnyDataValue::Record(record) => record.columns.get(key).and_then(json_to_i64),
            _ => None,
        }
    }

    /// Gets an f32 metadata value.
    fn get_metadata_f32(&self, data: &AnyDataValue, key: &str) -> Option<f32> {
        self.get_metadata_f64(data, key).map(|v| v as f32)
    }

    /// Gets an f64 metadata value.
    fn get_metadata_f64(&self, data: &AnyDataValue, key: &str) -> Option<f64> {
        match data {
            AnyDataValue::Blob(blob) => blob.metadata.get(key).and_then(json_to_f64),
            AnyDataValue::Record(record) => record.columns.get(key).and_then(json_to_f64),
            _ => None,
        }
    }

    /// Gets a timestamp metadata value.
    fn get_metadata_timestamp(&self, data: &AnyDataValue, key: &str) -> Option<Timestamp> {
        let s = self.get_metadata_string(data, key)?;
        s.parse().ok()
    }
}

impl From<SwitchDef> for CompiledSwitch {
    fn from(def: SwitchDef) -> Self {
        Self::new(def.condition, def.match_port, def.else_port)
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

/// Converts a JSON value to u64.
fn json_to_u64(value: &serde_json::Value) -> Option<u64> {
    value.as_u64().or_else(|| value.as_f64().map(|f| f as u64))
}

/// Converts a JSON value to i64.
fn json_to_i64(value: &serde_json::Value) -> Option<i64> {
    value.as_i64().or_else(|| value.as_f64().map(|f| f as i64))
}

/// Converts a JSON value to f64.
fn json_to_f64(value: &serde_json::Value) -> Option<f64> {
    value.as_f64()
}

/// Simple glob-style pattern matching.
///
/// Supports:
/// - `*` matches any sequence of characters
/// - `?` matches any single character
/// - Literal matching for other characters (case-insensitive)
fn glob_match(pattern: &str, text: &str) -> bool {
    let mut pattern_chars = pattern.chars().peekable();
    let mut text_chars = text.chars().peekable();

    while let Some(p) = pattern_chars.next() {
        match p {
            '*' => {
                // Try matching zero or more characters
                if pattern_chars.peek().is_none() {
                    // Pattern ends with *, matches everything remaining
                    return true;
                }
                // Try each position in the remaining text
                loop {
                    let remaining_pattern: String = pattern_chars.clone().collect();
                    let remaining_text: String = text_chars.clone().collect();
                    if glob_match(&remaining_pattern, &remaining_text) {
                        return true;
                    }
                    if text_chars.next().is_none() {
                        return false;
                    }
                }
            }
            '?' => {
                // Match any single character
                if text_chars.next().is_none() {
                    return false;
                }
            }
            c => {
                // Literal match (case-insensitive)
                match text_chars.next() {
                    Some(t) if c.eq_ignore_ascii_case(&t) => {}
                    _ => return false,
                }
            }
        }
    }

    // Pattern is exhausted, text should also be exhausted
    text_chars.peek().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definition::{ContentTypeCondition, FileExtensionCondition};

    #[test]
    fn test_evaluate_file_extension() {
        let switch = CompiledSwitch::new(
            SwitchCondition::FileExtension(FileExtensionCondition {
                extensions: vec!["pdf".into(), "docx".into()],
            }),
            "documents".into(),
            "other".into(),
        );

        let pdf = AnyDataValue::Blob(nvisy_dal::datatype::Blob::new("report.pdf", vec![]));
        let txt = AnyDataValue::Blob(nvisy_dal::datatype::Blob::new("notes.txt", vec![]));

        assert_eq!(switch.evaluate(&pdf), "documents");
        assert_eq!(switch.evaluate(&txt), "other");
    }

    #[test]
    fn test_evaluate_content_type() {
        let switch = CompiledSwitch::new(
            SwitchCondition::ContentType(ContentTypeCondition {
                category: ContentTypeCategory::Image,
            }),
            "images".into(),
            "other".into(),
        );

        let mut blob = nvisy_dal::datatype::Blob::new("photo.jpg", vec![]);
        blob.content_type = Some("image/jpeg".into());
        let image = AnyDataValue::Blob(blob);

        let mut blob = nvisy_dal::datatype::Blob::new("doc.pdf", vec![]);
        blob.content_type = Some("application/pdf".into());
        let pdf = AnyDataValue::Blob(blob);

        assert_eq!(switch.evaluate(&image), "images");
        assert_eq!(switch.evaluate(&pdf), "other");
    }
}
