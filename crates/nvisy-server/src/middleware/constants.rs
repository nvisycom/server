//! Shared constants used across the server crate.

/// Default maximum request body size: 4MB.
///
/// Used for security middleware to limit incoming request body sizes
/// and prevent denial-of-service attacks via large payloads.
pub const DEFAULT_MAX_BODY_SIZE: usize = 4 * 1024 * 1024;

/// Default server-wide hard cap for a file upload: 500 MiB.
///
/// This is the ceiling, not a per-request buffer — uploads stream through the
/// encrypt/hash readers straight into S3 multipart parts, so raising it costs no
/// extra memory (one part, ~8 MiB, is in flight at a time). A workspace may set a
/// lower soft cap via its settings; it can never exceed this.
pub const DEFAULT_MAX_FILE_BODY_SIZE: usize = 500 * 1024 * 1024;
