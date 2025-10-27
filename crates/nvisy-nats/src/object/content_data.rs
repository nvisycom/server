//! Content data structure for storing and managing content with metadata
//!
//! This module provides the [`ContentData`] struct for storing content data
//! along with its metadata and source information.

use std::fmt;

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::object_headers::ObjectHeaders;
use super::object_metadata::ObjectMetadata;
use crate::{Error, Result};

/// Content data with metadata and computed hashes.
///
/// This struct is a minimal wrapper around `bytes::Bytes` that stores content data
/// along with optional metadata and headers.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Serialize, Deserialize)]
pub struct ContentData {
    /// The actual content data
    data: Bytes,
    /// Object metadata
    metadata: ObjectMetadata,
    /// Object headers
    headers: ObjectHeaders,
}

impl ContentData {
    /// Create new content data with just bytes
    ///
    /// # Example
    ///
    /// ```
    /// use nvisy_nats::object::ContentData;
    /// use bytes::Bytes;
    ///
    /// let data = Bytes::from("Hello, world!");
    /// let content = ContentData::new(data);
    ///
    /// assert_eq!(content.size(), 13);
    /// ```
    pub fn new(data: Bytes) -> Self {
        Self {
            data,
            metadata: ObjectMetadata::new(),
            headers: ObjectHeaders::new(),
        }
    }

    /// Set the metadata, consuming and returning self
    pub fn with_metadata(mut self, metadata: ObjectMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Set the headers, consuming and returning self
    pub fn with_headers(mut self, headers: ObjectHeaders) -> Self {
        self.headers = headers;
        self
    }

    /// Get the content data as bytes
    pub fn data(&self) -> &Bytes {
        &self.data
    }

    /// Get the metadata
    pub fn metadata(&self) -> &ObjectMetadata {
        &self.metadata
    }

    /// Get the headers
    pub fn headers(&self) -> &ObjectHeaders {
        &self.headers
    }

    /// Get the size of the content in bytes
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Get the content data as bytes slice
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Get the content data as bytes
    pub fn into_bytes(self) -> Bytes {
        self.data
    }

    /// Check if the content is likely text (basic heuristic)
    pub fn is_likely_text(&self) -> bool {
        self.data
            .iter()
            .all(|&b| b.is_ascii_graphic() || b.is_ascii_whitespace())
    }

    /// Try to convert the content data to a UTF-8 string
    ///
    /// # Errors
    ///
    /// Returns an error if the content data contains invalid UTF-8 sequences.
    pub fn as_string(&self) -> Result<String> {
        String::from_utf8(self.data.to_vec())
            .map_err(|e| Error::operation("utf8_decode", format!("Invalid UTF-8: {e}")))
    }

    /// Try to convert the content data to a UTF-8 string slice
    ///
    /// # Errors
    ///
    /// Returns an error if the content data contains invalid UTF-8 sequences.
    pub fn as_str(&self) -> Result<&str> {
        std::str::from_utf8(&self.data)
            .map_err(|e| Error::operation("utf8_decode", format!("Invalid UTF-8: {e}")))
    }

    /// Compute SHA256 hash of the content, returning the hash as bytes
    pub fn compute_sha256(&self) -> Bytes {
        let mut hasher = Sha256::new();
        hasher.update(&self.data);
        Bytes::from(hasher.finalize().to_vec())
    }

    /// Get the SHA256 hash as hex string
    pub fn sha256_hex(&self) -> String {
        hex::encode(self.compute_sha256())
    }

    /// Verify the content against a provided SHA256 hash
    ///
    /// # Errors
    ///
    /// Returns an error if the computed hash does not match the expected hash.
    pub fn verify_sha256(&self, expected_hash: impl AsRef<[u8]>) -> Result<()> {
        let actual_hash = self.compute_sha256();
        let expected = expected_hash.as_ref();

        if actual_hash.as_ref() == expected {
            Ok(())
        } else {
            Err(Error::operation(
                "hash_verify",
                format!(
                    "Hash mismatch: expected {}, got {}",
                    hex::encode(expected),
                    hex::encode(&actual_hash)
                ),
            ))
        }
    }

    /// Get a slice of the content data
    ///
    /// # Errors
    ///
    /// Returns an error if the end index is beyond the content length or if start is greater than end.
    pub fn slice(&self, start: usize, end: usize) -> Result<Bytes> {
        if end > self.data.len() {
            return Err(Error::operation(
                "slice",
                format!(
                    "Slice end {} exceeds content length {}",
                    end,
                    self.data.len()
                ),
            ));
        }
        if start > end {
            return Err(Error::operation(
                "slice",
                format!("Slice start {start} is greater than end {end}"),
            ));
        }

        Ok(self.data.slice(start..end))
    }

    /// Check if the content is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

// Implement From conversions for common types
impl From<&str> for ContentData {
    fn from(s: &str) -> Self {
        Self::new(Bytes::from(s.to_string()))
    }
}

impl From<String> for ContentData {
    fn from(s: String) -> Self {
        Self::new(Bytes::from(s))
    }
}

impl From<&[u8]> for ContentData {
    fn from(bytes: &[u8]) -> Self {
        Self::new(Bytes::copy_from_slice(bytes))
    }
}

impl From<Vec<u8>> for ContentData {
    fn from(vec: Vec<u8>) -> Self {
        Self::new(Bytes::from(vec))
    }
}

impl From<Bytes> for ContentData {
    fn from(bytes: Bytes) -> Self {
        Self::new(bytes)
    }
}

impl AsRef<[u8]> for ContentData {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

impl fmt::Display for ContentData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Ok(text) = self.as_str() {
            write!(f, "{text}")
        } else {
            write!(f, "[Binary data: {} bytes]", self.size())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_data_creation() {
        let data = Bytes::from("Hello, world!");
        let content = ContentData::new(data);

        assert!(content.metadata().is_empty());
        assert!(content.headers().is_empty());
        assert_eq!(content.size(), 13);
    }

    #[test]
    fn test_with_methods() {
        let metadata = ObjectMetadata::new();
        let headers = ObjectHeaders::new();
        let content = ContentData::new(Bytes::from("Hello, world!"))
            .with_metadata(metadata.clone())
            .with_headers(headers.clone());

        assert_eq!(content.metadata(), &metadata);
        assert_eq!(content.headers(), &headers);
    }

    #[test]
    fn test_sha256_computation() {
        let content = ContentData::from("Hello, world!");
        let hash = content.compute_sha256();

        assert_eq!(hash.len(), 32); // SHA256 is 32 bytes

        // Test getting hash again
        let hash2 = content.compute_sha256();
        assert_eq!(hash, hash2);
    }

    #[test]
    fn test_sha256_verification() {
        let content = ContentData::from("Hello, world!");
        let hash = content.compute_sha256();

        // Should verify successfully against itself
        assert!(content.verify_sha256(&hash).is_ok());

        // Should fail against different hash
        let wrong_hash = vec![0u8; 32];
        assert!(content.verify_sha256(&wrong_hash).is_err());
    }

    #[test]
    fn test_string_conversion() {
        let content = ContentData::from("Hello, world!");
        assert_eq!(content.as_string().unwrap(), "Hello, world!");
        assert_eq!(content.as_str().unwrap(), "Hello, world!");

        let binary_content = ContentData::from(vec![0xFF, 0xFE, 0xFD]);
        assert!(binary_content.as_string().is_err());
        assert!(binary_content.as_str().is_err());
    }

    #[test]
    fn test_is_likely_text() {
        let text_content = ContentData::from("Hello, world!");
        assert!(text_content.is_likely_text());

        let binary_content = ContentData::from(vec![0xFF, 0xFE, 0xFD]);
        assert!(!binary_content.is_likely_text());
    }

    #[test]
    fn test_slice() {
        let content = ContentData::from("Hello, world!");

        let slice = content.slice(0, 5).unwrap();
        assert_eq!(slice, Bytes::from("Hello"));

        let slice = content.slice(7, 12).unwrap();
        assert_eq!(slice, Bytes::from("world"));

        // Test bounds checking
        assert!(content.slice(0, 100).is_err());
        assert!(content.slice(10, 5).is_err());
    }

    #[test]
    fn test_from_conversions() {
        let from_str = ContentData::from("test");
        let from_string = ContentData::from("test".to_string());
        let from_bytes = ContentData::from(b"test".as_slice());
        let from_vec = ContentData::from(b"test".to_vec());
        let from_bytes_type = ContentData::from(Bytes::from("test"));

        assert_eq!(from_str.as_str().unwrap(), "test");
        assert_eq!(from_string.as_str().unwrap(), "test");
        assert_eq!(from_bytes.as_str().unwrap(), "test");
        assert_eq!(from_vec.as_str().unwrap(), "test");
        assert_eq!(from_bytes_type.as_str().unwrap(), "test");
    }

    #[test]
    fn test_display() {
        let text_content = ContentData::from("Hello");
        assert_eq!(format!("{}", text_content), "Hello");

        let binary_content = ContentData::from(vec![0xFF, 0xFE]);
        assert!(format!("{}", binary_content).contains("Binary data"));
    }

    #[test]
    fn test_cloning_is_cheap() {
        let original = ContentData::from("Hello, world!");
        let cloned = original.clone();

        // They should be equal
        assert_eq!(original, cloned);

        // But the underlying bytes should share the same memory
        assert_eq!(original.data.as_ptr(), cloned.data.as_ptr());
    }

    #[test]
    fn test_into_bytes() {
        let content = ContentData::from("Hello, world!");
        let bytes = content.into_bytes();
        assert_eq!(bytes, Bytes::from("Hello, world!"));
    }

    #[test]
    fn test_empty_content() {
        let content = ContentData::from("");
        assert!(content.is_empty());
        assert_eq!(content.size(), 0);
    }

    #[test]
    fn test_getters() {
        let data = Bytes::from("Hello, world!");
        let content = ContentData::new(data.clone());

        assert_eq!(content.data(), &data);
        assert!(content.metadata().is_empty());
        assert!(content.headers().is_empty());
    }
}
