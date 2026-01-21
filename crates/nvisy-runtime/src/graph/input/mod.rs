//! Input node types for reading data from storage backends or cache.

use derive_more::From;
use nvisy_dal::DataTypeId;
use serde::{Deserialize, Serialize};

use super::route::CacheSlot;
use crate::provider::InputProviderParams;

/// Source of input data.
#[derive(Debug, Clone, PartialEq, From, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum InputSource {
    /// Read from a storage provider.
    Provider(InputProviderParams),
    /// Read from a cache slot.
    Cache(CacheSlot),
}

/// A data input node that reads or produces data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputNode {
    /// Input source (provider or cache).
    #[serde(flatten)]
    pub source: InputSource,
}

impl InputNode {
    /// Creates a new input node from a provider.
    pub fn from_provider(provider: InputProviderParams) -> Self {
        Self {
            source: InputSource::Provider(provider),
        }
    }

    /// Creates a new input node from a cache slot.
    pub fn from_cache(slot: CacheSlot) -> Self {
        Self {
            source: InputSource::Cache(slot),
        }
    }

    /// Returns the output data type based on the source kind.
    ///
    /// For cache slots, the type is unknown at compile time.
    pub fn output_type(&self) -> Option<DataTypeId> {
        match &self.source {
            InputSource::Provider(p) => Some(p.output_type()),
            InputSource::Cache(_) => None,
        }
    }

    /// Returns whether this input reads from a provider.
    pub const fn is_provider(&self) -> bool {
        matches!(self.source, InputSource::Provider(_))
    }

    /// Returns whether this input reads from a cache slot.
    pub const fn is_cache(&self) -> bool {
        matches!(self.source, InputSource::Cache(_))
    }

    /// Returns the cache slot name if this is a cache input.
    pub fn cache_slot(&self) -> Option<&str> {
        match &self.source {
            InputSource::Cache(slot) => Some(&slot.slot),
            _ => None,
        }
    }
}

impl From<InputProviderParams> for InputNode {
    fn from(provider: InputProviderParams) -> Self {
        Self::from_provider(provider)
    }
}

impl From<CacheSlot> for InputNode {
    fn from(slot: CacheSlot) -> Self {
        Self::from_cache(slot)
    }
}
