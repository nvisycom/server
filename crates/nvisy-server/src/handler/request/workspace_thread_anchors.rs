//! Thread-anchor request types (the modality-tagged location and the add/address
//! payloads).

use elide_pipeline::modality::audio::AudioLocation;
use elide_pipeline::modality::image::ImageLocation;
use elide_pipeline::modality::tabular::TabularLocation;
use elide_pipeline::modality::text::TextLocation;
use garde::Validate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

/// Request payload to add an anchor (location pin) to a thread.
#[must_use]
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct AddThreadAnchor {
    /// The location to pin.
    #[garde(skip)]
    pub anchor: CommentAnchor,
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
