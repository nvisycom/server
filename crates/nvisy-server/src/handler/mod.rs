//! All `axum::`[`Router`]s with related `axum::`[`Handler`]s.
//!
//! # Usage Example
//!
//! ```rust
//! use nvisy_server::handler::{openapi_routes, CustomRoutes};
//! use nvisy_server::service::{ServiceConfig, ServiceState};
//! use utoipa_axum::router::OpenApiRouter;
//! use axum::routing::get;
//!
//! async fn custom_handler() -> &'static str {
//!     "Hello from custom route!"
//! }
//!
//! # async fn example() -> anyhow::Result<()> {
//! let config = ServiceConfig::default();
//! let state = ServiceState::from_config(&config).await?;
//!
//! // Create custom routes
//! let custom_private_router = OpenApiRouter::new()
//!     .route("/custom-private", get(custom_handler));
//!
//! let custom_public_router = OpenApiRouter::new()
//!     .route("/custom-public", get(custom_handler));
//!
//! // Build custom routes configuration
//! let custom_routes = CustomRoutes::new()
//!     .with_private_routes(custom_private_router)
//!     .with_public_routes(custom_public_router);
//!
//! // Create the complete router
//! let router = openapi_routes(custom_routes, state);
//! # Ok(())
//! # }
//! ```
//!
//! [`Router`]: axum::routing::Router
//! [`Handler`]: axum::handler::Handler

mod accounts;
mod authentication;
mod constraints;
mod document_files;
mod document_versions;
mod documents;
mod error;
mod monitors;
mod project_invites;
mod project_members;
pub mod project_websocket;
mod projects;
mod response;
mod utils;

use axum::middleware::from_fn_with_state;
use axum::response::{IntoResponse, Response};
use utoipa_axum::router::OpenApiRouter;

pub use crate::extract::ProjectPermission;
pub use crate::handler::error::{Error, ErrorKind, Result};
pub(crate) use crate::handler::response::ErrorResponse;
pub use crate::handler::utils::{CustomRoutes, Pagination};
use crate::middleware::{refresh_token_middleware, require_authentication};
use crate::service::ServiceState;

#[inline]
async fn handler() -> Response {
    ErrorKind::NotFound.into_response()
}

/// Returns an [`OpenApiRouter`] with all private routes.
fn private_routes(
    additional_routes: Option<OpenApiRouter<ServiceState>>,
    state: ServiceState,
) -> OpenApiRouter<ServiceState> {
    let mut router = OpenApiRouter::new()
        .merge(accounts::routes(state.clone()))
        .merge(projects::routes())
        .merge(project_invites::routes())
        .merge(project_members::routes())
        .merge(project_websocket::routes())
        .merge(documents::routes())
        .merge(document_files::routes())
        .merge(document_versions::routes());

    if let Some(additional) = additional_routes {
        router = router.merge(additional);
    }

    router
}

/// Returns an [`OpenApiRouter`] with all public routes.
fn public_routes(
    additional_routes: Option<OpenApiRouter<ServiceState>>,
) -> OpenApiRouter<ServiceState> {
    let mut router = OpenApiRouter::new()
        .merge(authentication::routes())
        .merge(monitors::routes());

    if let Some(additional) = additional_routes {
        router = router.merge(additional);
    }

    router
}

/// Returns an [`OpenApiRouter`] with all routes.
pub fn openapi_routes(routes: CustomRoutes, state: ServiceState) -> OpenApiRouter<ServiceState> {
    let require_authentication = from_fn_with_state(state.clone(), require_authentication);
    let refresh_token_middleware = from_fn_with_state(state.clone(), refresh_token_middleware);

    let mut router = OpenApiRouter::new();

    // Private routes with authentication middleware
    let private_router = private_routes(routes.private_routes, state.clone())
        .route_layer(require_authentication)
        .route_layer(refresh_token_middleware);

    // Public routes without authentication
    let public_router = public_routes(routes.public_routes);

    router = router
        .merge(private_router)
        .merge(public_router)
        .fallback(handler);

    router
}

#[cfg(test)]
mod test {
    use axum_test::TestServer;
    use utoipa_axum::router::OpenApiRouter;

    use crate::handler::{CustomRoutes, openapi_routes};
    use crate::service::{ServiceConfig, ServiceState};

    /// Returns a new [`TestServer`] with the given router.
    pub async fn create_test_server_with_router(
        router: impl Fn(ServiceState) -> OpenApiRouter<ServiceState>,
    ) -> anyhow::Result<TestServer> {
        let config = ServiceConfig::default();
        let state = ServiceState::from_config(&config).await?;
        let router = router(state.clone());
        create_test_server_with_state(router, state).await
    }

    /// Returns a new [`TestServer`] with the given router and state.
    pub async fn create_test_server_with_state(
        router: OpenApiRouter<ServiceState>,
        state: ServiceState,
    ) -> anyhow::Result<TestServer> {
        let app = router.with_state(state);
        let (app, _) = app.split_for_parts();
        let server = TestServer::new(app)?;
        Ok(server)
    }

    /// Returns a new [`TestServer`] with the default router and state.
    pub async fn create_test_server() -> anyhow::Result<TestServer> {
        let config = ServiceConfig::default();
        let state = ServiceState::from_config(&config).await?;
        let router = openapi_routes(CustomRoutes::new(), state.clone());
        create_test_server_with_state(router, state).await
    }

    #[tokio::test]
    async fn handlers() -> anyhow::Result<()> {
        let server = create_test_server().await?;
        assert!(server.is_running());
        Ok(())
    }
}
