//! Workflow execution engine.

use std::sync::Arc;

use tokio::sync::Semaphore;

use crate::error::WorkflowResult;
use crate::graph::WorkflowGraph;

use super::EngineConfig;

/// Tracing target for engine operations.
const TRACING_TARGET: &str = "nvisy_workflow::engine";

/// The workflow execution engine.
///
/// Manages workflow execution, concurrency, and resource allocation.
pub struct Engine {
    config: EngineConfig,
    semaphore: Arc<Semaphore>,
}

impl Engine {
    /// Creates a new engine with the given configuration.
    pub fn new(config: EngineConfig) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_runs));

        tracing::info!(
            target: TRACING_TARGET,
            max_concurrent_runs = config.max_concurrent_runs,
            default_timeout_secs = config.default_timeout.as_secs(),
            "Workflow engine initialized"
        );

        Self { config, semaphore }
    }

    /// Creates a new engine with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(EngineConfig::default())
    }

    /// Returns the engine configuration.
    pub fn config(&self) -> &EngineConfig {
        &self.config
    }

    /// Validates a workflow graph.
    pub fn validate(&self, workflow: &WorkflowGraph) -> WorkflowResult<()> {
        workflow.validate()
    }

    /// Executes a workflow graph.
    ///
    /// This will:
    /// 1. Acquire a semaphore permit for concurrency control
    /// 2. Validate the workflow
    /// 3. Execute nodes in topological order
    /// 4. Handle errors and retries
    pub async fn execute(&self, workflow: &WorkflowGraph) -> WorkflowResult<()> {
        let _permit = self.semaphore.acquire().await.map_err(|e| {
            crate::error::WorkflowError::Internal(format!("semaphore closed: {}", e))
        })?;

        // Validate the workflow first
        workflow.validate()?;

        // Get execution order
        let order = workflow.topological_order()?;

        tracing::debug!(
            target: TRACING_TARGET,
            node_count = order.len(),
            "Starting workflow execution"
        );

        // TODO: Execute each node in order
        // For now, just log the execution plan
        for (idx, node_id) in order.iter().enumerate() {
            if let Some(node) = workflow.get_node(*node_id) {
                tracing::trace!(
                    target: TRACING_TARGET,
                    step = idx + 1,
                    node_id = %node_id,
                    node_name = node.name(),
                    "Would execute node"
                );
            }
        }

        tracing::debug!(
            target: TRACING_TARGET,
            "Workflow execution completed (placeholder)"
        );

        Ok(())
    }

    /// Returns the number of available execution slots.
    pub fn available_slots(&self) -> usize {
        self.semaphore.available_permits()
    }
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("config", &self.config)
            .field("available_slots", &self.available_slots())
            .finish()
    }
}
