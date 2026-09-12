//! Document kind enumeration: a human-facing document's role.

use super::db_enum;

db_enum! {
    /// The kind of a human-facing document.
    ///
    /// Corresponds to the `DOCUMENT_KIND` PostgreSQL enum. Machine byproducts
    /// (detection audits, review audits, enrichment intermediates) are not
    /// documents — they reference blobs directly from their own tables.
    pub enum DocumentKind: Default = Original, "crate::schema::sql_types::DocumentKind" {
        /// Source document, uploaded or imported.
        Original = "original",
        /// Redacted output produced by a redaction.
        Redacted = "redacted",
    }
}
