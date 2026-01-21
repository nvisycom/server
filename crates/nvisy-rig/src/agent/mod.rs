//! Agent module for LLM-powered document processing tasks.
//!
//! This module provides specialized agents for different types of tasks:
//!
//! - [`VisionAgent`] - VLM tasks (image description, OCR, object detection)
//! - [`TableAgent`] - Table processing (descriptions, format conversion)
//! - [`TextAnalysisAgent`] - Text analysis (NER, keywords, classification, sentiment)
//! - [`TextGenerationAgent`] - Text generation (summarization, titles)
//! - [`StructuredOutputAgent`] - JSON conversion (structured extraction)
//!
//! Use [`Agents`] to create all agents from a single provider.

mod structured_output;
mod table;
mod text_analysis;
mod text_generation;
mod vision;

pub use structured_output::StructuredOutputAgent;
pub use table::TableAgent;
pub use text_analysis::TextAnalysisAgent;
pub use text_generation::TextGenerationAgent;
pub use vision::VisionAgent;

use crate::provider::CompletionProvider;

/// Collection of all specialized agents.
///
/// Provides convenient access to all agents created from a single completion provider.
///
/// # Example
///
/// ```ignore
/// let provider = CompletionProvider::new(...);
/// let agents = Agents::new(provider);
///
/// let summary = agents.text_generation().summarize("...").await?;
/// let entities = agents.text_analysis().extract_entities("...").await?;
/// ```
pub struct Agents {
    pub structured_output_agent: StructuredOutputAgent,
    pub table_agent: TableAgent,
    pub text_analysis_agent: TextAnalysisAgent,
    pub text_generation_agent: TextGenerationAgent,
    pub vision_agent: VisionAgent,
}

impl Agents {
    /// Creates all agents from a completion provider.
    pub fn new(provider: CompletionProvider) -> Self {
        Self {
            structured_output_agent: StructuredOutputAgent::new(provider.clone()),
            table_agent: TableAgent::new(provider.clone()),
            text_analysis_agent: TextAnalysisAgent::new(provider.clone()),
            text_generation_agent: TextGenerationAgent::new(provider.clone()),
            vision_agent: VisionAgent::new(provider),
        }
    }
}
