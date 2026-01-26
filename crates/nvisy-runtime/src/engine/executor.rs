//! Workflow execution engine.

use std::sync::Arc;

use futures::{SinkExt, StreamExt};
use tokio::sync::Semaphore;

use super::EngineConfig;
use super::compiler::WorkflowCompiler;
use super::context::{Context, ExecutionContext};
use super::credentials::CredentialsRegistry;
use crate::definition::{NodeId, Workflow};
use crate::error::{Error, Result};
use crate::graph::{CompiledGraph, CompiledNode, InputStream, OutputStream, Process};

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

    /// Executes a workflow definition.
    ///
    /// The definition is compiled into an executable graph and then executed.
    /// Execution is pipe-based: items are read from inputs one at a time,
    /// flow through all transformers, and are written to outputs before
    /// the next item is processed.
    pub async fn execute(
        &self,
        definition: Workflow,
        credentials: CredentialsRegistry,
        ctx: Context,
    ) -> Result<ExecutionContext> {
        // Compile the definition into an executable graph
        let compiler = WorkflowCompiler::new(&credentials, ctx);
        let graph = compiler.compile(definition).await?;

        self.execute_graph(graph, credentials).await
    }

    /// Executes a pre-compiled workflow graph.
    ///
    /// Use [`Self::execute`] to compile and execute a workflow definition in one step.
    /// This method is useful when you want to reuse a compiled graph multiple times.
    pub async fn execute_graph(
        &self,
        mut graph: CompiledGraph,
        credentials: CredentialsRegistry,
    ) -> Result<ExecutionContext> {
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|e| Error::Internal(format!("semaphore closed: {}", e)))?;

        let order = graph
            .topological_order()
            .ok_or_else(|| Error::InvalidDefinition("compiled graph contains a cycle".into()))?;

        tracing::debug!(
            target: TRACING_TARGET,
            node_count = order.len(),
            "Starting workflow execution"
        );

        let mut ctx = ExecutionContext::new(credentials);

        // Execute the compiled pipeline
        self.execute_pipeline(&mut graph, &order, &mut ctx).await?;

        tracing::debug!(
            target: TRACING_TARGET,
            items_processed = ctx.items_processed(),
            "Workflow execution completed"
        );

        Ok(ctx)
    }

    /// Executes a compiled pipeline by streaming items through.
    async fn execute_pipeline(
        &self,
        graph: &mut CompiledGraph,
        order: &[NodeId],
        ctx: &mut ExecutionContext,
    ) -> Result<()> {
        // Collect input and output node IDs
        let input_ids: Vec<NodeId> = order
            .iter()
            .filter(|id| graph.node(id).map(|n| n.is_input()).unwrap_or(false))
            .copied()
            .collect();

        let output_ids: Vec<NodeId> = order
            .iter()
            .filter(|id| graph.node(id).map(|n| n.is_output()).unwrap_or(false))
            .copied()
            .collect();

        let transform_ids: Vec<NodeId> = order
            .iter()
            .filter(|id| graph.node(id).map(|n| n.is_transform()).unwrap_or(false))
            .copied()
            .collect();

        // Take ownership of input streams
        let mut input_streams: Vec<(NodeId, InputStream)> = Vec::new();
        for id in &input_ids {
            if let Some(node) = graph.node_mut(id)
                && let CompiledNode::Input(compiled_input) = node
            {
                // Create a placeholder stream and swap with the real one
                let placeholder = InputStream::new(Box::pin(futures::stream::empty()));
                let stream = std::mem::replace(compiled_input.stream_mut(), placeholder);
                input_streams.push((*id, stream));
            }
        }

        // Take ownership of output streams
        let mut output_streams: Vec<(NodeId, OutputStream)> = Vec::new();
        for id in &output_ids {
            if let Some(CompiledNode::Output(compiled_output)) = graph.node_mut(id) {
                // Create a placeholder sink
                let placeholder = OutputStream::new(Box::pin(futures::sink::drain().sink_map_err(
                    |_: std::convert::Infallible| Error::Internal("drain sink error".into()),
                )));
                let stream = std::mem::replace(compiled_output.stream_mut(), placeholder);
                output_streams.push((*id, stream));
            }
        }

        // Process each input stream
        for (input_node_id, mut input_stream) in input_streams {
            tracing::debug!(
                target: TRACING_TARGET,
                node_id = %input_node_id,
                "Reading from input stream"
            );

            while let Some(result) = input_stream.next().await {
                let item = result?;

                // Start with single input item
                ctx.set_current_single(item);

                // Execute transforms in order
                for transform_id in &transform_ids {
                    if let Some(node) = graph.node(transform_id)
                        && let Some(transform) = node.as_transform()
                    {
                        let input_data = ctx.take_current();
                        let output_data = transform.process(input_data).await?;
                        ctx.set_current(output_data);
                    }
                }

                // Write to outputs
                let output_data = ctx.take_current();
                if !output_data.is_empty() {
                    for (output_node_id, output_stream) in &mut output_streams {
                        tracing::trace!(
                            target: TRACING_TARGET,
                            node_id = %output_node_id,
                            item_count = output_data.len(),
                            "Writing to output stream"
                        );

                        for item in output_data.clone() {
                            output_stream.send(item).await?;
                        }
                    }
                }

                ctx.mark_processed();
                ctx.clear();
            }
        }

        // Close all output streams
        for (_, mut output_stream) in output_streams {
            output_stream.close().await?;
        }

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
