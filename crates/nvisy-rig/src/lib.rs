#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod rag;
pub mod service;
pub mod session;
pub mod tool;

mod error;

pub use error::{Error, Result};
pub use service::RigService;
pub use service::provider::{ProviderConfig, ProviderRegistry};
pub use session::Session;
// Re-export agent types from session::agent for convenience
pub use session::agent::{Agent, AgentConfig, AgentContext, AgentExecutor, PromptBuilder};
// Re-export edit types from tool::edit for convenience
pub use tool::edit::{ApplyError, ApplyResult, EditLocation, EditOperation, ProposedEdit};

/// Tracing target for the main library.
pub const TRACING_TARGET: &str = "nvisy_rig";
