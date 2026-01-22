//! Tools module for agent function calling capabilities.
//!
//! This module provides tools that agents can use during execution:
//!
//! - [`VectorSearchTool`] - Search vector store for similar chunks
//! - [`DocumentFetchTool`] - Fetch document/chunk by ID
//! - [`MetadataQueryTool`] - Query documents by metadata filters
//! - [`ContextStoreTool`] - Save/retrieve from agent memory
//! - [`ScratchpadTool`] - Temporary working storage
//! - [`WebFetchTool`] - Fetch content from URLs
//! - [`ImageAnalysisTool`] - Analyze images with VLM
//! - [`JsonSchemaTool`] - Validate JSON against schema

mod context_store;
mod document_fetch;
mod image_analysis;
mod json_schema;
mod metadata_query;
mod scratchpad;
mod vector_search;
mod web_fetch;

pub use context_store::{ContextStore, ContextStoreTool};
pub use document_fetch::{DocumentFetchTool, DocumentFetcher};
pub use image_analysis::ImageAnalysisTool;
pub use json_schema::JsonSchemaTool;
pub use metadata_query::{MetadataQuerier, MetadataQueryTool};
pub use scratchpad::{Scratchpad, ScratchpadTool};
pub use vector_search::{VectorSearchTool, VectorSearcher};
pub use web_fetch::{FetchResponse, WebFetchTool, WebFetcher};
