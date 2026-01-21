//! Workflow execution engine.

use std::sync::Arc;

use nvisy_dal::core::Context;
use tokio::sync::Semaphore;

use super::EngineConfig;
use super::context::ExecutionContext;
use crate::error::{WorkflowError, WorkflowResult};
use crate::graph::{NodeData, NodeId, WorkflowGraph};
use crate::provider::{CredentialsRegistry, InputProvider, OutputProvider};

/// Tracing target for engine operations.
const TRACING_TARGET: &str = "nvisy_workflow::engine";

/// The workflow execution engine.
///
/// Manages workflow execution, concurrency, and resource allocation.
/// Executes workflows in a pipe-based streaming manner: each data item
/// flows through the entire pipeline before the next item is processed.
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

    /// Executes a workflow graph with the given credentials.
    ///
    /// Execution is pipe-based: items are read from inputs one at a time,
    /// flow through all transformers, and are written to outputs before
    /// the next item is processed.
    pub async fn execute(
        &self,
        workflow: &WorkflowGraph,
        credentials: CredentialsRegistry,
    ) -> WorkflowResult<ExecutionContext> {
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|e| WorkflowError::Internal(format!("semaphore closed: {}", e)))?;

        workflow.validate()?;

        let order = workflow.topological_order()?;

        tracing::debug!(
            target: TRACING_TARGET,
            node_count = order.len(),
            "Starting workflow execution"
        );

        let mut ctx = ExecutionContext::new(credentials);

        // Build the pipeline: create providers for input and output nodes
        let pipeline = self.build_pipeline(workflow, &order, &ctx).await?;

        // Execute the pipeline: stream items through
        self.execute_pipeline(workflow, &order, &pipeline, &mut ctx)
            .await?;

        tracing::debug!(
            target: TRACING_TARGET,
            items_processed = ctx.items_processed(),
            "Workflow execution completed"
        );

        Ok(ctx)
    }

    /// Builds the pipeline by creating providers for input and output nodes.
    async fn build_pipeline(
        &self,
        workflow: &WorkflowGraph,
        order: &[NodeId],
        ctx: &ExecutionContext,
    ) -> WorkflowResult<Pipeline> {
        let mut input_providers = Vec::new();
        let mut output_providers = Vec::new();

        for node_id in order {
            let Some(node) = workflow.get_node(*node_id) else {
                continue;
            };

            match node {
                NodeData::Input(input_node) => {
                    let credentials_id = input_node.provider.credentials_id();
                    let credentials = ctx.credentials().get(credentials_id)?.clone();
                    let config = input_node.provider.clone().into_config(credentials)?;
                    let provider = config.into_provider()?;
                    input_providers.push((*node_id, provider));
                }
                NodeData::Output(output_node) => {
                    let credentials_id = output_node.provider.credentials_id();
                    let credentials = ctx.credentials().get(credentials_id)?.clone();
                    let config = output_node.provider.clone().into_config(credentials)?;
                    let provider = config.into_provider().await?;
                    output_providers.push((*node_id, provider));
                }
                NodeData::Transformer(_) => {
                    // Transformers don't need pre-built providers
                }
            }
        }

        Ok(Pipeline {
            input_providers,
            output_providers,
        })
    }

    /// Executes the pipeline by streaming items through.
    ///
    /// For each input item:
    /// 1. Set as current (single item)
    /// 2. Run through transformers (can expand: 1 item → N items)
    /// 3. Write all resulting items to outputs
    async fn execute_pipeline(
        &self,
        workflow: &WorkflowGraph,
        order: &[NodeId],
        pipeline: &Pipeline,
        ctx: &mut ExecutionContext,
    ) -> WorkflowResult<()> {
        // For each input provider, stream items through the pipeline
        for (input_node_id, input_provider) in &pipeline.input_providers {
            tracing::debug!(
                target: TRACING_TARGET,
                node_id = %input_node_id,
                "Reading from input provider"
            );

            let dal_ctx = Context::default();
            let items = input_provider.read(&dal_ctx).await?;

            // Process each input item through the pipeline
            for item in items {
                // Start with single input item
                ctx.set_current_single(item);

                // Execute transformers in order (each can expand 1→N)
                for node_id in order {
                    let Some(node) = workflow.get_node(*node_id) else {
                        continue;
                    };

                    if let NodeData::Transformer(transformer_node) = node {
                        self.execute_transformer(*node_id, transformer_node, ctx)?;
                    }
                }

                // Write all resulting items to output providers
                let output_data = ctx.take_current();
                if !output_data.is_empty() {
                    for (output_node_id, output_provider) in &pipeline.output_providers {
                        tracing::trace!(
                            target: TRACING_TARGET,
                            node_id = %output_node_id,
                            item_count = output_data.len(),
                            "Writing to output provider"
                        );

                        let dal_ctx = Context::default();
                        output_provider.write(&dal_ctx, output_data.clone()).await?;
                    }
                }

                ctx.mark_processed();
                ctx.clear();
            }
        }

        Ok(())
    }

    /// Executes a transformer node on the current data.
    fn execute_transformer(
        &self,
        node_id: NodeId,
        _transformer_node: &crate::graph::TransformerNode,
        ctx: &mut ExecutionContext,
    ) -> WorkflowResult<()> {
        // TODO: Apply transformation based on transformer_node.config
        // For now, pass through data unchanged

        tracing::trace!(
            target: TRACING_TARGET,
            node_id = %node_id,
            has_data = ctx.has_current(),
            "Transformer node executed (passthrough)"
        );

        Ok(())
    }

    /// Returns the number of available execution slots.
    pub fn available_slots(&self) -> usize {
        self.semaphore.available_permits()
    }
}

/// Pre-built pipeline with providers ready for execution.
struct Pipeline {
    input_providers: Vec<(NodeId, InputProvider)>,
    output_providers: Vec<(NodeId, OutputProvider)>,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("config", &self.config)
            .field("available_slots", &self.available_slots())
            .finish()
    }
}
