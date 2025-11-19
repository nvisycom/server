//! Object headers for NATS object storage.

use std::collections::HashMap;

use async_nats::HeaderMap;
use derive_more::{Deref, DerefMut};
use serde::{Deserialize, Serialize};

/// HTTP-like headers for object storage operations.
///
/// This struct represents headers that can be sent with object storage requests,
/// similar to HTTP headers but specific to object storage operations.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize, Deref, DerefMut)]
pub struct ObjectHeaders {
    /// Custom headers as key-value pairs
    #[deref]
    #[deref_mut]
    headers: HashMap<String, String>,
}

impl ObjectHeaders {
    /// Creates a new empty ObjectHeaders.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates ObjectHeaders from a HeaderMap.
    pub fn from_header_map(headers: HeaderMap) -> Self {
        let mut header_map = HashMap::new();
        for (key, values) in headers.iter() {
            if let Some(first_value) = values.first()
                && let Ok(value_str) = std::str::from_utf8(first_value.as_ref())
            {
                header_map.insert(key.to_string(), value_str.to_string());
            }
        }
        Self {
            headers: header_map,
        }
    }

    /// Creates a HeaderMap from ObjectHeaders.
    pub fn into_header_map(self) -> Option<HeaderMap> {
        if self.is_empty() {
            return None;
        }

        let mut header_map = HeaderMap::new();
        for (header_key, header_value) in self.headers.into_iter() {
            header_map.insert(header_key, header_value);
        }

        Some(header_map)
    }

    /// Sets a header value.
    pub fn set(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Gets a header value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.headers.get(key).map(|s| s.as_str())
    }

    /// Gets all headers as a reference to the HashMap.
    pub fn as_map(&self) -> &HashMap<String, String> {
        &self.headers
    }

    /// Consumes the ObjectHeaders and returns the inner HashMap.
    pub fn into_map(self) -> HashMap<String, String> {
        self.headers
    }

    /// Gets an iterator over all header key-value pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.headers.iter()
    }

    /// Gets a mutable iterator over all header key-value pairs.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&String, &mut String)> {
        self.headers.iter_mut()
    }

    /// Checks if the headers are empty.
    pub fn is_empty(&self) -> bool {
        self.headers.is_empty()
    }

    /// Gets the number of headers.
    pub fn len(&self) -> usize {
        self.headers.len()
    }

    /// Removes a header by key.
    pub fn remove(&mut self, key: &str) -> Option<String> {
        self.headers.remove(key)
    }

    /// Clears all headers.
    pub fn clear(&mut self) {
        self.headers.clear();
    }

    /// Checks if a header exists.
    pub fn contains_key(&self, key: &str) -> bool {
        self.headers.contains_key(key)
    }

    /// Inserts a header directly (mutable version of set).
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) -> Option<String> {
        self.headers.insert(key.into(), value.into())
    }

    /// Extends headers with another set of headers.
    pub fn extend(&mut self, other: ObjectHeaders) {
        self.headers.extend(other.headers);
    }

    /// Extends headers with a HashMap.
    pub fn extend_from_map(&mut self, other: HashMap<String, String>) {
        self.headers.extend(other);
    }
}

impl From<HashMap<String, String>> for ObjectHeaders {
    fn from(headers: HashMap<String, String>) -> Self {
        Self { headers }
    }
}

impl From<ObjectHeaders> for HashMap<String, String> {
    fn from(headers: ObjectHeaders) -> Self {
        headers.headers
    }
}

impl From<HeaderMap> for ObjectHeaders {
    fn from(value: HeaderMap) -> Self {
        Self::from_header_map(value)
    }
}

impl From<ObjectHeaders> for HeaderMap {
    fn from(value: ObjectHeaders) -> Self {
        value.into_header_map().unwrap_or_default()
    }
}

impl FromIterator<(String, String)> for ObjectHeaders {
    fn from_iter<T: IntoIterator<Item = (String, String)>>(iter: T) -> Self {
        Self {
            headers: iter.into_iter().collect(),
        }
    }
}

impl<'a> FromIterator<(&'a str, &'a str)> for ObjectHeaders {
    fn from_iter<T: IntoIterator<Item = (&'a str, &'a str)>>(iter: T) -> Self {
        Self {
            headers: iter
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_headers_creation() {
        let headers = ObjectHeaders::new();
        assert!(headers.is_empty());
        assert_eq!(headers.len(), 0);
    }

    #[test]
    fn test_object_headers_builder_pattern() {
        let headers = ObjectHeaders::new()
            .set("content-type", "application/json")
            .set("content-encoding", "gzip")
            .set("content-length", "1024")
            .set("cache-control", "max-age=3600");

        assert_eq!(headers.get("content-type"), Some("application/json"));
        assert_eq!(headers.get("content-encoding"), Some("gzip"));
        assert_eq!(headers.get("content-length"), Some("1024"));
        assert_eq!(headers.get("cache-control"), Some("max-age=3600"));
        assert_eq!(headers.len(), 4);
    }

    #[test]
    fn test_object_headers_from_map() {
        let mut map = HashMap::new();
        map.insert("content-type".to_string(), "text/html".to_string());
        map.insert("custom-header".to_string(), "value".to_string());

        let headers = ObjectHeaders::from(map.clone());
        assert_eq!(headers.get("content-type"), Some("text/html"));
        assert_eq!(headers.get("custom-header"), Some("value"));

        let converted_map: HashMap<String, String> = headers.into();
        assert_eq!(converted_map, map);
    }

    #[test]
    fn test_object_headers_mutations() {
        let mut headers = ObjectHeaders::new()
            .set("content-type", "text/plain")
            .set("test-header", "test-value");

        assert_eq!(headers.len(), 2);

        headers.remove("test-header");
        assert_eq!(headers.len(), 1);
        assert_eq!(headers.get("test-header"), None);

        headers.insert("new-header", "new-value");
        assert_eq!(headers.get("new-header"), Some("new-value"));

        headers.clear();
        assert!(headers.is_empty());
    }

    #[test]
    fn test_from_iterator() {
        let vec_headers = vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("cache-control".to_string(), "no-cache".to_string()),
        ];

        let headers: ObjectHeaders = vec_headers.into_iter().collect();
        assert_eq!(headers.get("content-type"), Some("application/json"));
        assert_eq!(headers.get("cache-control"), Some("no-cache"));

        let str_headers = vec![("etag", "123456"), ("last-modified", "Mon, 01 Jan 2024")];
        let headers2: ObjectHeaders = str_headers.into_iter().collect();
        assert_eq!(headers2.get("etag"), Some("123456"));
        assert_eq!(headers2.get("last-modified"), Some("Mon, 01 Jan 2024"));
    }

    #[test]
    fn test_extend() {
        let mut headers1 = ObjectHeaders::new().set("content-type", "text/plain");
        let headers2 = ObjectHeaders::new().set("cache-control", "max-age=300");

        headers1.extend(headers2);
        assert_eq!(headers1.get("content-type"), Some("text/plain"));
        assert_eq!(headers1.get("cache-control"), Some("max-age=300"));

        let mut map = HashMap::new();
        map.insert("etag".to_string(), "abc123".to_string());
        headers1.extend_from_map(map);
        assert_eq!(headers1.get("etag"), Some("abc123"));
    }

    #[test]
    fn test_deref() {
        let mut headers = ObjectHeaders::new();
        headers.insert("direct-key", "direct-value");

        // Test deref to HashMap functionality
        assert_eq!(headers.get("direct-key"), Some("direct-value"));
        assert!(headers.contains_key("direct-key"));
    }
}
