//! Vector-related types and utilities for Qdrant operations.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

/// Vector distance metrics supported by Qdrant.
///
/// These metrics determine how vector similarity is calculated during search operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "PascalCase")]
pub enum Distance {
    /// Cosine distance - measures the cosine of the angle between two vectors.
    /// Best for normalized vectors and semantic similarity.
    Cosine,

    /// Euclidean distance - measures the straight-line distance between two points.
    /// Good for continuous features and when magnitude matters.
    Euclid,

    /// Dot product - measures the dot product between two vectors.
    /// Useful when vectors represent frequencies or counts.
    Dot,

    /// Manhattan distance - measures the sum of absolute differences.
    /// Robust to outliers and good for high-dimensional spaces.
    Manhattan,
}

impl Distance {
    /// Convert to Qdrant's internal distance enum
    pub fn to_qdrant_distance(self) -> qdrant_client::qdrant::Distance {
        match self {
            Distance::Cosine => qdrant_client::qdrant::Distance::Cosine,
            Distance::Euclid => qdrant_client::qdrant::Distance::Euclid,
            Distance::Dot => qdrant_client::qdrant::Distance::Dot,
            Distance::Manhattan => qdrant_client::qdrant::Distance::Manhattan,
        }
    }

    /// Create from Qdrant's internal distance enum
    pub fn from_qdrant_distance(distance: qdrant_client::qdrant::Distance) -> Self {
        match distance {
            qdrant_client::qdrant::Distance::Cosine => Distance::Cosine,
            qdrant_client::qdrant::Distance::Euclid => Distance::Euclid,
            qdrant_client::qdrant::Distance::Dot => Distance::Dot,
            qdrant_client::qdrant::Distance::Manhattan => Distance::Manhattan,
            qdrant_client::qdrant::Distance::UnknownDistance => Distance::Cosine, // Default fallback
        }
    }
}

impl Default for Distance {
    fn default() -> Self {
        Distance::Cosine
    }
}

/// Configuration parameters for vector fields in a collection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct VectorParams {
    /// The size (dimensionality) of the vectors
    pub size: u64,

    /// The distance metric to use for similarity calculations
    pub distance: Distance,

    /// Optional HNSW configuration parameters
    pub hnsw_config: Option<HnswConfig>,

    /// Optional quantization configuration
    pub quantization_config: Option<QuantizationConfig>,

    /// Whether to store vectors on disk (true) or in memory (false)
    pub on_disk: Option<bool>,
}

impl VectorParams {
    /// Create new vector parameters with the specified size and distance metric
    pub fn new(size: u64, distance: Distance) -> Self {
        Self {
            size,
            distance,
            hnsw_config: None,
            quantization_config: None,
            on_disk: None,
        }
    }

    /// Set HNSW configuration
    pub fn with_hnsw_config(mut self, config: HnswConfig) -> Self {
        self.hnsw_config = Some(config);
        self
    }

    /// Set quantization configuration
    pub fn with_quantization(mut self, config: QuantizationConfig) -> Self {
        self.quantization_config = Some(config);
        self
    }

    /// Set whether to store vectors on disk
    pub fn on_disk(mut self, on_disk: bool) -> Self {
        self.on_disk = Some(on_disk);
        self
    }

    /// Convert to Qdrant's internal VectorParams
    pub fn to_qdrant_vector_params(self) -> qdrant_client::qdrant::VectorParams {
        qdrant_client::qdrant::VectorParams {
            size: self.size,
            distance: self.distance.to_qdrant_distance().into(),
            hnsw_config: self.hnsw_config.map(|h| h.to_qdrant_hnsw_config()),
            quantization_config: self
                .quantization_config
                .map(|q| q.to_qdrant_quantization_config()),
            on_disk: self.on_disk,
            datatype: None,
            multivector_config: None,
        }
    }
}

/// HNSW (Hierarchical Navigable Small World) algorithm configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct HnswConfig {
    /// Number of connections each node will have
    pub m: Option<u64>,

    /// Size of the dynamic candidate list
    pub ef_construct: Option<u64>,

    /// Minimal size of the dynamic candidate list
    pub full_scan_threshold: Option<u64>,

    /// Number of parallel threads used for background index building
    pub max_indexing_threads: Option<u64>,

    /// Whether to store HNSW index on disk
    pub on_disk: Option<bool>,
}

impl HnswConfig {
    /// Create new HNSW configuration with default values
    pub fn new() -> Self {
        Self {
            m: None,
            ef_construct: None,
            full_scan_threshold: None,
            max_indexing_threads: None,
            on_disk: None,
        }
    }

    /// Set the number of connections (m parameter)
    pub fn with_m(mut self, m: u64) -> Self {
        self.m = Some(m);
        self
    }

    /// Set the ef_construct parameter
    pub fn with_ef_construct(mut self, ef_construct: u64) -> Self {
        self.ef_construct = Some(ef_construct);
        self
    }

    /// Set whether to store on disk
    pub fn on_disk(mut self, on_disk: bool) -> Self {
        self.on_disk = Some(on_disk);
        self
    }

    /// Convert to Qdrant's internal HnswConfigDiff
    pub fn to_qdrant_hnsw_config(self) -> qdrant_client::qdrant::HnswConfigDiff {
        qdrant_client::qdrant::HnswConfigDiff {
            m: self.m,
            ef_construct: self.ef_construct,
            full_scan_threshold: self.full_scan_threshold,
            max_indexing_threads: self.max_indexing_threads,
            on_disk: self.on_disk,
            payload_m: None,
            inline_storage: None,
        }
    }
}

impl Default for HnswConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Quantization configuration for reducing vector memory usage.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct QuantizationConfig {
    /// Scalar quantization configuration
    pub scalar: Option<ScalarQuantization>,

    /// Product quantization configuration
    pub product: Option<ProductQuantization>,

    /// Binary quantization configuration
    pub binary: Option<BinaryQuantization>,
}

impl QuantizationConfig {
    /// Create scalar quantization configuration
    pub fn scalar(config: ScalarQuantization) -> Self {
        Self {
            scalar: Some(config),
            product: None,
            binary: None,
        }
    }

    /// Create product quantization configuration
    pub fn product(config: ProductQuantization) -> Self {
        Self {
            scalar: None,
            product: Some(config),
            binary: None,
        }
    }

    /// Create binary quantization configuration
    pub fn binary(config: BinaryQuantization) -> Self {
        Self {
            scalar: None,
            product: None,
            binary: Some(config),
        }
    }

    /// Convert to Qdrant's internal QuantizationConfig
    pub fn to_qdrant_quantization_config(self) -> qdrant_client::qdrant::QuantizationConfig {
        qdrant_client::qdrant::QuantizationConfig {
            quantization: if let Some(scalar) = self.scalar {
                Some(
                    qdrant_client::qdrant::quantization_config::Quantization::Scalar(
                        scalar.to_qdrant_scalar_quantization(),
                    ),
                )
            } else if let Some(product) = self.product {
                Some(
                    qdrant_client::qdrant::quantization_config::Quantization::Product(
                        product.to_qdrant_product_quantization(),
                    ),
                )
            } else if let Some(binary) = self.binary {
                Some(
                    qdrant_client::qdrant::quantization_config::Quantization::Binary(
                        binary.to_qdrant_binary_quantization(),
                    ),
                )
            } else {
                None
            },
        }
    }
}

/// Scalar quantization configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct ScalarQuantization {
    /// Quantization type (int8 or uint8)
    pub r#type: ScalarType,

    /// Quantile for quantization bounds
    pub quantile: Option<f32>,

    /// Whether to always use RAM for quantized vectors
    pub always_ram: Option<bool>,
}

/// Scalar quantization types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub enum ScalarType {
    /// 8-bit signed integer
    Int8,
}

impl ScalarQuantization {
    /// Create new scalar quantization configuration
    pub fn new(r#type: ScalarType) -> Self {
        Self {
            r#type,
            quantile: None,
            always_ram: None,
        }
    }

    /// Convert to Qdrant's internal ScalarQuantization
    pub fn to_qdrant_scalar_quantization(self) -> qdrant_client::qdrant::ScalarQuantization {
        qdrant_client::qdrant::ScalarQuantization {
            r#type: match self.r#type {
                ScalarType::Int8 => qdrant_client::qdrant::QuantizationType::Int8.into(),
            },
            quantile: self.quantile,
            always_ram: self.always_ram,
        }
    }
}

/// Product quantization configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct ProductQuantization {
    /// Compression ratio
    pub compression: CompressionRatio,

    /// Whether to always use RAM for quantized vectors
    pub always_ram: Option<bool>,
}

/// Compression ratio for product quantization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub enum CompressionRatio {
    /// 4x compression
    X4,
    /// 8x compression
    X8,
    /// 16x compression
    X16,
    /// 32x compression
    X32,
    /// 64x compression
    X64,
}

impl ProductQuantization {
    /// Create new product quantization configuration
    pub fn new(compression: CompressionRatio) -> Self {
        Self {
            compression,
            always_ram: None,
        }
    }

    /// Convert to Qdrant's internal ProductQuantization
    pub fn to_qdrant_product_quantization(self) -> qdrant_client::qdrant::ProductQuantization {
        qdrant_client::qdrant::ProductQuantization {
            compression: match self.compression {
                CompressionRatio::X4 => qdrant_client::qdrant::CompressionRatio::X4 as i32,
                CompressionRatio::X8 => qdrant_client::qdrant::CompressionRatio::X8 as i32,
                CompressionRatio::X16 => qdrant_client::qdrant::CompressionRatio::X16 as i32,
                CompressionRatio::X32 => qdrant_client::qdrant::CompressionRatio::X32 as i32,
                CompressionRatio::X64 => qdrant_client::qdrant::CompressionRatio::X64 as i32,
            },
            always_ram: self.always_ram,
        }
    }
}

/// Binary quantization configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct BinaryQuantization {
    /// Whether to always use RAM for quantized vectors
    pub always_ram: Option<bool>,
}

impl BinaryQuantization {
    /// Create new binary quantization configuration
    pub fn new() -> Self {
        Self { always_ram: None }
    }

    /// Convert to Qdrant's internal BinaryQuantization
    pub fn to_qdrant_binary_quantization(self) -> qdrant_client::qdrant::BinaryQuantization {
        qdrant_client::qdrant::BinaryQuantization {
            always_ram: self.always_ram,
            encoding: None,
            query_encoding: None,
        }
    }
}

impl Default for BinaryQuantization {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents a vector with its values and metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct Vector {
    /// The vector values
    pub values: Vec<f32>,

    /// Optional vector name/identifier
    pub name: Option<String>,
}

impl Vector {
    /// Create a new vector from values
    pub fn new(values: Vec<f32>) -> Self {
        Self { values, name: None }
    }

    /// Create a named vector
    pub fn named(values: Vec<f32>, name: String) -> Self {
        Self {
            values,
            name: Some(name),
        }
    }

    /// Get the dimensionality of this vector
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Check if the vector is empty
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Normalize the vector (make it unit length)
    pub fn normalize(&mut self) {
        let magnitude = self.magnitude();
        if magnitude > 0.0 {
            for value in &mut self.values {
                *value /= magnitude;
            }
        }
    }

    /// Get the magnitude (length) of the vector
    pub fn magnitude(&self) -> f32 {
        self.values.iter().map(|x| x * x).sum::<f32>().sqrt()
    }

    /// Create a normalized copy of this vector
    pub fn normalized(&self) -> Self {
        let mut normalized = self.clone();
        normalized.normalize();
        normalized
    }

    /// Convert to Qdrant's internal Vector representation
    #[allow(deprecated)]
    pub fn to_qdrant_vector(self) -> qdrant_client::qdrant::Vector {
        qdrant_client::qdrant::Vector {
            vector: Some(qdrant_client::qdrant::vector::Vector::Dense(
                qdrant_client::qdrant::DenseVector {
                    data: self.values,
                    ..Default::default()
                },
            )),
            data: vec![],
            indices: None,
            vectors_count: None,
        }
    }
}

impl From<Vec<f32>> for Vector {
    fn from(values: Vec<f32>) -> Self {
        Self::new(values)
    }
}

impl From<&[f32]> for Vector {
    fn from(values: &[f32]) -> Self {
        Self::new(values.to_vec())
    }
}

impl From<Vec<f64>> for Vector {
    fn from(values: Vec<f64>) -> Self {
        Self::new(values.into_iter().map(|v| v as f32).collect())
    }
}

impl From<&[f64]> for Vector {
    fn from(values: &[f64]) -> Self {
        Self::new(values.iter().map(|&v| v as f32).collect())
    }
}

impl From<qdrant_client::qdrant::Vector> for Vector {
    fn from(vector: qdrant_client::qdrant::Vector) -> Self {
        match vector.vector {
            Some(qdrant_client::qdrant::vector::Vector::Dense(dense)) => Vector::new(dense.data),
            Some(qdrant_client::qdrant::vector::Vector::Sparse(sparse)) => {
                // For sparse vectors, we'll just use the indices as values for now
                // This is a simplified conversion
                Vector::new(sparse.indices.into_iter().map(|i| i as f32).collect())
            }
            Some(qdrant_client::qdrant::vector::Vector::MultiDense(_)) => {
                // Multi-dense vectors not supported, use empty vector
                Vector::new(vec![])
            }
            Some(qdrant_client::qdrant::vector::Vector::Document(_)) => {
                // Document vectors not supported, use empty vector
                Vector::new(vec![])
            }
            Some(qdrant_client::qdrant::vector::Vector::Image(_)) => {
                // Image vectors not supported, use empty vector
                Vector::new(vec![])
            }
            Some(qdrant_client::qdrant::vector::Vector::Object(_)) => {
                // Object vectors not supported, use empty vector
                Vector::new(vec![])
            }
            None => {
                // Fallback for empty vector if vector field is None
                Vector::new(vec![])
            }
        }
    }
}

impl Default for Vector {
    fn default() -> Self {
        Self {
            values: Vec::new(),
            name: None,
        }
    }
}

/// Named vectors configuration for collections that support multiple vector fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct NamedVectors {
    /// Map of vector names to their configurations
    pub vectors: HashMap<String, VectorParams>,
}

impl NamedVectors {
    /// Create new named vectors configuration
    pub fn new() -> Self {
        Self {
            vectors: HashMap::new(),
        }
    }

    /// Add a named vector field
    pub fn add_vector(mut self, name: String, params: VectorParams) -> Self {
        self.vectors.insert(name, params);
        self
    }

    /// Convert to Qdrant's internal VectorParamsMap
    pub fn to_qdrant_vector_params_map(
        self,
    ) -> HashMap<String, qdrant_client::qdrant::VectorParams> {
        self.vectors
            .into_iter()
            .map(|(name, params)| (name, params.to_qdrant_vector_params()))
            .collect()
    }
}

impl Default for NamedVectors {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_creation() {
        let values = vec![1.0, 2.0, 3.0];
        let vector = Vector::new(values.clone());
        assert_eq!(vector.values, values);
        assert_eq!(vector.len(), 3);
        assert!(!vector.is_empty());
    }

    #[test]
    fn test_vector_normalization() {
        let mut vector = Vector::new(vec![3.0, 4.0]);
        let magnitude = vector.magnitude();
        assert_eq!(magnitude, 5.0);

        vector.normalize();
        assert!((vector.magnitude() - 1.0).abs() < f32::EPSILON);
        assert!((vector.values[0] - 0.6).abs() < f32::EPSILON);
        assert!((vector.values[1] - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn test_distance_conversion() {
        assert_eq!(
            Distance::Cosine.to_qdrant_distance(),
            qdrant_client::qdrant::Distance::Cosine
        );
        assert_eq!(
            Distance::from_qdrant_distance(qdrant_client::qdrant::Distance::Euclid),
            Distance::Euclid
        );
    }

    #[test]
    fn test_vector_params() {
        let params = VectorParams::new(384, Distance::Cosine).on_disk(true);

        assert_eq!(params.size, 384);
        assert_eq!(params.distance, Distance::Cosine);
        assert_eq!(params.on_disk, Some(true));
    }

    #[test]
    fn test_from_f64_vector() {
        let f64_vec = vec![1.0f64, 2.0f64, 3.0f64];
        let vector = Vector::from(f64_vec);
        assert_eq!(vector.values, vec![1.0f32, 2.0f32, 3.0f32]);
    }
}
