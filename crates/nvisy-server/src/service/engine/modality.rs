//! The server's document-modality buckets.
//!
//! The document-list filter (`?modality=text|tabular|image|audio`) groups
//! formats into four coarse, user-facing buckets. This grouping is a
//! presentation concern the server owns, not a fact read from the engine: a
//! format no longer has "a modality" upstream — a `.docx` decodes to a text body
//! plus a metadata part, a `.jpg` to image pixels plus an exif part, a `.pdf` is
//! a container of parts — so a part's modality is only knowable by decoding the
//! bytes. A list filter must not decode every file, so the server buckets by the
//! format's stable identifier instead.
//!
//! The bucket is keyed off the format's namespace segment
//! (`elide.<segment>.<name>`), which is stable; the matching extensions are read
//! from the engine's registry, so the server never reinvents elide's spellings.

/// The modality bucket a format id belongs to for the document-list filter, or
/// `None` for a format that is not user-filterable (metadata sidecars, test
/// fixtures).
///
/// `format_id` is an elide [`FormatId`] as a string (`elide.<segment>.<name>`,
/// e.g. `elide.text.txt`, `elide.document.pdf`). Document-shaped formats (`pdf`,
/// `docx`, `pptx`) bucket as `text` — they decode to a text body, which is what a
/// "text documents" filter means to a user. Metadata-only formats (`office`
/// docprops) and test fixtures are excluded.
///
/// [`FormatId`]: elide_pipeline::FormatRegistry
#[must_use]
pub fn format_modality(format_id: &str) -> Option<&'static str> {
    // `elide.<segment>.<name>`: the segment is the stable grouping key.
    match format_id.split('.').nth(1)? {
        "text" | "document" => Some("text"),
        "tabular" => Some("tabular"),
        "image" => Some("image"),
        "audio" => Some("audio"),
        // `office` (docprops) is metadata, `test` is fixtures: not user buckets.
        _ => None,
    }
}
