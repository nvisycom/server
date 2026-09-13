//! File-name and remote-key helpers shared by the import and export paths.

use nvisy_postgres::model::WorkspaceDocument;

/// The final path segment of a key (its file name), or the whole key when it
/// contains no separator.
///
/// Object-store keys use `/` as their only delimiter (see
/// [`object_store::path::Path`]), so the split is on `/` alone rather than
/// [`std::path::Path`], whose host-specific rules would treat a backslash as a
/// separator on Windows and mis-split a key like `dir\name`.
pub(super) fn object_basename(key: &str) -> String {
    key.rsplit_once('/')
        .map_or(key, |(_, name)| name)
        .to_string()
}

/// The lowercased extension of a key (the part after the last `.` in its base
/// name), if any.
///
/// Splits on `/` for the base name, then on `.`, so parsing is identical on every
/// platform. A leading-dot name (`.env`) or a name with no dot has no extension.
pub(super) fn object_extension(key: &str) -> Option<String> {
    let name = object_basename(key);
    name.rsplit_once('.')
        .filter(|(stem, _)| !stem.is_empty())
        .map(|(_, ext)| ext.to_ascii_lowercase())
}

/// The remote key an exported document is written under.
///
/// The base name is the document's display name carrying the document's
/// extension, so an exported object is never left without its type. For an object
/// store the key is namespaced under `object_prefix` (e.g. `redacted/`) so exports
/// never overwrite imported originals, and under the document's id so two
/// documents that share a display name do not overwrite each other at the same
/// path. A file service creates a new file from the base name directly, since it
/// never overwrites.
pub(super) fn export_key(
    document: &WorkspaceDocument,
    object_store: bool,
    object_prefix: &str,
) -> String {
    let name = export_basename(&document.display_name, &document.file_extension);
    if object_store {
        format!("{object_prefix}{}/{name}", document.id)
    } else {
        name
    }
}

/// A display name with `extension` appended when the name does not already carry
/// it (case-insensitively), so an exported object is never left extensionless.
fn export_basename(display_name: &str, extension: &str) -> String {
    let name = object_basename(display_name);
    if extension.is_empty()
        || name
            .rsplit_once('.')
            .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case(extension))
    {
        name
    } else {
        format!("{name}.{extension}")
    }
}

/// Guesses the MIME type for a file extension, falling back to
/// `application/octet-stream` when unknown. Used to set the export Content-Type,
/// since files store only their extension, not a MIME type.
pub(super) fn mime_from_extension(extension: &str) -> String {
    mime_guess::from_ext(extension.trim_start_matches('.'))
        .first_or_octet_stream()
        .essence_str()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::{export_basename, object_basename, object_extension};

    #[test]
    fn basename_and_extension() {
        assert_eq!(object_basename("incoming/report.pdf"), "report.pdf");
        assert_eq!(
            object_extension("incoming/report.PDF").as_deref(),
            Some("pdf")
        );
        assert_eq!(object_basename("flat"), "flat");
        assert_eq!(object_extension("flat"), None);
        // A leading-dot name is all-extension-no-stem, i.e. no extension.
        assert_eq!(object_extension(".env"), None);
    }

    #[test]
    fn a_backslash_is_part_of_the_key_not_a_separator() {
        // Object-store keys delimit only on `/`. A backslash is an ordinary key
        // byte, so it must parse identically on every platform (on Windows,
        // std::path would wrongly split on it).
        // The backslash never splits the basename: `a\b` is one segment, so the
        // basename of `dir/a\b` is `a\b`, not `b`.
        assert_eq!(object_basename(r"dir/a\b"), r"a\b");
        assert_eq!(object_basename(r"a\b"), r"a\b");
        // Extension is still the part after the last `.`, and a backslash is an
        // ordinary character within it — parsed the same on every platform.
        assert_eq!(object_extension(r"a\b.txt").as_deref(), Some("txt"));
        assert_eq!(object_extension(r"a\b").as_deref(), None);
    }

    #[test]
    fn export_basename_carries_the_extension() {
        // A display name without the extension gains it.
        assert_eq!(export_basename("report", "pdf"), "report.pdf");
        // A name that already ends in the extension (any case) is left as-is.
        assert_eq!(export_basename("report.PDF", "pdf"), "report.PDF");
        // No extension to append leaves the name untouched.
        assert_eq!(export_basename("report", ""), "report");
    }
}
