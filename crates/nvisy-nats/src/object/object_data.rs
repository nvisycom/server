//! Result types for object store operations.

use async_nats::jetstream::object_store::{self, ObjectInfo};

/// Result of a put operation containing upload metadata.
///
/// All fields are private to ensure immutability after creation.
#[derive(Debug, Clone)]
pub struct PutResult {
    /// Size in bytes as reported by NATS.
    size: u64,
    /// SHA-256 hash computed during streaming.
    sha256: Vec<u8>,
    /// SHA-256 hash as hex string.
    sha256_hex: String,
    /// NATS object unique identifier.
    nuid: String,
}

impl PutResult {
    /// Creates a new put result.
    pub(crate) fn new(size: u64, sha256: Vec<u8>, sha256_hex: String, nuid: String) -> Self {
        Self {
            size,
            sha256,
            sha256_hex,
            nuid,
        }
    }

    /// Returns the size in bytes.
    #[inline]
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Returns the SHA-256 hash as bytes.
    #[inline]
    pub fn sha256(&self) -> &[u8] {
        &self.sha256
    }

    /// Returns the SHA-256 hash as a hex string.
    #[inline]
    pub fn sha256_hex(&self) -> &str {
        &self.sha256_hex
    }

    /// Returns the NATS object unique identifier.
    #[inline]
    pub fn nuid(&self) -> &str {
        &self.nuid
    }
}

/// Result of a get operation with streaming reader.
///
/// Provides access to the object content via an async reader
/// and metadata about the stored object.
pub struct GetResult {
    /// The async reader for streaming the object content.
    reader: object_store::Object,
    /// Object metadata including size.
    info: ObjectInfo,
}

impl GetResult {
    /// Creates a new get result.
    pub(crate) fn new(reader: object_store::Object, info: ObjectInfo) -> Self {
        Self { reader, info }
    }

    /// Returns the async reader for streaming the object content.
    ///
    /// The reader implements `AsyncRead` for streaming the content.
    #[inline]
    pub fn reader(&mut self) -> &mut object_store::Object {
        &mut self.reader
    }

    /// Consumes self and returns the reader.
    #[inline]
    pub fn into_reader(self) -> object_store::Object {
        self.reader
    }

    /// Returns the object metadata.
    #[inline]
    pub fn info(&self) -> &ObjectInfo {
        &self.info
    }

    /// Returns the object size in bytes.
    #[inline]
    pub fn size(&self) -> usize {
        self.info.size
    }

    /// Returns the object name/key.
    #[inline]
    pub fn name(&self) -> &str {
        &self.info.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_put_result_getters() {
        let result = PutResult::new(1024, vec![0u8; 32], "0".repeat(64), "test-nuid".to_string());

        assert_eq!(result.size(), 1024);
        assert_eq!(result.sha256().len(), 32);
        assert_eq!(result.sha256_hex().len(), 64);
        assert_eq!(result.nuid(), "test-nuid");
    }
}
