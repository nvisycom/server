//! Archive service for creating compressed archives.

use derive_more::{Deref, DerefMut};
use nvisy_rt_engine::{ArchiveRegistry, ArchiveType};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Supported archive formats.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema
)]
#[serde(rename_all = "lowercase")]
pub enum ArchiveFormat {
    /// ZIP archive format.
    Zip,
    /// TAR archive format (gzip compressed).
    Tar,
}

impl ArchiveFormat {
    /// Returns the file extension for this format.
    #[must_use]
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Zip => "zip",
            Self::Tar => "tar.gz",
        }
    }

    /// Returns the MIME type for this format.
    #[must_use]
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Zip => "application/zip",
            Self::Tar => "application/x-tar",
        }
    }

    /// Converts to the underlying [`ArchiveType`].
    #[must_use]
    pub fn to_archive_type(self) -> ArchiveType {
        match self {
            Self::Zip => ArchiveType::Zip,
            Self::Tar => ArchiveType::TarGz,
        }
    }
}

/// Error type for archive operations.
#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    /// Error from the archive library.
    #[error("Archive error: {0}")]
    Archive(#[from] nvisy_rt_engine::arc::Error),

    /// IO error during archive creation.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type for archive operations.
pub type ArchiveResult<T> = Result<T, ArchiveError>;

/// Service for creating compressed archives.
///
/// This service derefs to the underlying [`ArchiveRegistry`].
#[derive(Debug, Clone, Deref, DerefMut)]
pub struct ArchiveService {
    #[deref]
    #[deref_mut]
    registry: ArchiveRegistry,
}

impl ArchiveService {
    /// Creates a new archive service with default settings.
    ///
    /// # Panics
    ///
    /// Panics if the temp directory cannot be created.
    #[must_use]
    pub fn new() -> Self {
        Self {
            registry: ArchiveRegistry::new(std::env::temp_dir().join("nvisy-archive"))
                .expect("failed to create archive registry"),
        }
    }

    /// Creates an archive from a list of files.
    ///
    /// # Arguments
    ///
    /// * `files` - A list of (filename, content) tuples.
    /// * `format` - The archive format to create.
    ///
    /// # Errors
    ///
    /// Returns an error if archive creation fails.
    pub async fn create_archive(
        &self,
        files: Vec<(String, Vec<u8>)>,
        format: ArchiveFormat,
    ) -> ArchiveResult<Vec<u8>> {
        let archive_type = format.to_archive_type();

        // Create a handler for assembling files
        let mut handler = self.registry.create_archive_dir(archive_type)?;

        // Write all files to the directory
        for (filename, content) in files {
            handler.write_file(&filename, &content).await?;
        }

        // Pack into an archive and read the bytes
        let archive_name = format!("archive.{}", format.extension());
        let archive_file = handler.pack(&archive_name).await?;
        let archive_path = archive_file
            .path()
            .ok_or_else(|| ArchiveError::Io(std::io::Error::other("Archive has no path")))?;
        let archive_bytes = tokio::fs::read(archive_path).await?;

        Ok(archive_bytes)
    }
}

impl Default for ArchiveService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_archive_format_extension() {
        assert_eq!(ArchiveFormat::Zip.extension(), "zip");
        assert_eq!(ArchiveFormat::Tar.extension(), "tar.gz");
    }

    #[test]
    fn test_archive_format_mime_type() {
        assert_eq!(ArchiveFormat::Zip.mime_type(), "application/zip");
        assert_eq!(ArchiveFormat::Tar.mime_type(), "application/x-tar");
    }

    #[test]
    fn test_archive_format_to_archive_type() {
        assert_eq!(ArchiveFormat::Zip.to_archive_type(), ArchiveType::Zip);
        assert_eq!(ArchiveFormat::Tar.to_archive_type(), ArchiveType::TarGz);
    }

    #[tokio::test]
    async fn test_create_zip_archive() {
        let service = ArchiveService::new();
        let files = vec![
            ("test1.txt".to_string(), b"Hello".to_vec()),
            ("test2.txt".to_string(), b"World".to_vec()),
        ];

        let archive = service
            .create_archive(files, ArchiveFormat::Zip)
            .await
            .unwrap();
        assert!(!archive.is_empty());

        // Verify it's a valid ZIP (starts with PK signature)
        assert_eq!(&archive[0..2], b"PK");
    }

    #[tokio::test]
    async fn test_create_tar_archive() {
        let service = ArchiveService::new();
        let files = vec![
            ("test1.txt".to_string(), b"Hello".to_vec()),
            ("test2.txt".to_string(), b"World".to_vec()),
        ];

        let archive = service
            .create_archive(files, ArchiveFormat::Tar)
            .await
            .unwrap();
        assert!(!archive.is_empty());

        // Verify it's a valid gzip (starts with 0x1f 0x8b)
        assert_eq!(&archive[0..2], &[0x1f, 0x8b]);
    }
}
