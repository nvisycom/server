//! Routing types for conditional data flow.
//!
//! This module provides types for controlling data flow in workflows:
//! - [`CacheSlot`]: Named connection point for linking workflow branches
//! - [`SwitchDef`]: Conditional routing based on data properties

mod cache;
mod switch;

pub use cache::CacheSlot;
pub use switch::{
    FileCategory, FileCategoryCondition, LanguageCondition, SwitchCondition, SwitchDef,
};
