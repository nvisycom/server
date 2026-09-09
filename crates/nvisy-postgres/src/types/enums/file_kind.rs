//! File kind enumeration: a file's role in the system.

use super::db_enum;

db_enum! {
    /// The role a file plays, which drives its data-retention scope and whether
    /// it is a user-facing document.
    ///
    /// Corresponds to the `FILE_KIND` PostgreSQL enum. Orthogonal to the
    /// `parent_id` version chain (lineage); import origin (connection and remote
    /// key) lives in the `workspace_file_imports` satellite.
    pub enum FileKind: Default = Original, "crate::schema::sql_types::FileKind" {
        /// Source document, uploaded or imported.
        Original = "original",
        /// Redacted output produced by a pipeline.
        Redacted = "redacted",
        /// Engine detection-analysis blob, not shown in file lists.
        Audit = "audit",
        /// Engine analysis after reviewer edits and redaction (a redaction's
        /// review audit), not shown in file lists.
        Review = "review",
        /// Enrichment content a detection extracted from a non-text document — an
        /// image's OCR layout, an audio clip's transcript — served to the client
        /// so a reviewer can search it and add entities the analysis missed.
        /// Carries document content, not shown in file lists.
        Intermediate = "intermediate",
    }
}

impl FileKind {
    /// The kinds that are user-facing documents (shown in file lists), as opposed
    /// to internal artifacts and audits.
    pub const DOCUMENTS: [FileKind; 2] = [FileKind::Original, FileKind::Redacted];
}
