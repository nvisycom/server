//! Enrich processor.

use nvisy_dal::datatype::AnyDataValue;
use nvisy_rig::agent::{TableAgent, VisionAgent};

use super::Process;
use crate::definition::EnrichTask;
use crate::error::Result;

/// Processor for enriching elements with metadata/descriptions.
pub struct EnrichProcessor {
    /// Agent for vision/image tasks.
    vision_agent: VisionAgent,
    /// Agent for table processing.
    table_agent: TableAgent,
    /// The enrichment task to perform.
    task: EnrichTask,
    /// Optional prompt override.
    override_prompt: Option<String>,
}

impl EnrichProcessor {
    /// Creates a new enrich processor.
    pub fn new(
        vision_agent: VisionAgent,
        table_agent: TableAgent,
        task: EnrichTask,
        override_prompt: Option<String>,
    ) -> Self {
        Self {
            vision_agent,
            table_agent,
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
        // Use self.vision_agent for image tasks
        // Use self.table_agent for table tasks
        let _ = (&self.vision_agent, &self.table_agent); // Suppress unused warning
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
