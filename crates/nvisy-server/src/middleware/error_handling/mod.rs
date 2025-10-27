//! Error handling middleware for transforming errors into responses.

mod handlers;
mod panic;

pub use handlers::handle_error;
pub use panic::catch_panic;
