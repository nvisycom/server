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
//! # Tool Support
//!
//! Each agent can optionally be created with tools enabled via the `with_tools`
//! parameter. When enabled, agents have access to relevant tools:
//!
//! | Agent | Tools |
//! |-------|-------|
//! | `VisionAgent` | `ScratchpadTool` |
//! | `TextAnalysisAgent` | `ScratchpadTool`, `JsonSchemaTool` |
//! | `TextGenerationAgent` | `ScratchpadTool` |
//! | `TableAgent` | `ScratchpadTool`, `JsonSchemaTool` |
//! | `StructuredOutputAgent` | `ScratchpadTool`, `JsonSchemaTool` |

pub mod memory;
mod tool;

mod structured_output;
mod table;
mod text_analysis;
mod text_generation;
mod vision;

pub use structured_output::{StructuredOutput, StructuredOutputAgent};
pub use table::{ColumnDescription, TableAgent};
pub use text_analysis::{
    Classification, Entity, Relationship, Sentiment, TextAnalysisAgent, TextAnalysisOutput,
};
pub use text_generation::TextGenerationAgent;
pub use vision::VisionAgent;
