//! [`CustomRoutes`] and other utilities.

mod accounts;
mod avatar;
mod custom_routes;

pub use accounts::{build_password_user_inputs, resolve_creator};
pub use avatar::{avatar_response, read_image_field};
pub use custom_routes::{BuiltinModule, CustomRoutes, RouterMapFn};
