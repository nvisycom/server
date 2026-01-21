//! Table agent for table processing tasks.

use rig::agent::{Agent, AgentBuilder};
use rig::completion::Prompt;

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

/// Agent for table processing tasks.
///
/// Handles tasks that involve understanding and transforming tables:
/// - Table description
/// - Column descriptions
/// - Format conversion (HTML, Markdown, CSV, JSON)
pub struct TableAgent {
    agent: Agent<CompletionProvider>,
}

impl TableAgent {
    /// Creates a new table agent with the given completion provider.
    pub fn new(provider: CompletionProvider) -> Self {
        let agent = AgentBuilder::new(provider)
            .name(NAME)
            .description(DESCRIPTION)
            .preamble(PREAMBLE)
            .build();
        Self { agent }
    }

    /// Generates a description of a table.
    pub async fn describe(&self, table_content: &str) -> Result<String> {
        let prompt = format!("{}\n\nTable:\n{}", PROMPT_DESCRIBE, table_content);
        Ok(self.agent.prompt(&prompt).await?)
    }

    /// Generates descriptions for each column in a table.
    pub async fn describe_columns(&self, table_content: &str) -> Result<String> {
        let prompt = format!("{}\n\nTable:\n{}", PROMPT_DESCRIBE_COLUMNS, table_content);
        Ok(self.agent.prompt(&prompt).await?)
    }

    /// Converts a table to HTML format.
    pub async fn to_html(&self, table_content: &str) -> Result<String> {
        let prompt = format!("{}\n\nTable:\n{}", PROMPT_TO_HTML, table_content);
        Ok(self.agent.prompt(&prompt).await?)
    }

    /// Converts a table to Markdown format.
    pub async fn to_markdown(&self, table_content: &str) -> Result<String> {
        let prompt = format!("{}\n\nTable:\n{}", PROMPT_TO_MARKDOWN, table_content);
        Ok(self.agent.prompt(&prompt).await?)
    }

    /// Converts a table to CSV format.
    pub async fn to_csv(&self, table_content: &str) -> Result<String> {
        let prompt = format!("{}\n\nTable:\n{}", PROMPT_TO_CSV, table_content);
        Ok(self.agent.prompt(&prompt).await?)
    }

    /// Converts a table to JSON format.
    pub async fn to_json(&self, table_content: &str) -> Result<String> {
        let prompt = format!("{}\n\nTable:\n{}", PROMPT_TO_JSON, table_content);
        Ok(self.agent.prompt(&prompt).await?)
    }
}
