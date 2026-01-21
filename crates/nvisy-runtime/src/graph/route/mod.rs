//! Routing nodes for conditional data flow.
//!
//! This module provides nodes for controlling data flow in workflows:
//! - [`CacheSlot`]: Named temporary storage for data sharing between branches
//! - [`SwitchNode`]: Conditional routing based on data properties

mod cache;
mod switch;

pub use cache::CacheSlot;
pub use switch::{ContentTypeCategory, DateField, SwitchBranch, SwitchCondition, SwitchNode};
