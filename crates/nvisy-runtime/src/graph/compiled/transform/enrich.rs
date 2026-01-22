//! Enrich processor.

use nvisy_dal::AnyDataValue;
use nvisy_rig::agent::Agents;

use super::Process;
use crate::error::Result;
use crate::graph::definition::EnrichTask;

/// Processor for enriching elements with metadata/descriptions.
pub struct EnrichProcessor {
    /// Agents for enrichment tasks.
    agents: Agents,
    /// The enrichment task to perform.
    task: EnrichTask,
    /// Optional prompt override.
    override_prompt: Option<String>,
}

impl EnrichProcessor {
    /// Creates a new enrich processor.
    pub fn new(agents: Agents, task: EnrichTask, override_prompt: Option<String>) -> Self {
        Self {
            agents,
            task,
            override_prompt,
        }
    }

    /// Returns the enrichment task.
    pub fn task(&self) -> &EnrichTask {
        &self.task
    }

    /// Returns the prompt override, if any.
    pub fn override_prompt(&self) -> Option<&str> {
        self.override_prompt.as_deref()
    }
}

impl Process for EnrichProcessor {
    async fn process(&self, input: Vec<AnyDataValue>) -> Result<Vec<AnyDataValue>> {
        // TODO: Implement enrichment using agents
        // Use self.agents.vision_agent for image tasks
        // Use self.agents.table_agent for table tasks
        let _ = &self.agents; // Suppress unused warning
        Ok(input)
    }
}

impl std::fmt::Debug for EnrichProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnrichProcessor")
            .field("task", &self.task)
            .field("override_prompt", &self.override_prompt)
            .finish_non_exhaustive()
    }
}
