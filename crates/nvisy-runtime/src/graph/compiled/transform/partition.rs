//! Partition processor.

use nvisy_dal::AnyDataValue;

use super::Process;
use crate::error::Result;
use crate::graph::definition::PartitionStrategy;

/// Processor for partitioning documents into elements.
#[derive(Debug)]
pub struct PartitionProcessor {
    /// Partitioning strategy to use.
    strategy: PartitionStrategy,
    /// Whether to include page break markers.
    include_page_breaks: bool,
    /// Whether to discard unsupported element types.
    discard_unsupported: bool,
}

impl PartitionProcessor {
    /// Creates a new partition processor.
    pub fn new(
        strategy: PartitionStrategy,
        include_page_breaks: bool,
        discard_unsupported: bool,
    ) -> Self {
        Self {
            strategy,
            include_page_breaks,
            discard_unsupported,
        }
    }

    /// Returns the partitioning strategy.
    pub fn strategy(&self) -> PartitionStrategy {
        self.strategy
    }

    /// Returns whether page breaks are included.
    pub fn include_page_breaks(&self) -> bool {
        self.include_page_breaks
    }

    /// Returns whether unsupported types are discarded.
    pub fn discard_unsupported(&self) -> bool {
        self.discard_unsupported
    }
}

impl Process for PartitionProcessor {
    async fn process(&self, input: Vec<AnyDataValue>) -> Result<Vec<AnyDataValue>> {
        // TODO: Implement document partitioning based on strategy
        // For now, pass through unchanged
        Ok(input)
    }
}
