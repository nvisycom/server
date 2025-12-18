//! Qdrant client connection management and configuration.

mod qdrant_client;
mod qdrant_config;

pub use qdrant_client::{QdrantClient, QdrantConnection};
pub use qdrant_config::QdrantConfig;

/// Health status of the Qdrant cluster
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct HealthStatus {
    /// Overall cluster status
    pub status: String,
    /// Title of the service
    pub title: Option<String>,
    /// Version of Qdrant
    pub version: Option<String>,
    /// Commit hash of the build
    pub commit: Option<String>,
}

impl HealthStatus {
    /// Check if the cluster is healthy
    pub fn is_healthy(&self) -> bool {
        self.status == "ok" || self.status == "healthy"
    }
}

/// Collection statistics and information
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ClusterInfo {
    /// Cluster name
    pub name: String,
    /// List of peers in the cluster
    pub peers: Vec<PeerInfo>,
    /// Current peer ID
    pub peer_id: Option<u64>,
}

/// Information about a peer in the cluster
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct PeerInfo {
    /// Peer ID
    pub id: u64,
    /// Peer URI
    pub uri: String,
    /// Peer state
    pub state: String,
}
