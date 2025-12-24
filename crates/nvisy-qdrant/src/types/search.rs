//! Search-related types and utilities for Qdrant operations.

use serde::{Deserialize, Serialize};
#[cfg(feature = "schema")]
use schemars::JsonSchema;

use crate::error::{Error, Result};
use crate::types::{Payload, Point, PointId, PointVectors};

/// A search result containing a point and its similarity score.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct SearchResult {
    /// The point ID
    pub id: PointId,
    /// The point vectors
    pub vectors: PointVectors,
    /// The point payload
    pub payload: Payload,
    /// The similarity score (higher is more similar)
    pub score: f32,
}

impl SearchResult {
    /// Create a new search result from a point and score
    pub fn new(point: Point, score: f32) -> Self {
        Self {
            id: point.id,
            vectors: point.vectors,
            payload: point.payload,
            score,
        }
    }

    /// Create a search result from a point with a specific score
    pub fn from_point(point: Point, score: f32) -> Self {
        Self::new(point, score)
    }

    /// Convert to a Point
    pub fn to_point(self) -> Point {
        Point {
            id: self.id,
            vectors: self.vectors,
            payload: self.payload,
        }
    }

    /// Get a reference to the point ID
    pub fn id(&self) -> &PointId {
        &self.id
    }

    /// Get a reference to the point vectors
    pub fn vectors(&self) -> &PointVectors {
        &self.vectors
    }

    /// Get a reference to the point payload
    pub fn payload(&self) -> &Payload {
        &self.payload
    }

    /// Get the similarity score
    pub fn score(&self) -> f32 {
        self.score
    }

    /// Get the vector if this is a single vector result
    pub fn vector(&self) -> Option<crate::types::Vector> {
        match &self.vectors {
            PointVectors::Single(vector) => Some(vector.clone()),
            PointVectors::Named(_) => None,
        }
    }
}

impl TryFrom<qdrant_client::qdrant::ScoredPoint> for SearchResult {
    type Error = Error;

    fn try_from(scored_point: qdrant_client::qdrant::ScoredPoint) -> Result<Self> {
        // Handle the conversion from ScoredPoint which has VectorsOutput to Point which needs Vectors
        let id = match scored_point.id {
            Some(id) => crate::types::PointId::from_qdrant_point_id(id)?,
            None => return Err(Error::invalid_input().with_message("Missing point ID")),
        };

        // For now, create empty vectors since VectorsOutput conversion is complex
        // This is a placeholder implementation that needs to be improved
        let vectors = PointVectors::Single(crate::types::Vector::new(vec![]));

        let payload = Payload::from_qdrant_payload(scored_point.payload);

        let point = Point {
            id,
            vectors,
            payload,
        };

        Ok(SearchResult::new(point, scored_point.score))
    }
}

/// Type alias for Qdrant conditions for filtering
pub type Condition = qdrant_client::qdrant::Condition;

/// Type alias for Qdrant payload selectors
pub type WithPayloadSelector = qdrant_client::qdrant::WithPayloadSelector;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_result_creation() {
        let point = Point {
            id: PointId::text("test-id"),
            vectors: crate::types::PointVectors::Single(crate::types::Vector::new(vec![
                1.0, 2.0, 3.0,
            ])),
            payload: crate::types::Payload::new(),
        };
        let result = SearchResult::new(point.clone(), 0.95);

        assert_eq!(result.score(), 0.95);
        assert_eq!(result.id(), &PointId::text("test-id"));
    }
}
