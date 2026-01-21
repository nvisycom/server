//! Output node types for writing data to storage backends, vector databases, or cache.

use derive_more::From;
use nvisy_dal::DataTypeId;
use serde::{Deserialize, Serialize};

use super::route::CacheSlot;
use crate::provider::OutputProviderParams;

/// Destination for output data.
#[derive(Debug, Clone, PartialEq, From, Serialize, Deserialize)]
#[serde(tag = "destination", rename_all = "snake_case")]
pub enum OutputDestination {
    /// Write to a storage provider or vector database.
    Provider(OutputProviderParams),
    /// Write to a cache slot.
    Cache(CacheSlot),
}

/// A data output node that writes or consumes data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputNode {
    /// Output destination (provider or cache).
    #[serde(flatten)]
    pub destination: OutputDestination,
}

impl OutputNode {
    /// Creates a new output node from a provider.
    pub fn from_provider(provider: OutputProviderParams) -> Self {
        Self {
            destination: OutputDestination::Provider(provider),
        }
    }

    /// Creates a new output node from a cache slot.
    pub fn from_cache(slot: CacheSlot) -> Self {
        Self {
            destination: OutputDestination::Cache(slot),
        }
    }

    /// Returns the expected input data type based on the destination kind.
    ///
    /// For cache slots, the type is unknown at compile time.
    pub fn input_type(&self) -> Option<DataTypeId> {
        match &self.destination {
            OutputDestination::Provider(p) => Some(p.output_type()),
            OutputDestination::Cache(_) => None,
        }
    }

    /// Returns whether this output writes to a provider.
    pub const fn is_provider(&self) -> bool {
        matches!(self.destination, OutputDestination::Provider(_))
    }

    /// Returns whether this output writes to a cache slot.
    pub const fn is_cache(&self) -> bool {
        matches!(self.destination, OutputDestination::Cache(_))
    }

    /// Returns the cache slot name if this is a cache output.
    pub fn cache_slot(&self) -> Option<&str> {
        match &self.destination {
            OutputDestination::Cache(slot) => Some(&slot.slot),
            _ => None,
        }
    }
}

impl From<OutputProviderParams> for OutputNode {
    fn from(provider: OutputProviderParams) -> Self {
        Self::from_provider(provider)
    }
}

impl From<CacheSlot> for OutputNode {
    fn from(slot: CacheSlot) -> Self {
        Self::from_cache(slot)
    }
}
