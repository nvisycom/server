//! Object store types and result helpers.

use std::collections::HashMap;

use bytes::Bytes;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// Metadata for object storage
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObjectMeta {
    pub content_type: Option<String>,
    pub headers: HashMap<String, String>,
}

impl ObjectMeta {
    /// Create new object metadata
    pub fn new() -> Self {
        Self::default()
    }

    /// Set content type
    pub fn content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    /// Add a header
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Add multiple headers
    pub fn headers(mut self, headers: HashMap<String, String>) -> Self {
        self.headers.extend(headers);
        self
    }

    /// Check if content type is set
    pub fn has_content_type(&self) -> bool {
        self.content_type.is_some()
    }

    /// Get content type or default
    pub fn content_type_or_default(&self) -> &str {
        self.content_type
            .as_deref()
            .unwrap_or("application/octet-stream")
    }

    /// Check if header exists
    pub fn has_header(&self, key: &str) -> bool {
        self.headers.contains_key(key)
    }

    /// Get header value
    pub fn get_header(&self, key: &str) -> Option<&str> {
        self.headers.get(key).map(|s| s.as_str())
    }

    /// Helper for common file types
    pub fn for_file_type(extension: &str) -> Self {
        let content_type = match extension.to_lowercase().as_str() {
            "txt" => "text/plain",
            "html" | "htm" => "text/html",
            "css" => "text/css",
            "js" => "application/javascript",
            "json" => "application/json",
            "xml" => "application/xml",
            "pdf" => "application/pdf",
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "gif" => "image/gif",
            "svg" => "image/svg+xml",
            "webp" => "image/webp",
            "mp4" => "video/mp4",
            "webm" => "video/webm",
            "mp3" => "audio/mpeg",
            "wav" => "audio/wav",
            "ogg" => "audio/ogg",
            "zip" => "application/zip",
            "tar" => "application/x-tar",
            "gz" => "application/gzip",
            "bz2" => "application/x-bzip2",
            "7z" => "application/x-7z-compressed",
            "rar" => "application/vnd.rar",
            "doc" => "application/msword",
            "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "xls" => "application/vnd.ms-excel",
            "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "ppt" => "application/vnd.ms-powerpoint",
            "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            _ => "application/octet-stream",
        };

        Self::new().content_type(content_type)
    }

    /// Helper for common image types
    pub fn for_image() -> Self {
        Self::new().content_type("image/jpeg")
    }

    /// Helper for JSON data
    pub fn for_json() -> Self {
        Self::new()
            .content_type("application/json")
            .header("charset", "utf-8")
    }

    /// Helper for text data
    pub fn for_text() -> Self {
        Self::new()
            .content_type("text/plain")
            .header("charset", "utf-8")
    }

    /// Helper for binary data
    pub fn for_binary() -> Self {
        Self::new().content_type("application/octet-stream")
    }
}

/// Object information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectInfo {
    pub name: String,
    pub size: u64,
    pub modified: Option<Timestamp>,
    pub nuid: String,
    pub bucket: String,
    pub headers: HashMap<String, String>,
    pub content_type: Option<String>,
    pub chunk_count: usize,
}

impl ObjectInfo {
    /// Create new object info
    pub fn new(
        name: impl Into<String>,
        size: u64,
        nuid: impl Into<String>,
        bucket: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            size,
            modified: Some(Timestamp::now()),
            nuid: nuid.into(),
            bucket: bucket.into(),
            headers: HashMap::new(),
            content_type: None,
            chunk_count: 0,
        }
    }

    /// Check if object is empty
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    /// Get human readable size
    pub fn human_size(&self) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = self.size as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        if unit_index == 0 {
            format!("{} {}", self.size, UNITS[unit_index])
        } else {
            format!("{:.1} {}", size, UNITS[unit_index])
        }
    }

    /// Get object age in seconds
    pub fn age_seconds(&self) -> Option<i64> {
        self.modified.map(|modified| {
            let now = Timestamp::now();
            now.duration_since(modified).as_secs()
        })
    }

    /// Check if object is chunked
    pub fn is_chunked(&self) -> bool {
        self.chunk_count > 1
    }

    /// Get content type or default
    pub fn content_type_or_default(&self) -> &str {
        self.content_type
            .as_deref()
            .unwrap_or("application/octet-stream")
    }

    /// Check if object is an image
    pub fn is_image(&self) -> bool {
        self.content_type_or_default().starts_with("image/")
    }

    /// Check if object is text
    pub fn is_text(&self) -> bool {
        let ct = self.content_type_or_default();
        ct.starts_with("text/") || ct == "application/json" || ct == "application/xml"
    }

    /// Check if object is binary
    pub fn is_binary(&self) -> bool {
        !self.is_text()
    }

    /// Get file extension from name
    pub fn extension(&self) -> Option<&str> {
        std::path::Path::new(&self.name)
            .extension()
            .and_then(|ext| ext.to_str())
    }
}

/// Result of a put operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PutResult {
    pub name: String,
    pub size: u64,
    pub nuid: String,
    pub bucket: String,
}

impl PutResult {
    /// Create new put result
    pub fn new(
        name: impl Into<String>,
        size: u64,
        nuid: impl Into<String>,
        bucket: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            size,
            nuid: nuid.into(),
            bucket: bucket.into(),
        }
    }

    /// Check if put was successful (size > 0)
    pub fn is_success(&self) -> bool {
        !self.name.is_empty() && !self.nuid.is_empty()
    }

    /// Get human readable size
    pub fn human_size(&self) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = self.size as f64;
        let mut unit_index = 0;

        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }

        if unit_index == 0 {
            format!("{} {}", self.size, UNITS[unit_index])
        } else {
            format!("{:.1} {}", size, UNITS[unit_index])
        }
    }

    /// Convert to ObjectInfo (partial)
    pub fn to_object_info(&self) -> ObjectInfo {
        ObjectInfo::new(&self.name, self.size, &self.nuid, &self.bucket)
    }
}

/// Result of a get operation
#[derive(Debug, Clone)]
pub struct GetResult {
    pub data: Bytes,
    pub info: ObjectInfo,
}

impl GetResult {
    /// Create new get result
    pub fn new(data: Bytes, info: ObjectInfo) -> Self {
        Self { data, info }
    }

    /// Check if result is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get data size
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Get data as string (for text objects)
    pub fn as_string(&self) -> Result<String, std::str::Utf8Error> {
        std::str::from_utf8(&self.data).map(|s| s.to_string())
    }

    /// Get data as JSON value (for JSON objects)
    pub fn as_json<T>(&self) -> Result<T, serde_json::Error>
    where
        T: serde::de::DeserializeOwned,
    {
        serde_json::from_slice(&self.data)
    }

    /// Split into data and info
    pub fn into_parts(self) -> (Bytes, ObjectInfo) {
        (self.data, self.info)
    }

    /// Check if data matches expected size
    pub fn size_matches_info(&self) -> bool {
        self.data.len() == self.info.size as usize
    }

    /// Get human readable size
    pub fn human_size(&self) -> String {
        self.info.human_size()
    }
}

/// Object tombstone for soft deletes (internal type)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ObjectTombstone {
    pub object_name: String,
    pub deleted_at: Timestamp,
}

impl ObjectTombstone {
    pub fn new(object_name: impl Into<String>) -> Self {
        Self {
            object_name: object_name.into(),
            deleted_at: Timestamp::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_meta_builder() {
        let meta = ObjectMeta::new()
            .content_type("application/json")
            .header("x-custom", "value")
            .header("x-version", "1.0");

        assert_eq!(meta.content_type, Some("application/json".to_string()));
        assert_eq!(meta.headers.get("x-custom"), Some(&"value".to_string()));
        assert_eq!(meta.headers.get("x-version"), Some(&"1.0".to_string()));
        assert!(meta.has_content_type());
        assert!(meta.has_header("x-custom"));
        assert!(!meta.has_header("non-existent"));
        assert_eq!(meta.get_header("x-custom"), Some("value"));
    }

    #[test]
    fn test_file_type_detection() {
        let meta = ObjectMeta::for_file_type("json");
        assert_eq!(meta.content_type, Some("application/json".to_string()));

        let meta = ObjectMeta::for_file_type("png");
        assert_eq!(meta.content_type, Some("image/png".to_string()));

        let meta = ObjectMeta::for_file_type("unknown");
        assert_eq!(
            meta.content_type,
            Some("application/octet-stream".to_string())
        );
    }

    #[test]
    fn test_object_info_helpers() {
        let info = ObjectInfo::new("test.jpg", 1024 * 1024, "nuid123", "bucket1");

        assert!(!info.is_empty());
        assert_eq!(info.human_size(), "1.0 MB");
        assert!(!info.is_chunked());
        assert_eq!(info.extension(), Some("jpg"));
    }

    #[test]
    fn test_put_result() {
        let result = PutResult::new("test.txt", 100, "nuid456", "bucket2");

        assert!(result.is_success());
        assert_eq!(result.human_size(), "100 B");
    }

    #[test]
    fn test_get_result() {
        let data = Bytes::from("hello world");
        let info = ObjectInfo::new("hello.txt", 11, "nuid789", "bucket3");
        let result = GetResult::new(data, info);

        assert!(!result.is_empty());
        assert_eq!(result.size(), 11);
        assert!(result.size_matches_info());
        assert_eq!(result.as_string().unwrap(), "hello world");
    }

    #[test]
    fn test_human_size_calculations() {
        let info = ObjectInfo::new("test", 0, "nuid", "bucket");
        assert_eq!(info.human_size(), "0 B");

        let info = ObjectInfo::new("test", 1023, "nuid", "bucket");
        assert_eq!(info.human_size(), "1023 B");

        let info = ObjectInfo::new("test", 1024, "nuid", "bucket");
        assert_eq!(info.human_size(), "1.0 KB");

        let info = ObjectInfo::new("test", 1024 * 1024, "nuid", "bucket");
        assert_eq!(info.human_size(), "1.0 MB");

        let info = ObjectInfo::new("test", 1536 * 1024, "nuid", "bucket"); // 1.5 MB
        assert_eq!(info.human_size(), "1.5 MB");
    }

    #[test]
    fn test_content_type_helpers() {
        let meta = ObjectMeta::for_json();
        assert_eq!(meta.content_type, Some("application/json".to_string()));
        assert_eq!(meta.get_header("charset"), Some("utf-8"));

        let meta = ObjectMeta::for_text();
        assert_eq!(meta.content_type, Some("text/plain".to_string()));

        let meta = ObjectMeta::for_binary();
        assert_eq!(
            meta.content_type,
            Some("application/octet-stream".to_string())
        );
    }
}
