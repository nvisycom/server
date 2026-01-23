//! Workflow compiler for transforming definitions into executable graphs.
//!
//! The compiler takes a [`WorkflowDefinition`] and a [`CredentialsRegistry`]
//! and produces a [`CompiledGraph`] that can be executed by the engine.
//!
//! # Compilation Process
//!
//! 1. **Validation**: Check that the definition is structurally valid
//! 2. **Cache Resolution**: Connect cache slot inputs to outputs
//! 3. **Node Compilation**: Create processors and streams for each node
//! 4. **Graph Building**: Build the petgraph structure with compiled nodes

use std::collections::HashMap;

use super::context::Context;
use nvisy_rig::agent::Agents;
use nvisy_rig::provider::CompletionProvider;
use petgraph::graph::{DiGraph, NodeIndex};

use crate::definition::{Input, NodeId, NodeKind, Output, Workflow};
use crate::error::{Error, Result};
use crate::graph::{
    ChunkProcessor, CompiledGraph, CompiledInput, CompiledNode, CompiledOutput, CompiledSwitch,
    CompiledTransform, DeriveProcessor, EdgeData, EmbeddingProcessor, EnrichProcessor,
    ExtractProcessor, InputStream, OutputStream, PartitionProcessor,
};
use crate::provider::{
    CompletionProviderParams, CredentialsRegistry, EmbeddingProviderParams, InputProvider,
    InputProviderParams, IntoProvider, OutputProviderParams,
};

/// Workflow compiler that transforms definitions into executable graphs.
pub struct WorkflowCompiler<'a> {
    /// Credentials registry for resolving provider credentials.
    registry: &'a CredentialsRegistry,
    /// Execution context for provider initialization.
    ctx: Context,
}

impl<'a> WorkflowCompiler<'a> {
    /// Creates a new workflow compiler.
    pub fn new(registry: &'a CredentialsRegistry, ctx: Context) -> Self {
        Self { registry, ctx }
    }

    /// Compiles a workflow definition into an executable graph.
    pub async fn compile(&self, def: Workflow) -> Result<CompiledGraph> {
        // Phase 1: Validate definition structure
        self.validate(&def)?;

        // Phase 2: Resolve cache slots
        let resolved = self.resolve_cache_slots(&def)?;

        // Phase 3: Compile each node
        let mut compiled_nodes = HashMap::new();
        for (id, node) in &def.nodes {
            // Skip cache slot nodes - they're resolved during edge building
            if self.is_cache_only_node(&node.kind) {
                continue;
            }
            let compiled = self.compile_node(&node.kind).await?;
            compiled_nodes.insert(*id, compiled);
        }

        // Phase 4: Build petgraph
        let (graph, node_indices) = self.build_graph(compiled_nodes, &resolved.edges)?;

        Ok(CompiledGraph::new(graph, node_indices, def.metadata))
    }

    /// Validates the workflow definition structure.
    fn validate(&self, def: &Workflow) -> Result<()> {
        // Check edge references
        for edge in &def.edges {
            if !def.nodes.contains_key(&edge.from) {
                return Err(Error::InvalidDefinition(format!(
                    "edge references non-existent node: {}",
                    edge.from
                )));
            }
            if !def.nodes.contains_key(&edge.to) {
                return Err(Error::InvalidDefinition(format!(
                    "edge references non-existent node: {}",
                    edge.to
                )));
            }
        }

        // Check for at least one input and output
        let has_input = def.nodes.values().any(|n| n.is_input());
        let has_output = def.nodes.values().any(|n| n.is_output());

        if !has_input {
            return Err(Error::InvalidDefinition(
                "workflow must have at least one input node".into(),
            ));
        }
        if !has_output {
            return Err(Error::InvalidDefinition(
                "workflow must have at least one output node".into(),
            ));
        }

        Ok(())
    }

    /// Checks if a node is a cache-only node (input from cache or output to cache).
    fn is_cache_only_node(&self, def: &NodeKind) -> bool {
        match def {
            NodeKind::Input(input) => matches!(input, Input::CacheSlot(_)),
            NodeKind::Output(output) => matches!(output, Output::Cache(_)),
            _ => false,
        }
    }

    /// Resolves cache slots by connecting cache inputs to cache outputs.
    fn resolve_cache_slots(&self, def: &Workflow) -> Result<ResolvedDefinition> {
        // Collect cache slot outputs (nodes that write to cache slots)
        let mut cache_outputs: HashMap<String, Vec<NodeId>> = HashMap::new();
        for (id, node) in &def.nodes {
            if let NodeKind::Output(Output::Cache(slot)) = &node.kind {
                cache_outputs
                    .entry(slot.slot.clone())
                    .or_default()
                    .push(*id);
            }
        }

        // Collect cache slot inputs (nodes that read from cache slots)
        let mut cache_inputs: HashMap<String, Vec<NodeId>> = HashMap::new();
        for (id, node) in &def.nodes {
            if let NodeKind::Input(Input::CacheSlot(slot)) = &node.kind {
                cache_inputs.entry(slot.slot.clone()).or_default().push(*id);
            }
        }

        // Build resolved edges
        let mut resolved_edges = Vec::new();

        // Add original edges (excluding edges to/from cache nodes)
        for edge in &def.edges {
            let from_node = def.nodes.get(&edge.from);
            let to_node = def.nodes.get(&edge.to);

            let from_is_cache = from_node
                .map(|n| self.is_cache_only_node(&n.kind))
                .unwrap_or(false);
            let to_is_cache = to_node
                .map(|n| self.is_cache_only_node(&n.kind))
                .unwrap_or(false);

            if !from_is_cache && !to_is_cache {
                resolved_edges.push(ResolvedEdge {
                    from: edge.from,
                    to: edge.to,
                    data: EdgeData {
                        from_port: edge.from_port.clone(),
                        to_port: edge.to_port.clone(),
                    },
                });
            }
        }

        // Connect nodes writing to cache slots with nodes reading from them
        // by looking at incoming/outgoing edges
        for (slot_name, output_ids) in &cache_outputs {
            if let Some(input_ids) = cache_inputs.get(slot_name) {
                // For each cache output node, find what writes to it
                for output_id in output_ids {
                    let writers: Vec<NodeId> = def
                        .edges
                        .iter()
                        .filter(|e| e.to == *output_id)
                        .map(|e| e.from)
                        .collect();

                    // For each cache input node, find what reads from it
                    for input_id in input_ids {
                        let readers: Vec<NodeId> = def
                            .edges
                            .iter()
                            .filter(|e| e.from == *input_id)
                            .map(|e| e.to)
                            .collect();

                        // Connect writers directly to readers
                        for writer in &writers {
                            for reader in &readers {
                                resolved_edges.push(ResolvedEdge {
                                    from: *writer,
                                    to: *reader,
                                    data: EdgeData {
                                        from_port: None,
                                        to_port: None,
                                    },
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(ResolvedDefinition {
            edges: resolved_edges,
        })
    }

    /// Compiles a single node definition into a compiled node.
    async fn compile_node(&self, def: &NodeKind) -> Result<CompiledNode> {
        match def {
            NodeKind::Input(input) => {
                let stream = self.create_input_stream(input).await?;
                Ok(CompiledNode::Input(CompiledInput::new(stream)))
            }
            NodeKind::Output(output) => {
                let stream = self.create_output_stream(output).await?;
                Ok(CompiledNode::Output(CompiledOutput::new(stream)))
            }
            NodeKind::Transform(transformer) => {
                let processor = self.create_processor(transformer).await?;
                Ok(CompiledNode::Transform(Box::new(processor)))
            }
            NodeKind::Switch(switch) => {
                Ok(CompiledNode::Switch(CompiledSwitch::from(switch.clone())))
            }
        }
    }

    /// Creates an input stream from an input definition.
    async fn create_input_stream(&self, input: &Input) -> Result<InputStream> {
        match input {
            Input::Provider(provider_def) => {
                let stream = self
                    .create_provider_input_stream(&provider_def.provider)
                    .await?;
                Ok(stream)
            }
            Input::CacheSlot(_) => {
                // Cache inputs are resolved during cache slot resolution
                // This shouldn't be called for cache inputs
                Err(Error::Internal(
                    "cache input nodes should be resolved before compilation".into(),
                ))
            }
        }
    }

    /// Creates an input stream from provider parameters.
    async fn create_provider_input_stream(
        &self,
        params: &InputProviderParams,
    ) -> Result<InputStream> {
        let creds = self.registry.get(params.credentials_id())?;
        let provider = params.clone().into_provider(creds.clone()).await?;

        let stream = self.read_from_provider(&provider).await?;

        // Map the stream to our Result type
        use futures::StreamExt;
        let mapped = stream.map(|r| r.map_err(|e| Error::Internal(e.to_string())));

        Ok(InputStream::new(Box::pin(mapped)))
    }

    /// Reads from an input provider using the appropriate context type.
    async fn read_from_provider(
        &self,
        provider: &InputProvider,
    ) -> Result<futures::stream::BoxStream<'static, nvisy_dal::Result<nvisy_dal::AnyDataValue>>>
    {
        match provider {
            InputProvider::S3(_) | InputProvider::Gcs(_) | InputProvider::Azblob(_) => {
                let ctx = self.ctx.to_object_context();
                provider.read_object_stream(&ctx).await
            }
            InputProvider::Postgres(_) | InputProvider::Mysql(_) => {
                let ctx = self.ctx.to_relational_context();
                provider.read_relational_stream(&ctx).await
            }
        }
    }

    /// Creates an output stream from an output definition.
    async fn create_output_stream(&self, output: &Output) -> Result<OutputStream> {
        match output {
            Output::Provider(provider_def) => {
                let stream = self
                    .create_provider_output_stream(&provider_def.provider)
                    .await?;
                Ok(stream)
            }
            Output::Cache(_) => {
                // Cache outputs are resolved during cache slot resolution
                Err(Error::Internal(
                    "cache output nodes should be resolved before compilation".into(),
                ))
            }
        }
    }

    /// Creates an output stream from provider parameters.
    async fn create_provider_output_stream(
        &self,
        params: &OutputProviderParams,
    ) -> Result<OutputStream> {
        let creds = self.registry.get(params.credentials_id())?;
        let provider = params.clone().into_provider(creds.clone()).await?;
        let sink = provider.write_sink();

        Ok(OutputStream::new(sink))
    }

    /// Creates a processor from a transformer definition.
    async fn create_processor(
        &self,
        transformer: &crate::definition::Transformer,
    ) -> Result<CompiledTransform> {
        use crate::definition::Transformer;

        match transformer {
            Transformer::Partition(p) => Ok(CompiledTransform::Partition(PartitionProcessor::new(
                p.strategy,
                p.include_page_breaks,
                p.discard_unsupported,
            ))),
            Transformer::Chunk(c) => {
                if c.contextual_chunking {
                    // Need completion provider for contextual chunking
                    // For now, we don't have provider params in chunk definition
                    // So contextual chunking won't have agents
                    Ok(CompiledTransform::Chunk(ChunkProcessor::new(
                        c.chunk_strategy.clone(),
                    )))
                } else {
                    Ok(CompiledTransform::Chunk(ChunkProcessor::new(
                        c.chunk_strategy.clone(),
                    )))
                }
            }
            Transformer::Embedding(e) => {
                let provider = self.create_embedding_provider(&e.provider).await?;
                Ok(CompiledTransform::Embedding(EmbeddingProcessor::new(
                    provider,
                    e.normalize,
                )))
            }
            Transformer::Enrich(e) => {
                let agents = self.create_agents(&e.provider).await?;
                Ok(CompiledTransform::Enrich(EnrichProcessor::new(
                    agents,
                    e.task.clone(),
                    e.override_prompt.clone(),
                )))
            }
            Transformer::Extract(e) => {
                let agents = self.create_agents(&e.provider).await?;
                Ok(CompiledTransform::Extract(ExtractProcessor::new(
                    agents,
                    e.task.clone(),
                    e.override_prompt.clone(),
                )))
            }
            Transformer::Derive(d) => {
                let agents = self.create_agents(&d.provider).await?;
                Ok(CompiledTransform::Derive(DeriveProcessor::new(
                    agents,
                    d.task,
                    d.override_prompt.clone(),
                )))
            }
        }
    }

    /// Creates an embedding provider from parameters.
    async fn create_embedding_provider(
        &self,
        params: &EmbeddingProviderParams,
    ) -> Result<nvisy_rig::provider::EmbeddingProvider> {
        let creds = self.registry.get(params.credentials_id())?;
        params.clone().into_provider(creds.clone()).await
    }

    /// Creates agents from completion provider parameters.
    async fn create_agents(&self, params: &CompletionProviderParams) -> Result<Agents> {
        let provider = self.create_completion_provider(params).await?;
        Ok(Agents::new(provider))
    }

    /// Creates a completion provider from parameters.
    async fn create_completion_provider(
        &self,
        params: &CompletionProviderParams,
    ) -> Result<CompletionProvider> {
        let creds = self.registry.get(params.credentials_id())?;
        params.clone().into_provider(creds.clone()).await
    }

    /// Builds the petgraph from compiled nodes and resolved edges.
    fn build_graph(
        &self,
        nodes: HashMap<NodeId, CompiledNode>,
        edges: &[ResolvedEdge],
    ) -> Result<(DiGraph<CompiledNode, EdgeData>, HashMap<NodeId, NodeIndex>)> {
        let mut graph = DiGraph::new();
        let mut node_indices = HashMap::new();

        // Add nodes
        for (id, node) in nodes {
            let idx = graph.add_node(node);
            node_indices.insert(id, idx);
        }

        // Add edges
        for edge in edges {
            let from_idx = node_indices.get(&edge.from).ok_or_else(|| {
                Error::InvalidDefinition(format!("edge references unknown node: {}", edge.from))
            })?;
            let to_idx = node_indices.get(&edge.to).ok_or_else(|| {
                Error::InvalidDefinition(format!("edge references unknown node: {}", edge.to))
            })?;

            graph.add_edge(*from_idx, *to_idx, edge.data.clone());
        }

        // Verify acyclic
        if petgraph::algo::is_cyclic_directed(&graph) {
            return Err(Error::InvalidDefinition("workflow contains a cycle".into()));
        }

        Ok((graph, node_indices))
    }
}

/// Resolved edge after cache slot resolution.
struct ResolvedEdge {
    from: NodeId,
    to: NodeId,
    data: EdgeData,
}

/// Resolved workflow definition after cache slot resolution.
struct ResolvedDefinition {
    edges: Vec<ResolvedEdge>,
}
