//! Core types for Qdrant operations.
//!
//! This module provides strongly-typed interfaces for all Qdrant operations,
//! ensuring type safety and providing convenient constructors and utilities.

pub mod collection;
mod payload;
mod point;
mod vector;

pub use collection::{CollectionConfig, CollectionInfo, CollectionStatus};
pub use payload::Payload;
pub use point::{Point, PointId, PointVectors};
/// Re-export commonly used Qdrant client types for convenience
pub use qdrant_client::qdrant::{
    PointStruct as RawPoint, Value as PayloadValue, point_id::PointIdOptions,
    vectors::VectorsOptions,
};
pub use vector::{Distance, Vector, VectorParams};

/// Trait for types that can be converted to Qdrant point IDs
pub trait IntoPointId {
    /// Convert this type into a Qdrant point ID
    fn into_point_id(self) -> PointIdOptions;
}

/// Trait for types that can be converted to Qdrant vectors
pub trait IntoVector {
    /// Convert this type into a vector of f32 values
    fn into_vector(self) -> Vec<f32>;
}

/// Trait for types that can be used as payload values
pub trait IntoPayloadValue {
    /// Convert this type into a Qdrant payload value
    fn into_payload_value(self) -> PayloadValue;
}

impl IntoPointId for uuid::Uuid {
    fn into_point_id(self) -> PointIdOptions {
        PointIdOptions::Uuid(self.to_string())
    }
}

impl IntoPointId for u64 {
    fn into_point_id(self) -> PointIdOptions {
        PointIdOptions::Num(self)
    }
}

impl IntoPointId for String {
    fn into_point_id(self) -> PointIdOptions {
        PointIdOptions::Uuid(self)
    }
}

impl IntoPointId for &str {
    fn into_point_id(self) -> PointIdOptions {
        PointIdOptions::Uuid(self.to_string())
    }
}

impl IntoPointId for PointId {
    fn into_point_id(self) -> PointIdOptions {
        match self {
            PointId::Uuid(uuid) => PointIdOptions::Uuid(uuid),
            PointId::Num(num) => PointIdOptions::Num(num),
        }
    }
}

impl IntoVector for Vec<f32> {
    fn into_vector(self) -> Vec<f32> {
        self
    }
}

impl IntoVector for &[f32] {
    fn into_vector(self) -> Vec<f32> {
        self.to_vec()
    }
}

impl IntoVector for Vec<f64> {
    fn into_vector(self) -> Vec<f32> {
        self.into_iter().map(|v| v as f32).collect()
    }
}

impl IntoVector for &[f64] {
    fn into_vector(self) -> Vec<f32> {
        self.iter().map(|&v| v as f32).collect()
    }
}

impl IntoPayloadValue for String {
    fn into_payload_value(self) -> PayloadValue {
        PayloadValue {
            kind: Some(qdrant_client::qdrant::value::Kind::StringValue(self)),
        }
    }
}

impl IntoPayloadValue for &str {
    fn into_payload_value(self) -> PayloadValue {
        PayloadValue {
            kind: Some(qdrant_client::qdrant::value::Kind::StringValue(
                self.to_string(),
            )),
        }
    }
}

impl IntoPayloadValue for i64 {
    fn into_payload_value(self) -> PayloadValue {
        PayloadValue {
            kind: Some(qdrant_client::qdrant::value::Kind::IntegerValue(self)),
        }
    }
}

impl IntoPayloadValue for f64 {
    fn into_payload_value(self) -> PayloadValue {
        PayloadValue {
            kind: Some(qdrant_client::qdrant::value::Kind::DoubleValue(self)),
        }
    }
}

impl IntoPayloadValue for bool {
    fn into_payload_value(self) -> PayloadValue {
        PayloadValue {
            kind: Some(qdrant_client::qdrant::value::Kind::BoolValue(self)),
        }
    }
}

impl IntoPayloadValue for uuid::Uuid {
    fn into_payload_value(self) -> PayloadValue {
        PayloadValue {
            kind: Some(qdrant_client::qdrant::value::Kind::StringValue(
                self.to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    #[test]
    fn test_uuid_into_point_id() {
        let uuid = Uuid::new_v4();
        let point_id = uuid.into_point_id();
        match point_id {
            PointIdOptions::Uuid(s) => assert_eq!(s, uuid.to_string()),
            _ => panic!("Expected UUID point ID"),
        }
    }

    #[test]
    fn test_u64_into_point_id() {
        let id = 12345u64;
        let point_id = id.into_point_id();
        match point_id {
            PointIdOptions::Num(n) => assert_eq!(n, id),
            _ => panic!("Expected numeric point ID"),
        }
    }

    #[test]
    fn test_vec_f32_into_vector() {
        let vec = vec![1.0, 2.0, 3.0];
        let result = vec.clone().into_vector();
        assert_eq!(result, vec);
    }

    #[test]
    fn test_vec_f64_into_vector() {
        let vec = vec![1.0f64, 2.0f64, 3.0f64];
        let result = vec.into_vector();
        assert_eq!(result, vec![1.0f32, 2.0f32, 3.0f32]);
    }

    #[test]
    fn test_string_into_payload_value() {
        let s = "test".to_string();
        let value = s.clone().into_payload_value();
        match value.kind {
            Some(qdrant_client::qdrant::value::Kind::StringValue(v)) => assert_eq!(v, s),
            _ => panic!("Expected string payload value"),
        }
    }

    #[test]
    fn test_bool_into_payload_value() {
        let b = true;
        let value = b.into_payload_value();
        match value.kind {
            Some(qdrant_client::qdrant::value::Kind::BoolValue(v)) => assert_eq!(v, b),
            _ => panic!("Expected bool payload value"),
        }
    }
}
