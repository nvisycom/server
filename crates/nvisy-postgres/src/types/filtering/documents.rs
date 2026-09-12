//! Filtering options for document queries.

/// Filter options for documents.
///
/// Each field narrows the result when set; unset fields impose no constraint.
/// The workspace scope is applied by the query itself, not carried here.
///
/// The format filter is expressed as a flat list of file extensions the caller
/// has already resolved (e.g. from format or modality keywords); this layer
/// matches them against the stored `file_extension` without knowing the format
/// taxonomy, which lives in the engine's codec registry.
#[derive(Debug, Default, Clone)]
pub struct DocumentFilter {
    /// Search by document name (case-insensitive, partial match). An empty string
    /// is resolved to `None` by the caller, so a set value is always a real
    /// search.
    pub search: Option<String>,
    /// Extension constraint. `None` imposes no constraint; `Some(set)` matches
    /// only these extensions — including `Some(empty)`, which matches nothing (an
    /// active facet resolved to an empty set).
    pub extensions: Option<Vec<String>>,
    /// Exact SHA-256 content hash (32 raw bytes). `None` imposes no constraint;
    /// `Some(hash)` matches only documents with this exact content. Lets a client
    /// check whether identical content already exists before uploading it.
    pub hash: Option<Vec<u8>>,
}
