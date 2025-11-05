//! Cache management services and utilities.
//!
//! This module provides caching services for various system components to improve
//! performance and reduce load on external services. All cache implementations use
//! atomic operations and are thread-safe.
//!
//! ## Available Services
//!
//! - [`HealthCache`] - Health check caching with configurable TTL

mod health_cache;

pub use health_cache::HealthCache;
