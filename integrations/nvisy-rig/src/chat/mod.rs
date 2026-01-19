//! Chat service for AI-powered document conversations.
//!
//! This module provides:
//! - [`ChatService`] - Main entry point for chat functionality
//! - [`ChatStream`] - Streaming chat response
//! - [`ChatEvent`] - Events emitted during chat
//! - [`ChatResponse`] - Complete response after stream ends
//! - [`UsageStats`] - Token usage statistics
//! - [`agent`] - Agent execution for processing chat messages

pub mod agent;
mod event;
mod response;
mod service;
mod stream;
mod usage;

pub use agent::{Agent, AgentConfig, AgentContext, AgentExecutor, PromptBuilder};
pub use event::ChatEvent;
pub use response::ChatResponse;
pub use service::ChatService;
pub use stream::ChatStream;
pub use usage::UsageStats;
