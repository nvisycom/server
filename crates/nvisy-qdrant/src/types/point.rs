//! Point-related types for Qdrant operations.

use std::collections::HashMap;

use derive_more::{Display, From};
use qdrant_client::qdrant::vectors_output::VectorsOptions;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::{Payload, Vector};
use crate::error::{Error, Result};

/// Represents a point ID that can be a UUID, text string, or numeric ID.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[derive(Serialize, Deserialize, Display, From)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(untagged)]
pub enum PointId {
    /// UUID-based ID
    #[display("{_0}")]
    Uuid(#[from] Uuid),
    /// Text string ID
    #[display("{_0}")]
    Text(String),
    /// Numeric ID
    #[display("{_0}")]
    Num(u64),
}

impl PointId {
    /// Create a new UUID-based point ID
    pub fn uuid(id: Uuid) -> Self {
        Self::Uuid(id)
    }

    /// Create a new text-based point ID
    pub fn text(id: impl Into<String>) -> Self {
        Self::Text(id.into())
    }

    /// Create a new numeric point ID
    pub fn num(id: u64) -> Self {
        Self::Num(id)
    }

    /// Convert this point ID to Qdrant's internal PointId
    pub fn to_qdrant_point_id(self) -> qdrant_client::qdrant::PointId {
        qdrant_client::qdrant::PointId {
            point_id_options: Some(match self {
                PointId::Uuid(uuid) => {
                    qdrant_client::qdrant::point_id::PointIdOptions::Uuid(uuid.to_string())
                }
                PointId::Text(text) => qdrant_client::qdrant::point_id::PointIdOptions::Uuid(text),
                PointId::Num(num) => qdrant_client::qdrant::point_id::PointIdOptions::Num(num),
            }),
        }
    }

    /// Create from Qdrant's internal PointId
    pub fn from_qdrant_point_id(point_id: qdrant_client::qdrant::PointId) -> Result<Self> {
        match point_id.point_id_options {
            Some(qdrant_client::qdrant::point_id::PointIdOptions::Uuid(s)) => {
                // Try to parse as UUID first, otherwise treat as text
                if let Ok(uuid) = Uuid::parse_str(&s) {
                    Ok(PointId::Uuid(uuid))
                } else {
                    Ok(PointId::Text(s))
                }
            }
            Some(qdrant_client::qdrant::point_id::PointIdOptions::Num(num)) => {
                Ok(PointId::Num(num))
            }
            None => Err(Error::invalid_input().with_message("Missing point ID options")),
        }
    }
}

impl From<&str> for PointId {
    fn from(s: &str) -> Self {
        s.to_owned().into()
    }
}

/// Represents a point in Qdrant with an ID, vector data, and optional payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Point {
    /// The unique identifier for this point
    pub id: PointId,

    /// The vector data (single vector or named vectors)
    pub vectors: PointVectors,

    /// Optional payload data associated with this point
    #[serde(default)]
    pub payload: Payload,
}

impl Point {
    /// Create a new point with a single vector
    pub fn new(id: impl Into<PointId>, vector: impl Into<Vector>, payload: Payload) -> Self {
        Self {
            id: id.into(),
            vectors: PointVectors::Single(vector.into()),
            payload,
        }
    }

    /// Create a new point with named vectors
    pub fn with_named_vectors(
        id: impl Into<PointId>,
        vectors: HashMap<String, Vector>,
        payload: Payload,
    ) -> Self {
        Self {
            id: id.into(),
            vectors: PointVectors::Named(vectors),
            payload,
        }
    }

    /// Create a point with just an ID and single vector (empty payload)
    pub fn simple(id: impl Into<PointId>, vector: impl Into<Vector>) -> Self {
        Self::new(id, vector, Payload::new())
    }

    /// Get the point ID
    pub fn id(&self) -> &PointId {
        &self.id
    }

    /// Get a reference to the vectors
    pub fn vectors(&self) -> &PointVectors {
        &self.vectors
    }

    /// Get a mutable reference to the vectors
    pub fn vectors_mut(&mut self) -> &mut PointVectors {
        &mut self.vectors
    }

    /// Get a reference to the payload
    pub fn payload(&self) -> &Payload {
        &self.payload
    }

    /// Get a mutable reference to the payload
    pub fn payload_mut(&mut self) -> &mut Payload {
        &mut self.payload
    }

    /// Add or update a payload field
    pub fn set_payload(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.payload.insert(key, value);
        self
    }

    /// Convert to Qdrant's internal PointStruct
    pub fn to_qdrant_point_struct(self) -> Result<qdrant_client::qdrant::PointStruct> {
        let vectors = match self.vectors {
            PointVectors::Single(vector) => Some(
                qdrant_client::qdrant::vectors::VectorsOptions::Vector(vector.to_qdrant_vector()),
            ),
            PointVectors::Named(named_vectors) => {
                let mut vectors_map = HashMap::new();
                for (name, vector) in named_vectors {
                    vectors_map.insert(name, vector.to_qdrant_vector());
                }
                Some(qdrant_client::qdrant::vectors::VectorsOptions::Vectors(
                    qdrant_client::qdrant::NamedVectors {
                        vectors: vectors_map,
                    },
                ))
            }
        };

        Ok(qdrant_client::qdrant::PointStruct {
            id: Some(self.id.to_qdrant_point_id()),
            vectors: vectors.map(|v| qdrant_client::qdrant::Vectors {
                vectors_options: Some(v),
            }),
            payload: self.payload.into_qdrant_payload(),
        })
    }

    /// Convert to Qdrant's internal PointStruct (alias for compatibility)
    pub fn to_qdrant_point(self) -> Result<qdrant_client::qdrant::PointStruct> {
        self.to_qdrant_point_struct()
    }

    /// Create from Qdrant's internal PointStruct
    pub fn from_qdrant_point_struct(point: qdrant_client::qdrant::PointStruct) -> Result<Self> {
        let id = match point.id {
            Some(id) => PointId::from_qdrant_point_id(id)?,
            None => return Err(Error::invalid_input().with_message("Missing point ID")),
        };

        let vectors = match point.vectors.and_then(|v| v.vectors_options) {
            Some(qdrant_client::qdrant::vectors::VectorsOptions::Vector(vector)) => {
                PointVectors::Single(Vector::from(vector))
            }
            Some(qdrant_client::qdrant::vectors::VectorsOptions::Vectors(named_vectors)) => {
                let mut vectors_map = HashMap::new();
                for (name, vector) in named_vectors.vectors {
                    vectors_map.insert(name, Vector::from(vector));
                }
                PointVectors::Named(vectors_map)
            }
            None => return Err(Error::invalid_input().with_message("Missing vectors")),
        };

        let payload = Payload::from_qdrant_payload(point.payload);

        Ok(Self {
            id,
            vectors,
            payload,
        })
    }
}

/// Represents either a single vector or named vectors for a point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(untagged)]
pub enum PointVectors {
    /// A single vector
    Single(Vector),
    /// Named vectors (multiple vectors with names)
    Named(HashMap<String, Vector>),
}

impl PointVectors {
    /// Create from a single vector
    pub fn single(vector: impl Into<Vector>) -> Self {
        Self::Single(vector.into())
    }

    /// Create from named vectors
    pub fn named(vectors: HashMap<String, Vector>) -> Self {
        Self::Named(vectors)
    }

    /// Check if this contains a single vector
    pub fn is_single(&self) -> bool {
        matches!(self, Self::Single(_))
    }

    /// Check if this contains named vectors
    pub fn is_named(&self) -> bool {
        matches!(self, Self::Named(_))
    }

    /// Get the single vector, if this is a single vector
    pub fn as_single(&self) -> Option<&Vector> {
        match self {
            Self::Single(vector) => Some(vector),
            Self::Named(_) => None,
        }
    }

    /// Get the named vectors, if this contains named vectors
    pub fn as_named(&self) -> Option<&HashMap<String, Vector>> {
        match self {
            Self::Single(_) => None,
            Self::Named(vectors) => Some(vectors),
        }
    }

    /// Get a specific named vector
    pub fn get_named(&self, name: &str) -> Option<&Vector> {
        match self {
            Self::Single(_) => None,
            Self::Named(vectors) => vectors.get(name),
        }
    }
}

impl From<Vector> for PointVectors {
    fn from(vector: Vector) -> Self {
        Self::Single(vector)
    }
}

impl From<HashMap<String, Vector>> for PointVectors {
    fn from(vectors: HashMap<String, Vector>) -> Self {
        Self::Named(vectors)
    }
}

// Conversion implementations for Qdrant types
impl TryFrom<Point> for qdrant_client::qdrant::PointStruct {
    type Error = Error;

    fn try_from(point: Point) -> std::result::Result<Self, Self::Error> {
        point.to_qdrant_point_struct()
    }
}

impl TryFrom<qdrant_client::qdrant::PointStruct> for Point {
    type Error = Error;

    fn try_from(
        point: qdrant_client::qdrant::PointStruct,
    ) -> std::result::Result<Self, Self::Error> {
        Point::from_qdrant_point_struct(point)
    }
}

impl From<PointId> for qdrant_client::qdrant::PointId {
    fn from(point_id: PointId) -> Self {
        point_id.to_qdrant_point_id()
    }
}

impl From<PointId> for qdrant_client::qdrant::point_id::PointIdOptions {
    fn from(point_id: PointId) -> Self {
        match point_id {
            PointId::Uuid(uuid) => {
                qdrant_client::qdrant::point_id::PointIdOptions::Uuid(uuid.to_string())
            }
            PointId::Text(text) => qdrant_client::qdrant::point_id::PointIdOptions::Uuid(text),
            PointId::Num(num) => qdrant_client::qdrant::point_id::PointIdOptions::Num(num),
        }
    }
}

impl TryFrom<qdrant_client::qdrant::RetrievedPoint> for Point {
    type Error = Error;

    fn try_from(
        point: qdrant_client::qdrant::RetrievedPoint,
    ) -> std::result::Result<Self, Self::Error> {
        let id = match point.id {
            Some(id) => PointId::from_qdrant_point_id(id)?,
            None => return Err(Error::invalid_input().with_message("Missing point ID")),
        };

        let vectors = match point.vectors.and_then(|v| v.vectors_options) {
            Some(VectorsOptions::Vector(vector_output)) => {
                PointVectors::Single(Vector::from_vector_output(vector_output))
            }
            Some(VectorsOptions::Vectors(named_vectors)) => {
                let mut vectors_map = HashMap::new();
                for (name, vector_output) in named_vectors.vectors {
                    vectors_map.insert(name, Vector::from_vector_output(vector_output));
                }
                PointVectors::Named(vectors_map)
            }
            None => return Err(Error::invalid_input().with_message("Missing vectors")),
        };

        let payload = Payload::from_qdrant_payload(point.payload);

        Ok(Self {
            id,
            vectors,
            payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    #[test]
    fn test_point_id_creation() {
        let uuid = Uuid::new_v4();
        let uuid_id = PointId::from(uuid);
        assert!(matches!(uuid_id, PointId::Uuid(_)));

        let num_id = PointId::from(123u64);
        assert!(matches!(num_id, PointId::Num(123)));
    }

    #[test]
    fn test_point_id_display() {
        let uuid_id = PointId::text("test-uuid");
        assert_eq!(uuid_id.to_string(), "test-uuid");

        let num_id = PointId::num(123);
        assert_eq!(num_id.to_string(), "123");
    }

    #[test]
    fn test_point_creation() {
        let id = PointId::text("test-point");
        let vector = Vector::new(vec![1.0, 2.0, 3.0]);
        let payload = Payload::new().with("type", "test");

        let point = Point::new(id.clone(), vector.clone(), payload.clone());

        assert_eq!(point.id, id);
        assert!(point.vectors.is_single());
        assert_eq!(
            point.vectors.as_single().unwrap().values,
            vec![1.0, 2.0, 3.0]
        );
    }

    #[test]
    fn test_point_with_named_vectors() {
        let id = PointId::num(1);
        let mut vectors = HashMap::new();
        vectors.insert("text".to_string(), Vector::new(vec![0.1, 0.2]));
        vectors.insert("image".to_string(), Vector::new(vec![0.3, 0.4]));
        let payload = Payload::new();

        let point = Point::with_named_vectors(id.clone(), vectors.clone(), payload);

        assert_eq!(point.id, id);
        assert!(point.vectors.is_named());
        assert_eq!(point.vectors.as_named().unwrap(), &vectors);
    }

    #[test]
    fn test_point_with_payload() {
        let mut payload = Payload::new();
        payload.insert("category", "document");
        payload.insert("score", 0.95);

        let point = Point::new(
            PointId::text("test-id"),
            Vector::new(vec![1.0, 2.0, 3.0]),
            payload,
        );

        assert_eq!(point.id, PointId::text("test-id"));
        if let PointVectors::Single(ref vector) = point.vectors {
            assert_eq!(vector.values, vec![1.0, 2.0, 3.0]);
        } else {
            panic!("Expected single vector");
        }
        assert_eq!(
            point.payload.get("category").unwrap().as_str(),
            Some("document")
        );
    }

    #[test]
    fn test_point_named_vectors() {
        let mut vectors = std::collections::HashMap::new();
        vectors.insert("text".to_string(), Vector::new(vec![0.1, 0.2]));
        vectors.insert("image".to_string(), Vector::new(vec![0.3, 0.4]));

        let mut payload = Payload::new();
        payload.insert("type", "multimodal");

        let point = Point::with_named_vectors(PointId::num(42), vectors.clone(), payload);

        assert_eq!(point.id, PointId::num(42));
        if let PointVectors::Named(ref named) = point.vectors {
            assert_eq!(named.get("text").unwrap().values, vec![0.1, 0.2]);
            assert_eq!(named.get("image").unwrap().values, vec![0.3, 0.4]);
        } else {
            panic!("Expected named vectors");
        }
    }

    #[test]
    fn test_point_payload_access() {
        let mut payload = Payload::new();
        payload.insert("key", "value");

        let point = Point::new(
            PointId::text("test-id"),
            Vector::new(vec![1.0, 2.0]),
            payload,
        );

        assert_eq!(point.id, PointId::text("test-id"));
        assert_eq!(point.payload.get("key").unwrap().as_str(), Some("value"));
    }
}
