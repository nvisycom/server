//! Filtering options for document file queries.

/// Filter options for document files.
///
/// The format filter is expressed as a flat list of file extensions the caller
/// has already resolved (e.g. from format or modality keywords); this layer
/// matches them against the stored `file_extension` without knowing the format
/// taxonomy, which lives in the engine's codec registry.
#[derive(Debug, Default, Clone)]
pub struct FileFilter {
    /// Search by file name (case-insensitive, partial match).
    search: Option<String>,
    /// Extension constraint. `None` imposes no constraint; `Some(set)` matches
    /// only these extensions — including `Some(empty)`, which matches nothing
    /// (an active facet resolved to an empty set).
    extensions: Option<Vec<String>>,
    /// Exact SHA-256 content hash (32 raw bytes). `None` imposes no constraint;
    /// `Some(hash)` matches only files with this exact content. Lets a client
    /// check whether identical content already exists before uploading it.
    hash: Option<Vec<u8>>,
}

impl FileFilter {
    /// Creates a new empty filter.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Filters by search term.
    #[inline]
    pub fn with_search(mut self, search: String) -> Self {
        self.search = Some(search);
        self
    }

    /// Constrains to an explicit set of file extensions.
    ///
    /// An empty set is a real constraint that matches nothing, distinct from no
    /// extension filter at all.
    #[inline]
    pub fn with_extensions(mut self, extensions: Vec<String>) -> Self {
        self.extensions = Some(extensions);
        self
    }

    /// Returns whether a search filter is active.
    #[inline]
    pub fn has_search(&self) -> bool {
        self.search.as_ref().is_some_and(|s| !s.is_empty())
    }

    /// Returns the search term for trigram search.
    #[inline]
    pub fn search_term(&self) -> Option<&str> {
        self.search
            .as_ref()
            .filter(|s| !s.is_empty())
            .map(|s| s.as_str())
    }

    /// Returns the extension constraint, if one is set.
    ///
    /// `Some(set)` (even empty) means "match only these"; `None` means no
    /// extension constraint.
    #[inline]
    pub fn extensions(&self) -> Option<&[String]> {
        self.extensions.as_deref()
    }

    /// Constrains to files whose content hash is exactly `hash` (32 raw bytes).
    #[inline]
    pub fn with_hash(mut self, hash: Vec<u8>) -> Self {
        self.hash = Some(hash);
        self
    }

    /// Returns the content-hash constraint, if one is set.
    #[inline]
    pub fn hash(&self) -> Option<&[u8]> {
        self.hash.as_deref()
    }
}
