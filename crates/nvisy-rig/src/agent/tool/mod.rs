//! Tools module for agent function calling capabilities.
//!
//! This module provides tools used internally by agents:
//!
//! - [`ScratchpadTool`] - Temporary working storage for drafting
//! - [`JsonSchemaTool`] - Validate JSON against schema (generic over `T: JsonSchema`)
//! - [`JsonResponse`] - Parse JSON from LLM responses (handles markdown blocks, etc.)

mod json_schema;
mod scratchpad;

pub use json_schema::{JsonResponse, JsonSchemaTool};
pub use scratchpad::ScratchpadTool;
