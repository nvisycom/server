//! Rate limiting middleware for API endpoints.

mod by_ip;

pub(crate) use by_ip::{rate_limit_by_ip, rate_limit_strict};
