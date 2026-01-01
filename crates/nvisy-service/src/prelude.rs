//! Commonly used items from nvisy-service.
//!
//! This prelude module exports the most commonly used traits and the main
//! service container to simplify imports in consuming code.

pub use crate::inference::{InferenceProvider, InferenceService};
pub use crate::{Error, ErrorKind, Result};
#[cfg(feature = "test-utils")]
#[cfg_attr(docsrs, doc(cfg(feature = "test-utils")))]
pub use crate::{MockConfig, MockProvider};
