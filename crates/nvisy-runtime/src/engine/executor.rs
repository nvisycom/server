//! Workflow execution engine.

use std::sync::Arc;

use nvisy_dal::core::Context;
use tokio::sync::Semaphore;

use super::EngineConfig;
use super::context::ExecutionContext;
use crate::error::{WorkflowError, WorkflowResult};
use crate::graph::{InputSource, NodeData, NodeId, OutputDestination, WorkflowGraph};
use crate::provider::{CredentialsRegistry, InputProvider, IntoProvider, OutputProvider};

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

    /// Validates a workflow graph against a credentials registry.
    ///
    /// Checks graph structure, constraints, and that all referenced
    /// credentials exist in the registry.
    pub fn validate(
        &self,
        workflow: &WorkflowGraph,
        registry: &CredentialsRegistry,
    ) -> WorkflowResult<()> {
        workflow.validate(registry)
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

        workflow.validate(&credentials)?;

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
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();

        for node_id in order {
            let Some(node) = workflow.get_node(*node_id) else {
                continue;
            };

            match node {
                NodeData::Input(input_node) => {
                    let input = match &input_node.source {
                        InputSource::Provider(params) => {
                            let credentials_id = params.credentials_id();
                            let credentials = ctx.credentials().get(credentials_id)?.clone();
                            let config = params.clone().into_provider(credentials)?;
                            let provider = config.into_provider()?;
                            PipelineInput::Provider(provider)
                        }
                        InputSource::Cache(slot) => PipelineInput::Cache(slot.slot.clone()),
                    };
                    inputs.push((*node_id, input));
                }
                NodeData::Output(output_node) => {
                    let output = match &output_node.destination {
                        OutputDestination::Provider(params) => {
                            let credentials_id = params.credentials_id();
                            let credentials = ctx.credentials().get(credentials_id)?.clone();
                            let config = params.clone().into_provider(credentials)?;
                            let provider = config.into_provider().await?;
                            PipelineOutput::Provider(provider)
                        }
                        OutputDestination::Cache(slot) => PipelineOutput::Cache(slot.slot.clone()),
                    };
                    outputs.push((*node_id, output));
                }
                NodeData::Transformer(_) => {
                    // Transformers don't need pre-built providers
                }
            }
        }

        Ok(Pipeline { inputs, outputs })
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
        // For each input, stream items through the pipeline
        for (input_node_id, input) in &pipeline.inputs {
            tracing::debug!(
                target: TRACING_TARGET,
                node_id = %input_node_id,
                "Reading from input"
            );

            let items = match input {
                PipelineInput::Provider(provider) => {
                    let dal_ctx = Context::default();
                    provider.read(&dal_ctx).await?
                }
                PipelineInput::Cache(name) => ctx.read_cache(name),
            };

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

                // Write all resulting items to outputs
                let output_data = ctx.take_current();
                if !output_data.is_empty() {
                    for (output_node_id, output) in &pipeline.outputs {
                        tracing::trace!(
                            target: TRACING_TARGET,
                            node_id = %output_node_id,
                            item_count = output_data.len(),
                            "Writing to output"
                        );

                        match output {
                            PipelineOutput::Provider(provider) => {
                                let dal_ctx = Context::default();
                                provider.write(&dal_ctx, output_data.clone()).await?;
                            }
                            PipelineOutput::Cache(name) => {
                                ctx.write_cache(name, output_data.clone());
                            }
                        }
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
        _transformer_config: &crate::graph::TransformerConfig,
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

/// Pre-built pipeline with inputs and outputs ready for execution.
struct Pipeline {
    inputs: Vec<(NodeId, PipelineInput)>,
    outputs: Vec<(NodeId, PipelineOutput)>,
}

/// Input source in the pipeline.
enum PipelineInput {
    /// Read from a storage provider.
    Provider(InputProvider),
    /// Read from a named cache slot.
    Cache(String),
}

/// Output destination in the pipeline.
enum PipelineOutput {
    /// Write to a storage provider.
    Provider(OutputProvider),
    /// Write to a named cache slot.
    Cache(String),
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("config", &self.config)
            .field("available_slots", &self.available_slots())
            .finish()
    }
}
