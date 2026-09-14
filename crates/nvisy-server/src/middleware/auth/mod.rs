//! Authentication middleware.
//!
//! - [`session`] validates the session (via the [`AuthState`] extractor) and
//!   slides its idle bound forward on use.
//! - [`csrf`] enforces CSRF protection on cookie-authenticated state-changing
//!   requests.
//!
//! [`AuthState`]: crate::extract::AuthState

mod csrf;
mod session;

/// Tracing target shared by the auth middleware.
const TRACING_TARGET: &str = "nvisy_server::middleware::auth";

pub use csrf::csrf_protect;
pub use session::{require_authentication, slide_session};
