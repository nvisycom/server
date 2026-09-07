//! File-name and remote-key helpers shared by the import and export paths.

use std::path::Path as StdPath;

use nvisy_postgres::model::WorkspaceFile;

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

/// The remote key an exported file is written under.
///
/// The base name is the file's display name. For an object store the key is
/// namespaced under `object_prefix` (e.g. `redacted/`) so exports never overwrite
/// imported originals, and under the file's id so two files that share a display
/// name do not overwrite each other at the same path. A file service creates a
/// new file from the base name directly, since it never overwrites.
pub(super) fn export_key(file: &WorkspaceFile, object_store: bool, object_prefix: &str) -> String {
    let name = object_basename(&file.display_name);
    if object_store {
        format!("{object_prefix}{}/{name}", file.id)
    } else {
        name
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
    use super::{object_basename, object_extension};

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
}
