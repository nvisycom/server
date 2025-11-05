//! Application state and dependency injection.

mod cache;
mod config;
mod error;
mod policy;
mod security;
mod state;

pub use cache::HealthCache;
pub use config::ServiceConfig;
pub use error::{Result, ServiceError};
pub use policy::DataCollectionPolicy;
pub use security::{
    AuthHasher, AuthKeys, AuthKeysConfig, PasswordStrength, RateLimitKey, RateLimiter,
};
pub use state::ServiceState;
