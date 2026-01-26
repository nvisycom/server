//! Memory module for agent conversation history and context management.
//!
//! This module provides:
//!
//! - [`ChatHistory`] - Conversation history with automatic compaction
//! - [`CompactionStrategy`] - Strategy for handling history overflow (truncate or summarize)
//! - [`WorkingMemory`] - Key-value store for agent working context

mod history;
mod working;

pub use history::{ChatHistory, CompactionStrategy};
pub use working::WorkingMemory;
