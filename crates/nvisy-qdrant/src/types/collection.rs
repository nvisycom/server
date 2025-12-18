//! Collection-related types and configuration for Qdrant operations.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

use super::{Distance, VectorParams};
use crate::types::vector::NamedVectors;

/// Configuration for creating a new Qdrant collection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct CollectionConfig {
    /// The name of the collection
    pub name: String,

    /// Vector configuration (either single or named vectors)
    pub vectors_config: VectorsConfig,

    /// Number of shards in the collection
    pub shard_number: Option<u32>,

    /// Number of replicas for each shard
    pub replication_factor: Option<u32>,

    /// Maximum number of segments per shard
    pub write_consistency_factor: Option<u32>,

    /// Whether to store payload data on disk
    pub on_disk_payload: Option<bool>,

    /// HNSW configuration for the collection
    pub hnsw_config: Option<HnswCollectionConfig>,

    /// WAL (Write-Ahead Log) configuration
    pub wal_config: Option<WalConfig>,

    /// Optimizers configuration
    pub optimizers_config: Option<OptimizersConfig>,

    /// Collection initialization timeout in seconds
    pub timeout: Option<u64>,

    /// Initial capacity hint for the collection
    pub init_from: Option<InitFrom>,
}

impl CollectionConfig {
    /// Create a new collection configuration with the given name and single vector parameters
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            vectors_config: VectorsConfig::Single(VectorParams::new(384, Distance::Cosine)),
            shard_number: None,
            replication_factor: None,
            write_consistency_factor: None,
            on_disk_payload: None,
            hnsw_config: None,
            wal_config: None,
            optimizers_config: None,
            timeout: None,
            init_from: None,
        }
    }

    /// Set single vector parameters
    pub fn vectors(mut self, params: VectorParams) -> Self {
        self.vectors_config = VectorsConfig::Single(params);
        self
    }

    /// Set named vectors configuration
    pub fn named_vectors(mut self, vectors: NamedVectors) -> Self {
        self.vectors_config = VectorsConfig::Named(vectors);
        self
    }

    /// Set the number of shards
    pub fn shard_number(mut self, shard_number: u32) -> Self {
        self.shard_number = Some(shard_number);
        self
    }

    /// Set the replication factor
    pub fn replication_factor(mut self, replication_factor: u32) -> Self {
        self.replication_factor = Some(replication_factor);
        self
    }

    /// Set the write consistency factor
    pub fn write_consistency_factor(mut self, write_consistency_factor: u32) -> Self {
        self.write_consistency_factor = Some(write_consistency_factor);
        self
    }

    /// Set whether to store payload on disk
    pub fn on_disk_payload(mut self, on_disk: bool) -> Self {
        self.on_disk_payload = Some(on_disk);
        self
    }

    /// Set HNSW configuration
    pub fn hnsw_config(mut self, config: HnswCollectionConfig) -> Self {
        self.hnsw_config = Some(config);
        self
    }

    /// Set WAL configuration
    pub fn wal_config(mut self, config: WalConfig) -> Self {
        self.wal_config = Some(config);
        self
    }

    /// Set optimizers configuration
    pub fn optimizers_config(mut self, config: OptimizersConfig) -> Self {
        self.optimizers_config = Some(config);
        self
    }

    /// Set collection initialization timeout
    pub fn timeout(mut self, timeout_seconds: u64) -> Self {
        self.timeout = Some(timeout_seconds);
        self
    }

    /// Initialize collection from another collection or snapshot
    pub fn init_from(mut self, init_from: InitFrom) -> Self {
        self.init_from = Some(init_from);
        self
    }

    /// Convert to Qdrant's internal CreateCollection request
    pub fn to_qdrant_create_collection(self) -> qdrant_client::qdrant::CreateCollection {
        use qdrant_client::qdrant::vectors_config::Config;
        use qdrant_client::qdrant::{CreateCollection, VectorsConfig as QdrantVectorsConfig};

        let vectors_config = match self.vectors_config {
            VectorsConfig::Single(params) => Some(Config::Params(params.to_qdrant_vector_params())),
            VectorsConfig::Named(named) => {
                Some(Config::ParamsMap(qdrant_client::qdrant::VectorParamsMap {
                    map: named.to_qdrant_vector_params_map(),
                }))
            }
        };

        CreateCollection {
            collection_name: self.name,
            vectors_config: Some(QdrantVectorsConfig {
                config: vectors_config,
            }),
            shard_number: self.shard_number,
            replication_factor: self.replication_factor,
            write_consistency_factor: self.write_consistency_factor,
            on_disk_payload: self.on_disk_payload,
            hnsw_config: self.hnsw_config.map(|c| c.to_qdrant_hnsw_config()),
            wal_config: self.wal_config.map(|c| c.to_qdrant_wal_config()),
            optimizers_config: self
                .optimizers_config
                .map(|c| c.to_qdrant_optimizers_config()),
            timeout: self.timeout,
            metadata: std::collections::HashMap::new(),
            quantization_config: None,
            sharding_method: None,
            strict_mode_config: None,
            sparse_vectors_config: None,
        }
    }
}

/// Vector configuration for a collection (single or named vectors).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(untagged)]
pub enum VectorsConfig {
    /// Single vector configuration
    Single(VectorParams),
    /// Named vectors configuration
    Named(NamedVectors),
}

/// HNSW configuration for collections.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct HnswCollectionConfig {
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

    /// Payload M parameter for additional payload-based connections
    pub payload_m: Option<u64>,
}

impl HnswCollectionConfig {
    /// Create new HNSW collection configuration with default values
    pub fn new() -> Self {
        Self {
            m: None,
            ef_construct: None,
            full_scan_threshold: None,
            max_indexing_threads: None,
            on_disk: None,
            payload_m: None,
        }
    }

    /// Convert to Qdrant's internal HnswConfigDiff
    pub fn to_qdrant_hnsw_config(self) -> qdrant_client::qdrant::HnswConfigDiff {
        qdrant_client::qdrant::HnswConfigDiff {
            m: self.m,
            ef_construct: self.ef_construct,
            full_scan_threshold: self.full_scan_threshold,
            max_indexing_threads: self.max_indexing_threads,
            on_disk: self.on_disk,
            payload_m: self.payload_m,
            inline_storage: None,
        }
    }
}

impl Default for HnswCollectionConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Write-Ahead Log configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct WalConfig {
    /// WAL capacity threshold
    pub wal_capacity_mb: Option<u64>,

    /// WAL segments ahead
    pub wal_segments_ahead: Option<u64>,
}

impl WalConfig {
    /// Create new WAL configuration
    pub fn new() -> Self {
        Self {
            wal_capacity_mb: None,
            wal_segments_ahead: None,
        }
    }

    /// Set WAL capacity in MB
    pub fn capacity_mb(mut self, capacity: u64) -> Self {
        self.wal_capacity_mb = Some(capacity);
        self
    }

    /// Set WAL segments ahead
    pub fn segments_ahead(mut self, segments: u64) -> Self {
        self.wal_segments_ahead = Some(segments);
        self
    }

    /// Convert to Qdrant's internal WalConfigDiff
    pub fn to_qdrant_wal_config(self) -> qdrant_client::qdrant::WalConfigDiff {
        qdrant_client::qdrant::WalConfigDiff {
            wal_capacity_mb: self.wal_capacity_mb,
            wal_segments_ahead: self.wal_segments_ahead,
            wal_retain_closed: None,
        }
    }
}

impl Default for WalConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Optimizers configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct OptimizersConfig {
    /// The minimal fraction of deleted vectors in a segment required to perform segment optimization
    pub deleted_threshold: Option<f64>,

    /// The minimal number of vectors in a segment required to perform segment optimization
    pub vacuum_min_vector_number: Option<u64>,

    /// Target amount of segments the optimizer will try to keep
    pub default_segment_number: Option<u64>,

    /// Do not create segments larger this size (in KB)
    pub max_segment_size: Option<u64>,

    /// Maximum size (in KB) of vectors to store in-memory per segment
    pub memmap_threshold: Option<u64>,

    /// Maximum size (in KB) of vectors allowed for plain index
    pub indexing_threshold: Option<u64>,

    /// Interval between forced flushes
    pub flush_interval_sec: Option<u64>,

    /// Max number of threads for optimization
    pub max_optimization_threads: Option<u64>,
}

impl OptimizersConfig {
    /// Create new optimizers configuration
    pub fn new() -> Self {
        Self {
            deleted_threshold: None,
            vacuum_min_vector_number: None,
            default_segment_number: None,
            max_segment_size: None,
            memmap_threshold: None,
            indexing_threshold: None,
            flush_interval_sec: None,
            max_optimization_threads: None,
        }
    }

    /// Convert to Qdrant's internal OptimizersConfigDiff
    pub fn to_qdrant_optimizers_config(self) -> qdrant_client::qdrant::OptimizersConfigDiff {
        qdrant_client::qdrant::OptimizersConfigDiff {
            deleted_threshold: self.deleted_threshold,
            vacuum_min_vector_number: self.vacuum_min_vector_number,
            default_segment_number: self.default_segment_number,
            max_segment_size: self.max_segment_size,
            memmap_threshold: self.memmap_threshold,
            indexing_threshold: self.indexing_threshold,
            flush_interval_sec: self.flush_interval_sec,
            max_optimization_threads: None,
            deprecated_max_optimization_threads: None,
        }
    }
}

impl Default for OptimizersConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Collection initialization source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct InitFrom {
    /// Name of the collection to initialize from
    pub collection: String,
}

impl InitFrom {
    /// Create new initialization source
    pub fn collection(name: impl Into<String>) -> Self {
        Self {
            collection: name.into(),
        }
    }

    /// Convert to Qdrant's internal InitFrom
    pub fn to_qdrant_init_from(self) -> String {
        self.collection
    }
}

/// Information about an existing collection.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct CollectionInfo {
    /// The collection name
    pub name: String,

    /// Current status of the collection
    pub status: CollectionStatus,

    /// Optimizer status
    pub optimizer_status: OptimizerStatus,

    /// Vector configuration
    pub vectors_count: Option<u64>,

    /// Indexed vectors count
    pub indexed_vectors_count: Option<u64>,

    /// Points count in the collection
    pub points_count: Option<u64>,

    /// Collection configuration parameters
    pub config: CollectionConfigInfo,

    /// Payload schema information
    pub payload_schema: HashMap<String, PayloadSchemaInfo>,
}

impl CollectionInfo {
    /// Check if the collection is ready for operations
    pub fn is_ready(&self) -> bool {
        matches!(self.status, CollectionStatus::Green)
    }

    /// Check if the collection is being optimized
    pub fn is_optimizing(&self) -> bool {
        matches!(self.optimizer_status, OptimizerStatus::Ok)
    }

    /// Get the collection utilization ratio (indexed/total vectors)
    pub fn indexing_ratio(&self) -> Option<f64> {
        match (self.indexed_vectors_count, self.vectors_count) {
            (Some(indexed), Some(total)) if total > 0 => Some(indexed as f64 / total as f64),
            _ => None,
        }
    }
}

/// Status of a collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "PascalCase")]
pub enum CollectionStatus {
    /// Collection is ready for operations
    Green,
    /// Collection is partially available
    Yellow,
    /// Collection is unavailable
    Red,
}

impl CollectionStatus {
    /// Convert from Qdrant's internal CollectionStatus
    pub fn from_qdrant_status(status: i32) -> Self {
        match status {
            1 => CollectionStatus::Green,
            2 => CollectionStatus::Yellow,
            _ => CollectionStatus::Red,
        }
    }
}

/// Optimizer status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "PascalCase")]
pub enum OptimizerStatus {
    /// Optimizer is working normally
    Ok,
    /// Optimizer has errors
    Error,
}

impl OptimizerStatus {
    /// Convert from Qdrant's internal OptimizerStatus
    pub fn from_qdrant_status(status: i32) -> Self {
        match status {
            1 => OptimizerStatus::Ok,
            _ => OptimizerStatus::Error,
        }
    }
}

/// Collection configuration information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct CollectionConfigInfo {
    /// Vector parameters
    pub params: VectorsConfig,

    /// HNSW configuration
    pub hnsw_config: Option<HnswCollectionConfig>,

    /// Optimizer configuration
    pub optimizer_config: Option<OptimizersConfig>,

    /// WAL configuration
    pub wal_config: Option<WalConfig>,
}

/// Payload schema information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct PayloadSchemaInfo {
    /// Data type of the payload field
    pub data_type: PayloadSchemaType,

    /// Points count with this payload field
    pub points: Option<u64>,
}

/// Payload field data types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum PayloadSchemaType {
    /// String/keyword field
    Keyword,
    /// Integer field
    Integer,
    /// Float field
    Float,
    /// Geographical point
    Geo,
    /// Text field (for full-text search)
    Text,
    /// Boolean field
    Bool,
    /// Date/time field
    Datetime,
}

/// Builder for creating collection configurations with a fluent API.
#[derive(Debug, Clone)]
pub struct CollectionConfigBuilder {
    config: CollectionConfig,
}

impl CollectionConfigBuilder {
    /// Create a new collection configuration builder
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            config: CollectionConfig::new(name),
        }
    }

    /// Set single vector parameters
    pub fn vectors(mut self, params: VectorParams) -> Self {
        self.config = self.config.vectors(params);
        self
    }

    /// Set named vectors configuration
    pub fn named_vectors(mut self, vectors: NamedVectors) -> Self {
        self.config = self.config.named_vectors(vectors);
        self
    }

    /// Set the number of shards
    pub fn shard_number(mut self, shard_number: u32) -> Self {
        self.config = self.config.shard_number(shard_number);
        self
    }

    /// Set the replication factor
    pub fn replication_factor(mut self, replication_factor: u32) -> Self {
        self.config = self.config.replication_factor(replication_factor);
        self
    }

    /// Set whether to store payload on disk
    pub fn on_disk_payload(mut self, on_disk: bool) -> Self {
        self.config = self.config.on_disk_payload(on_disk);
        self
    }

    /// Set HNSW configuration
    pub fn hnsw_config(mut self, config: HnswCollectionConfig) -> Self {
        self.config = self.config.hnsw_config(config);
        self
    }

    /// Build the collection configuration
    pub fn build(self) -> CollectionConfig {
        self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collection_config_creation() {
        let config = CollectionConfig::new("test_collection");
        assert_eq!(config.name, "test_collection");
        assert!(matches!(config.vectors_config, VectorsConfig::Single(_)));
    }

    #[test]
    fn test_collection_config_builder() {
        let config = CollectionConfigBuilder::new("my_collection")
            .vectors(VectorParams::new(512, Distance::Euclid))
            .shard_number(2)
            .replication_factor(1)
            .on_disk_payload(true)
            .build();

        assert_eq!(config.name, "my_collection");
        assert_eq!(config.shard_number, Some(2));
        assert_eq!(config.replication_factor, Some(1));
        assert_eq!(config.on_disk_payload, Some(true));

        if let VectorsConfig::Single(params) = config.vectors_config {
            assert_eq!(params.size, 512);
            assert_eq!(params.distance, Distance::Euclid);
        } else {
            panic!("Expected single vector config");
        }
    }

    #[test]
    fn test_collection_config_fluent_api() {
        let config = CollectionConfig::new("test")
            .vectors(VectorParams::new(384, Distance::Cosine))
            .replication_factor(2)
            .on_disk_payload(false);

        assert_eq!(config.name, "test");
        assert_eq!(config.replication_factor, Some(2));
        assert_eq!(config.on_disk_payload, Some(false));
    }

    #[test]
    fn test_hnsw_config() {
        let hnsw = HnswCollectionConfig::new();
        assert_eq!(hnsw.m, None);
        assert_eq!(hnsw.ef_construct, None);
    }

    #[test]
    fn test_wal_config() {
        let wal = WalConfig::new().capacity_mb(256).segments_ahead(3);
        assert_eq!(wal.wal_capacity_mb, Some(256));
        assert_eq!(wal.wal_segments_ahead, Some(3));
    }

    #[test]
    fn test_collection_status() {
        assert_eq!(
            CollectionStatus::from_qdrant_status(1),
            CollectionStatus::Green
        );
        assert_eq!(
            CollectionStatus::from_qdrant_status(2),
            CollectionStatus::Yellow
        );
        assert_eq!(
            CollectionStatus::from_qdrant_status(0),
            CollectionStatus::Red
        );
        assert_eq!(
            CollectionStatus::from_qdrant_status(99),
            CollectionStatus::Red
        );
    }

    #[test]
    fn test_init_from() {
        let init = InitFrom::collection("source_collection");
        assert_eq!(init.collection, "source_collection");
        assert_eq!(init.to_qdrant_init_from(), "source_collection");
    }
}
