//! Common types shared across services.

mod health;
mod timing;

pub use health::{ServiceHealth, ServiceStatus};
pub use timing::Timing;
