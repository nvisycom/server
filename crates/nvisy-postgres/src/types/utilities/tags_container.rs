//! Tags helper module for consistent tag handling across models with serialization support.

use serde::{Deserialize, Serialize};

/// A wrapper around a vector of optional strings that provides convenient methods
/// for working with tags throughout the application.
///
/// This type handles the common pattern of `Vec<Option<String>>` used in database
/// models and provides type-safe operations for tag manipulation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Tags(Vec<Option<String>>);

impl Tags {
    /// Creates a new empty `Tags` collection.
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Creates a new `Tags` collection from a vector of optional strings.
    pub fn from_optional_strings(tags: Vec<Option<String>>) -> Self {
        Self(tags)
    }

    /// Creates a new `Tags` collection from a vector of strings.
    pub fn from_strings<I, S>(tags: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self(tags.into_iter().map(|s| Some(s.into())).collect())
    }

    /// Returns the raw vector of optional strings.
    pub fn as_raw(&self) -> &Vec<Option<String>> {
        &self.0
    }

    /// Converts into the raw vector of optional strings.
    pub fn into_raw(self) -> Vec<Option<String>> {
        self.0
    }

    /// Returns a vector containing only the non-empty tag strings.
    pub fn as_strings(&self) -> Vec<String> {
        self.0.iter().filter_map(|tag| tag.clone()).collect()
    }

    /// Returns an iterator over the non-empty tag strings.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.0.iter().filter_map(|tag| tag.as_deref())
    }

    /// Returns whether the collection contains the specified tag.
    pub fn contains(&self, tag: &str) -> bool {
        self.0.iter().any(|t| t.as_deref() == Some(tag))
    }

    /// Adds a new tag to the collection if it doesn't already exist.
    /// Returns `true` if the tag was added, `false` if it already existed.
    pub fn add<S: Into<String>>(&mut self, tag: S) -> bool {
        let tag_string = tag.into();
        if !self.contains(&tag_string) {
            self.0.push(Some(tag_string));
            true
        } else {
            false
        }
    }

    /// Removes a tag from the collection.
    /// Returns `true` if the tag was found and removed, `false` otherwise.
    pub fn remove(&mut self, tag: &str) -> bool {
        let initial_len = self.0.len();
        self.0.retain(|t| t.as_deref() != Some(tag));
        self.0.len() != initial_len
    }

    /// Removes all empty/None tag entries from the collection.
    pub fn compact(&mut self) {
        self.0
            .retain(|tag| tag.as_deref().map_or(false, |s| !s.is_empty()));
    }

    /// Returns a new `Tags` collection with empty/None entries removed.
    pub fn compacted(&self) -> Self {
        let mut result = self.clone();
        result.compact();
        result
    }

    /// Returns the number of tags in the collection (including empty/None entries).
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns the number of non-empty tags in the collection.
    pub fn non_empty_len(&self) -> usize {
        self.0.iter().filter(|t| t.is_some()).count()
    }

    /// Returns whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns whether the collection has any non-empty tags.
    pub fn has_tags(&self) -> bool {
        self.0.iter().any(|t| t.is_some())
    }

    /// Clears all tags from the collection.
    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Extends the collection with tags from another collection.
    pub fn extend<I, S>(&mut self, other: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for tag in other {
            self.add(tag);
        }
    }

    /// Merges another `Tags` collection into this one, avoiding duplicates.
    pub fn merge(&mut self, other: &Tags) {
        for tag in other.iter() {
            self.add(tag);
        }
    }

    /// Returns the tags as a comma-separated string.
    pub fn join(&self, separator: &str) -> String {
        self.as_strings().join(separator)
    }

    /// Creates a `Tags` collection from a comma-separated string.
    pub fn from_comma_separated(input: &str) -> Self {
        if input.trim().is_empty() {
            return Self::new();
        }

        let tags: Vec<String> = input
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Self::from_strings(tags)
    }

    /// Returns whether all tags in this collection are also in the other collection.
    pub fn is_subset_of(&self, other: &Tags) -> bool {
        self.iter().all(|tag| other.contains(tag))
    }

    /// Returns whether this collection and another have any tags in common.
    pub fn has_intersection_with(&self, other: &Tags) -> bool {
        self.iter().any(|tag| other.contains(tag))
    }

    /// Returns a new `Tags` collection containing only tags that exist in both collections.
    pub fn intersection(&self, other: &Tags) -> Self {
        let intersection_tags: Vec<String> = self
            .iter()
            .filter(|tag| other.contains(tag))
            .map(|s| s.to_string())
            .collect();

        Self::from_strings(intersection_tags)
    }

    /// Returns a new `Tags` collection containing tags from both collections.
    pub fn union(&self, other: &Tags) -> Self {
        let mut result = self.clone();
        result.merge(other);
        result
    }

    /// Returns a new `Tags` collection containing tags from this collection
    /// that are not in the other collection.
    pub fn difference(&self, other: &Tags) -> Self {
        let diff_tags: Vec<String> = self
            .iter()
            .filter(|tag| !other.contains(tag))
            .map(|s| s.to_string())
            .collect();

        Self::from_strings(diff_tags)
    }
}

impl Default for Tags {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Vec<Option<String>>> for Tags {
    fn from(tags: Vec<Option<String>>) -> Self {
        Self(tags)
    }
}

impl From<Vec<String>> for Tags {
    fn from(tags: Vec<String>) -> Self {
        Self::from_strings(tags)
    }
}

impl From<Tags> for Vec<Option<String>> {
    fn from(tags: Tags) -> Self {
        tags.0
    }
}

impl From<Tags> for Vec<String> {
    fn from(tags: Tags) -> Self {
        tags.as_strings()
    }
}

impl<'a> IntoIterator for &'a Tags {
    type IntoIter = std::iter::FilterMap<
        std::slice::Iter<'a, Option<String>>,
        fn(&'a Option<String>) -> Option<&'a str>,
    >;
    type Item = &'a str;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().filter_map(|tag| tag.as_deref())
    }
}

impl FromIterator<String> for Tags {
    fn from_iter<I: IntoIterator<Item = String>>(iter: I) -> Self {
        Self::from_strings(iter)
    }
}

impl FromIterator<Option<String>> for Tags {
    fn from_iter<I: IntoIterator<Item = Option<String>>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_tags() {
        let tags = Tags::new();
        assert!(tags.is_empty());
        assert_eq!(tags.len(), 0);
        assert_eq!(tags.non_empty_len(), 0);
    }

    #[test]
    fn test_from_strings() {
        let tags = Tags::from_strings(vec!["rust", "programming"]);
        assert_eq!(tags.as_strings(), vec!["rust", "programming"]);
        assert_eq!(tags.len(), 2);
        assert_eq!(tags.non_empty_len(), 2);
    }

    #[test]
    fn test_contains() {
        let tags = Tags::from_strings(vec!["rust", "programming"]);
        assert!(tags.contains("rust"));
        assert!(tags.contains("programming"));
        assert!(!tags.contains("python"));
    }

    #[test]
    fn test_add_and_remove() {
        let mut tags = Tags::new();

        assert!(tags.add("rust"));
        assert!(!tags.add("rust")); // Duplicate should return false
        assert_eq!(tags.as_strings(), vec!["rust"]);

        assert!(tags.remove("rust"));
        assert!(!tags.remove("rust")); // Remove non-existent should return false
        assert!(tags.is_empty());
    }

    #[test]
    fn test_compact() {
        let mut tags = Tags::from_optional_strings(vec![
            Some("rust".to_string()),
            None,
            Some("".to_string()),
            Some("programming".to_string()),
        ]);

        tags.compact();
        assert_eq!(tags.as_strings(), vec!["rust", "programming"]);
    }

    #[test]
    fn test_from_comma_separated() {
        let tags = Tags::from_comma_separated("rust, programming, web");
        assert_eq!(tags.as_strings(), vec!["rust", "programming", "web"]);

        let empty_tags = Tags::from_comma_separated("  ");
        assert!(empty_tags.is_empty());
    }

    #[test]
    fn test_intersection() {
        let tags1 = Tags::from_strings(vec!["rust", "programming", "web"]);
        let tags2 = Tags::from_strings(vec!["rust", "backend", "programming"]);

        let intersection = tags1.intersection(&tags2);
        let mut result = intersection.as_strings();
        result.sort();
        assert_eq!(result, vec!["programming", "rust"]);
    }

    #[test]
    fn test_serialization() {
        let tags = Tags::from_strings(vec!["rust", "programming"]);
        let json = serde_json::to_string(&tags).unwrap();
        let deserialized: Tags = serde_json::from_str(&json).unwrap();
        assert_eq!(tags, deserialized);
    }
}
