//! Thread-anchor response type: one location pin within a thread's file.

use jiff::Timestamp;
use nvisy_postgres::model::WorkspaceThreadAnchor as AnchorModel;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::handler::request::CommentAnchor;

/// Response type for a thread anchor: one location pin within the thread's file.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ThreadAnchor {
    /// Unique identifier of the anchor.
    pub id: Uuid,
    /// The modality-tagged location.
    pub anchor: CommentAnchor,
    /// When the anchor was added.
    pub created_at: Timestamp,
}

impl ThreadAnchor {
    /// Creates an anchor response, decoding its stored JSON into the typed
    /// anchor. Returns `None` for an anchor whose JSON no longer decodes (treated
    /// as absent rather than failing the read).
    pub fn from_model(anchor: AnchorModel) -> Option<Self> {
        let decoded = serde_json::from_value(anchor.anchor).ok()?;
        Some(Self {
            id: anchor.id,
            anchor: decoded,
            created_at: anchor.created_at.into(),
        })
    }
}
