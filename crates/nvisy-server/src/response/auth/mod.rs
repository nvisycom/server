//! Authentication response types: the outbound side of the auth flow.
//!
//! [`WebSession`] and [`ClearedSession`] emit the browser session and CSRF
//! cookies (set on sign-in, cleared on sign-out); [`CookieConfig`] is the
//! deployment cookie policy they apply.

mod session;

pub use session::{ClearedSession, CookieConfig, WebSession};
