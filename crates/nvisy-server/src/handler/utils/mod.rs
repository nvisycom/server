//! [`Pagination`], [`PaginationMetadata`], [`CustomRoutes`] and other utilities.

mod custom_routes;
mod pagination;

pub use crate::handler::utils::custom_routes::{CustomRoutes, RouterMapFn};
pub use crate::handler::utils::pagination::PaginationRequest;
