//! Compression services for file archival.
//!
//! This module provides services for creating tar and zip archives.

mod archive;

pub use archive::{ArchiveFormat, ArchiveService};
