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
}
