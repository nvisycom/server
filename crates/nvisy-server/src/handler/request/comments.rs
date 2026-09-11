//! Comment-thread request types (open a thread, post a comment, edit, filter).

use elide_pipeline::modality::audio::AudioLocation;
use elide_pipeline::modality::image::ImageLocation;
use elide_pipeline::modality::tabular::TabularLocation;
use elide_pipeline::modality::text::TextLocation;
use garde::Validate;
use nvisy_postgres::types::ThreadFilter;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::extract::validators::validate_non_blank;

/// Where a thread is pinned within a file: a location in one of the four
/// modalities, tagged so a single stored JSON value carries its own modality.
///
/// Each variant wraps the engine's own location type ([`elide_pipeline`]), so a
/// thread anchors to exactly what a detection/redaction does — a page region for
/// paginated/image documents, a time span for audio/video, a text span for
/// transcripts, a cell for tabular data. The engine's location types carry no
/// modality discriminator of their own, so the `modality` tag here supplies it.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "modality", rename_all = "snake_case")]
pub enum CommentAnchor {
    /// A span within text/transcript content.
    Text(TextLocation),
    /// A region on a page of image or paginated (PDF) content.
    Image(ImageLocation),
    /// A time span within audio/video content.
    Audio(AudioLocation),
    /// A cell (and optional intra-cell span) of tabular content.
    Tabular(TabularLocation),
}

/// Path parameters addressing one thread by its opaque id.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ThreadPathParams {
    /// Unique identifier of the thread.
    pub thread_id: Uuid,
}

/// Path parameters addressing one comment by its opaque id.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommentPathParams {
    /// Unique identifier of the comment.
    pub comment_id: Uuid,
}

/// Path parameters addressing one thread anchor by its opaque id.
#[must_use]
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ThreadAnchorPathParams {
    /// Unique identifier of the thread.
    pub thread_id: Uuid,
    /// Unique identifier of the anchor.
    pub anchor_id: Uuid,
}

/// Request payload to open a comment thread with its first message.
///
/// A thread pins a discussion to a location within a file (`anchor`), to a file
/// as a whole (no anchor), or — when opened on the workspace endpoint — to no
/// file at all. `@username` mentions in the opening body notify those members.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct OpenThread {
    /// Optional title for the thread (1-255 characters). Omit for an untitled
    /// thread.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(inner(length(chars, min = 1, max = 255), custom(validate_non_blank)))]
    pub display_name: Option<String>,
    /// The opening message text (1-10000 characters).
    #[garde(length(chars, min = 1, max = 10_000), custom(validate_non_blank))]
    pub body: String,
    /// Locations within the file the thread is pinned to. Empty for a file-level
    /// thread (no pin). Ignored for a workspace-level thread (no file).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[garde(length(max = 32))]
    pub anchors: Vec<CommentAnchor>,
}

/// Request payload to rename a thread (set or clear its title).
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct RenameThread {
    /// The new title (1-255 characters), or `null` to clear it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[garde(inner(length(chars, min = 1, max = 255), custom(validate_non_blank)))]
    pub display_name: Option<String>,
}

/// Request payload to add an anchor (location pin) to a thread.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct AddThreadAnchor {
    /// The location to pin.
    #[garde(skip)]
    pub anchor: CommentAnchor,
}

/// Request payload to post a comment (message) in a thread.
///
/// `@username` mentions in the body notify those workspace members.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct CreateComment {
    /// The comment text (1-10000 characters).
    #[garde(length(chars, min = 1, max = 10_000), custom(validate_non_blank))]
    pub body: String,
}

/// Request payload to edit a comment's body.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct UpdateComment {
    /// The new comment text (1-10000 characters).
    #[garde(length(chars, min = 1, max = 10_000), custom(validate_non_blank))]
    pub body: String,
}

/// Query parameters for listing a workspace's threads.
///
/// Every field is an optional filter; unset fields impose no constraint.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceThreadsQuery {
    /// Filter by the file the thread is pinned to.
    pub file_id: Option<Uuid>,
    /// Filter by the thread's opening author.
    pub author: Option<Uuid>,
    /// Filter by open/closed state: `true` = closed only, `false` = open only.
    pub closed: Option<bool>,
}

impl From<WorkspaceThreadsQuery> for ThreadFilter {
    fn from(query: WorkspaceThreadsQuery) -> Self {
        ThreadFilter {
            file_id: query.file_id,
            author_account_id: query.author,
            closed: query.closed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CommentAnchor;

    #[test]
    fn anchor_carries_its_modality_tag() {
        // An image anchor round-trips through JSON with a `modality` discriminator,
        // so a stored anchor is self-describing across modalities.
        let json = serde_json::json!({
            "modality": "image",
            "bounding_box": { "min": { "x": 1.0, "y": 2.0 }, "max": { "x": 3.0, "y": 4.0 } },
            "page": 2
        });
        let anchor: CommentAnchor =
            serde_json::from_value(json.clone()).expect("image anchor decodes");
        assert!(matches!(anchor, CommentAnchor::Image(_)));
        // Re-encoding keeps the modality tag.
        let reencoded = serde_json::to_value(&anchor).expect("encodes");
        assert_eq!(reencoded["modality"], "image");
    }

    #[test]
    fn anchor_modality_selects_the_variant() {
        let text = serde_json::json!({
            "modality": "text",
            "coord": { "kind": "decoded", "range": { "start": 0, "end": 5 }, "source": [] }
        });
        assert!(matches!(
            serde_json::from_value::<CommentAnchor>(text),
            Ok(CommentAnchor::Text(_))
        ));
    }
}
