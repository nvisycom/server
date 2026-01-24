//! Extract processor.

use nvisy_dal::AnyDataValue;
use nvisy_rig::agent::{StructuredOutputAgent, TableAgent, TextAnalysisAgent};

use super::Process;
use crate::definition::ExtractTask;
use crate::error::Result;

/// Processor for extracting structured data or converting formats.
pub struct ExtractProcessor {
    /// Agent for text analysis (NER, keywords, classification, sentiment).
    text_analysis_agent: TextAnalysisAgent,
    /// Agent for table processing.
    table_agent: TableAgent,
    /// Agent for structured output extraction.
    structured_output_agent: StructuredOutputAgent,
    /// The extraction task to perform.
    task: ExtractTask,
    /// Optional prompt override.
    override_prompt: Option<String>,
}

impl ExtractProcessor {
    /// Creates a new extract processor.
    pub fn new(
        text_analysis_agent: TextAnalysisAgent,
        table_agent: TableAgent,
        structured_output_agent: StructuredOutputAgent,
        task: ExtractTask,
        override_prompt: Option<String>,
    ) -> Self {
        Self {
            text_analysis_agent,
            table_agent,
            structured_output_agent,
            task,
            override_prompt,
        }
    }

    /// Returns the extraction task.
    pub fn task(&self) -> &ExtractTask {
        &self.task
    }

    /// Returns the prompt override, if any.
    pub fn override_prompt(&self) -> Option<&str> {
        self.override_prompt.as_deref()
    }
}

impl Process for ExtractProcessor {
    async fn process(&self, input: Vec<AnyDataValue>) -> Result<Vec<AnyDataValue>> {
        // TODO: Implement extraction using agents
        // Use self.text_analysis_agent for NER, keywords, classification, sentiment
        // Use self.table_agent for table conversion
        // Use self.structured_output_agent for JSON conversion
        let _ = (
            &self.text_analysis_agent,
            &self.table_agent,
            &self.structured_output_agent,
        );
        Ok(input)
    }
}

impl std::fmt::Debug for ExtractProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtractProcessor")
            .field("task", &self.task)
            .field("override_prompt", &self.override_prompt)
            .finish_non_exhaustive()
    }
}
