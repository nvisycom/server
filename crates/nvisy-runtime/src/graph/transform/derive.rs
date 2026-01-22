//! Derive processor.

use nvisy_dal::AnyDataValue;
use nvisy_rig::agent::Agents;

use super::Process;
use crate::definition::DeriveTask;
use crate::error::Result;

/// Processor for generating new content from input.
pub struct DeriveProcessor {
    /// Agents for derivation tasks.
    agents: Agents,
    /// The derivation task to perform.
    task: DeriveTask,
    /// Optional prompt override.
    override_prompt: Option<String>,
}

impl DeriveProcessor {
    /// Creates a new derive processor.
    pub fn new(agents: Agents, task: DeriveTask, override_prompt: Option<String>) -> Self {
        Self {
            agents,
            task,
            override_prompt,
        }
    }

    /// Returns the derivation task.
    pub fn task(&self) -> DeriveTask {
        self.task
    }

    /// Returns the prompt override, if any.
    pub fn override_prompt(&self) -> Option<&str> {
        self.override_prompt.as_deref()
    }
}

impl Process for DeriveProcessor {
    async fn process(&self, input: Vec<AnyDataValue>) -> Result<Vec<AnyDataValue>> {
        // TODO: Implement derivation using agents
        // Use self.agents.text_generation_agent for summarization and title generation
        let _ = &self.agents; // Suppress unused warning
        Ok(input)
    }
}

impl std::fmt::Debug for DeriveProcessor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeriveProcessor")
            .field("task", &self.task)
            .field("override_prompt", &self.override_prompt)
            .finish_non_exhaustive()
    }
}
