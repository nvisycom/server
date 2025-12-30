//! Query options for workspace integration queries.

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::types::IntegrationType;

/// Filter options for workspace integrations.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct IntegrationFilter {
    /// Filter by integration type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integration_type: Option<IntegrationType>,
}

impl IntegrationFilter {
    /// Creates a new empty filter.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Filters by integration type.
    #[inline]
    pub fn with_type(mut self, integration_type: IntegrationType) -> Self {
        self.integration_type = Some(integration_type);
        self
    }

    /// Returns whether any filter is active.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.integration_type.is_none()
    }
}
