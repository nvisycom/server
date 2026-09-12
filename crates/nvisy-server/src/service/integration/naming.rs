//! File-name and remote-key helpers shared by the import and export paths.

use std::path::Path as StdPath;

use nvisy_postgres::model::WorkspaceDocument;

/// The final path segment of a key (its file name), or the whole key when it
/// contains no separator.
pub(super) fn object_basename(key: &str) -> String {
    StdPath::new(key)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(key)
        .to_string()
}

/// The lowercased extension of a key, if any.
pub(super) fn object_extension(key: &str) -> Option<String> {
    StdPath::new(key)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
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
