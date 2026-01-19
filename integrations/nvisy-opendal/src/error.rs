//! Storage error types.

/// Result type for storage operations.
pub type StorageResult<T> = Result<T, StorageError>;

/// Errors that can occur during storage operations.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// Failed to initialize the storage backend.
    #[error("storage initialization failed: {0}")]
    Init(String),

    /// File or object not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// Permission denied.
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// Read operation failed.
    #[error("read failed: {0}")]
    Read(String),

    /// Write operation failed.
    #[error("write failed: {0}")]
    Write(String),

    /// Delete operation failed.
    #[error("delete failed: {0}")]
    Delete(String),

    /// List operation failed.
    #[error("list failed: {0}")]
    List(String),

    /// Invalid path or URI.
    #[error("invalid path: {0}")]
    InvalidPath(String),

    /// Backend-specific error.
    #[error("backend error: {0}")]
    Backend(opendal::Error),
}

impl StorageError {
    /// Creates a new initialization error.
    pub fn init(msg: impl Into<String>) -> Self {
        Self::Init(msg.into())
    }

    /// Creates a new not found error.
    pub fn not_found(path: impl Into<String>) -> Self {
        Self::NotFound(path.into())
    }

    /// Creates a new permission denied error.
    pub fn permission_denied(msg: impl Into<String>) -> Self {
        Self::PermissionDenied(msg.into())
    }

    /// Creates a new read error.
    pub fn read(msg: impl Into<String>) -> Self {
        Self::Read(msg.into())
    }

    /// Creates a new write error.
    pub fn write(msg: impl Into<String>) -> Self {
        Self::Write(msg.into())
    }

    /// Creates a new delete error.
    pub fn delete(msg: impl Into<String>) -> Self {
        Self::Delete(msg.into())
    }

    /// Creates a new list error.
    pub fn list(msg: impl Into<String>) -> Self {
        Self::List(msg.into())
    }

    /// Creates a new invalid path error.
    pub fn invalid_path(msg: impl Into<String>) -> Self {
        Self::InvalidPath(msg.into())
    }
}

impl From<opendal::Error> for StorageError {
    fn from(err: opendal::Error) -> Self {
        use opendal::ErrorKind;

        match err.kind() {
            ErrorKind::NotFound => Self::NotFound(err.to_string()),
            ErrorKind::PermissionDenied => Self::PermissionDenied(err.to_string()),
            _ => Self::Backend(err),
        }
    }
}
