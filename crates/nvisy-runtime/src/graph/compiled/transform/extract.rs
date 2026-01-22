//! Extract processor.

use nvisy_dal::AnyDataValue;
use nvisy_rig::agent::Agents;

use super::Process;
use crate::error::Result;
use crate::graph::definition::ExtractTask;

/// Processor for extracting structured data or converting formats.
pub struct ExtractProcessor {
    /// Agents for extraction tasks.
    agents: Agents,
    /// The extraction task to perform.
    task: ExtractTask,
    /// Optional prompt override.
    override_prompt: Option<String>,
}

impl ExtractProcessor {
    /// Creates a new extract processor.
    pub fn new(agents: Agents, task: ExtractTask, override_prompt: Option<String>) -> Self {
        Self {
            agents,
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
        // Use self.agents.text_analysis_agent for NER, keywords, classification, sentiment
        // Use self.agents.table_agent for table conversion
        // Use self.agents.structured_output_agent for JSON conversion
        let _ = &self.agents; // Suppress unused warning
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
