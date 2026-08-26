//! File kind enumeration: a file's role in the system.

use diesel_derive_enum::DbEnum;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

/// The role a file plays, which drives its data-retention scope and whether it
/// is a user-facing document.
///
/// Corresponds to the `FILE_KIND` PostgreSQL enum. Orthogonal to the `parent_id`
/// version chain (lineage); import origin (connection and remote key) lives in
/// the `workspace_file_imports` satellite.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize, DbEnum, Display, EnumIter, EnumString)]
#[ExistingTypePath = "crate::schema::sql_types::FileKind"]
pub enum FileKind {
    /// Source document, uploaded or imported.
    #[db_rename = "original"]
    #[serde(rename = "original")]
    #[default]
    Original,

    /// Redacted output produced by a pipeline.
    #[db_rename = "redacted"]
    #[serde(rename = "redacted")]
    Redacted,

    /// Engine detection-analysis blob, not shown in file lists.
    #[db_rename = "audit"]
    #[serde(rename = "audit")]
    Audit,

    /// Engine analysis after reviewer edits and redaction (a redaction's review
    /// audit), not shown in file lists.
    #[db_rename = "review"]
    #[serde(rename = "review")]
    Review,
}

impl FileKind {
    /// The kinds that are user-facing documents (shown in file lists), as
    /// opposed to internal artifacts and audits.
    pub const DOCUMENTS: [FileKind; 2] = [FileKind::Original, FileKind::Redacted];
}
