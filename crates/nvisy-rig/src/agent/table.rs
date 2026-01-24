//! Table agent for table processing tasks.

use rig::agent::{Agent, AgentBuilder};
use rig::completion::Prompt;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::tool::{JsonResponse, JsonSchemaTool, ScratchpadTool};
use crate::Result;
use crate::provider::CompletionProvider;

const NAME: &str = "TableAgent";
const DESCRIPTION: &str = "Agent for table processing including description and format conversion (HTML, Markdown, CSV, JSON)";

const PREAMBLE: &str = "\
You are a table processing assistant specialized in understanding and transforming tabular data.
Your task is to analyze tables and either describe their contents or convert them to different formats.
Preserve data accuracy and structure during conversions.
When outputting structured data, use valid JSON format.";

const PROMPT_DESCRIBE: &str = "\
Describe this table concisely. Include:
- What data the table contains
- Number of rows and columns
- Key insights or patterns";

const PROMPT_DESCRIBE_COLUMNS: &str = "\
For each column in this table, provide:
- Column name
- Data type (text, number, date, etc.)
- Brief description of what the column contains

Format as a JSON array with objects containing 'name', 'type', and 'description' fields.";

const PROMPT_TO_HTML: &str = "\
Convert this table to clean, semantic HTML.
Use <table>, <thead>, <tbody>, <tr>, <th>, and <td> tags appropriately.
Do not include any CSS or styling. Only output the HTML, no explanation.";

const PROMPT_TO_MARKDOWN: &str = "\
Convert this table to Markdown format.
Use proper Markdown table syntax with | separators and header dividers.
Only output the Markdown table, no explanation.";

const PROMPT_TO_CSV: &str = "\
Convert this table to CSV format.
Use commas as delimiters and quote fields containing commas or newlines.
Only output the CSV, no explanation.";

const PROMPT_TO_JSON: &str = "\
Convert this table to a JSON array of objects.
Each row should be an object with column names as keys.
Only output valid JSON, no explanation.";

/// Column description for table schema validation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ColumnDescription {
    /// Column name.
    pub name: String,
    /// Data type (text, number, date, etc.).
    #[serde(rename = "type")]
    pub data_type: String,
    /// Brief description of what the column contains.
    pub description: String,
}

/// Agent for table processing tasks.
///
/// Handles tasks that involve understanding and transforming tables:
/// - Table description
/// - Column descriptions
/// - Format conversion (HTML, Markdown, CSV, JSON)
///
/// When `with_tools` is enabled, the agent has access to:
/// - `ScratchpadTool` - For working on format conversions iteratively
/// - `JsonSchemaTool` - For validating JSON output
pub struct TableAgent {
    agent: Agent<CompletionProvider>,
    model_name: String,
}

impl TableAgent {
    /// Creates a new table agent with the given completion provider.
    ///
    /// # Arguments
    /// * `provider` - The completion provider to use
    /// * `with_tools` - Whether to enable tool usage (scratchpad, schema validation)
    pub fn new(provider: CompletionProvider, with_tools: bool) -> Self {
        let model_name = provider.model_name().to_string();
        let builder = AgentBuilder::new(provider)
            .name(NAME)
            .description(DESCRIPTION)
            .preamble(PREAMBLE);

        let agent = if with_tools {
            builder
                .tool(ScratchpadTool::new())
                .tool(JsonSchemaTool::<Vec<ColumnDescription>>::new())
                .build()
        } else {
            builder.build()
        };

        Self { agent, model_name }
    }

    /// Generates a description of a table.
    #[tracing::instrument(skip(self, table_content), fields(agent = NAME, model = %self.model_name, content_len = table_content.len()))]
    pub async fn describe(&self, table_content: &str) -> Result<String> {
        let prompt = format!("{}\n\nTable:\n{}", PROMPT_DESCRIBE, table_content);
        let response = self.agent.prompt(&prompt).await?;
        tracing::debug!(response_len = response.len(), "describe completed");
        Ok(response)
    }

    /// Generates descriptions for each column in a table.
    #[tracing::instrument(skip(self, table_content), fields(agent = NAME, model = %self.model_name, content_len = table_content.len()))]
    pub async fn describe_columns(&self, table_content: &str) -> Result<Vec<ColumnDescription>> {
        let prompt = format!("{}\n\nTable:\n{}", PROMPT_DESCRIBE_COLUMNS, table_content);
        let response = self.agent.prompt(&prompt).await?;
        let columns: Vec<ColumnDescription> = JsonResponse::parse(&response)?;
        tracing::debug!(column_count = columns.len(), "describe_columns completed");
        Ok(columns)
    }

    /// Converts a table to HTML format.
    #[tracing::instrument(skip(self, table_content), fields(agent = NAME, model = %self.model_name, content_len = table_content.len()))]
    pub async fn to_html(&self, table_content: &str) -> Result<String> {
        let prompt = format!("{}\n\nTable:\n{}", PROMPT_TO_HTML, table_content);
        let response = self.agent.prompt(&prompt).await?;
        tracing::debug!(response_len = response.len(), "to_html completed");
        Ok(response)
    }

    /// Converts a table to Markdown format.
    #[tracing::instrument(skip(self, table_content), fields(agent = NAME, model = %self.model_name, content_len = table_content.len()))]
    pub async fn to_markdown(&self, table_content: &str) -> Result<String> {
        let prompt = format!("{}\n\nTable:\n{}", PROMPT_TO_MARKDOWN, table_content);
        let response = self.agent.prompt(&prompt).await?;
        tracing::debug!(response_len = response.len(), "to_markdown completed");
        Ok(response)
    }

    /// Converts a table to CSV format.
    #[tracing::instrument(skip(self, table_content), fields(agent = NAME, model = %self.model_name, content_len = table_content.len()))]
    pub async fn to_csv(&self, table_content: &str) -> Result<String> {
        let prompt = format!("{}\n\nTable:\n{}", PROMPT_TO_CSV, table_content);
        let response = self.agent.prompt(&prompt).await?;
        tracing::debug!(response_len = response.len(), "to_csv completed");
        Ok(response)
    }

    /// Converts a table to JSON format.
    #[tracing::instrument(skip(self, table_content), fields(agent = NAME, model = %self.model_name, content_len = table_content.len()))]
    pub async fn to_json(&self, table_content: &str) -> Result<String> {
        let prompt = format!("{}\n\nTable:\n{}", PROMPT_TO_JSON, table_content);
        let response = self.agent.prompt(&prompt).await?;
        tracing::debug!(response_len = response.len(), "to_json completed");
        Ok(response)
    }
}
