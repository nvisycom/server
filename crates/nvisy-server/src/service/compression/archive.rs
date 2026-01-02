//! File archival service.
//!
//! This module provides functionality for creating tar and zip archives.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::handler::{ErrorKind, Result};

/// Archive format options for file downloads.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ArchiveFormat {
    /// TAR archive format (with gzip compression)
    #[default]
    Tar,
    /// ZIP archive format
    Zip,
}

/// Service for creating file archives in various formats.
#[derive(Clone)]
pub struct ArchiveService;

impl ArchiveService {
    /// Create a new archive service instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Create an archive from a list of files.
    ///
    /// # Arguments
    /// * `files` - Vector of (filename, content) tuples to include in the archive
    /// * `format` - The archive format to create (tar or zip)
    ///
    /// # Returns
    /// The archive as a byte vector
    ///
    /// # Errors
    /// Returns an error if archive creation fails
    pub async fn create_archive(
        &self,
        files: Vec<(String, Vec<u8>)>,
        format: ArchiveFormat,
    ) -> Result<Vec<u8>> {
        // TODO: Implement archive creation
        // This should create either a tar.gz or zip archive containing all the files
        // For tar: use tar crate with flate2 for gzip compression
        // For zip: use zip crate
        let _ = (files, format);
        Err(ErrorKind::InternalServerError.with_message(
            "Archive creation not yet implemented: please implement tar/zip creation logic",
        ))
    }
}

impl Default for ArchiveService {
    fn default() -> Self {
        Self::new()
    }
}
