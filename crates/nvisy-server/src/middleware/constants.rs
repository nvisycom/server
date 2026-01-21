//! Shared constants used across the server crate.

/// Default maximum request body size: 4MB.
///
/// Used for security middleware to limit incoming request body sizes
/// and prevent denial-of-service attacks via large payloads.
pub const DEFAULT_MAX_BODY_SIZE: usize = 4 * 1024 * 1024;

/// Maximum file size for uploads: 12MB.
///
/// Used in file upload handlers to enforce file size limits
/// before accepting file data into memory.
pub const DEFAULT_MAX_FILE_BODY_SIZE: usize = 12 * 1024 * 1024;
