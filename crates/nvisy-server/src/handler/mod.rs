//! All `axum::`[`Router`]s with related `axum::`[`Handler`]s.
//!
//! [`Router`]: axum::routing::Router
//! [`Handler`]: axum::handler::Handler

mod accounts;
mod api_tokens;
mod authentication;
mod document_comments;
mod document_files;
mod document_versions;
mod documents;
mod error;
mod monitors;
mod project_invites;
mod project_members;
mod project_websocket;
mod projects;
pub mod request;
pub mod response;
mod utils;

use axum::middleware::from_fn_with_state;
use axum::response::{IntoResponse, Response};
use utoipa_axum::router::OpenApiRouter;

pub use crate::extract::Permission;
pub use crate::handler::error::{Error, ErrorKind, Result};
pub use crate::handler::request::PaginationRequest;
pub(crate) use crate::handler::response::ErrorResponse;
pub use crate::handler::utils::{CustomRoutes, RouterMapFn};
use crate::middleware::{refresh_token_middleware, require_authentication};
use crate::service::ServiceState;

#[inline]
async fn handler() -> Response {
    ErrorKind::NotFound.into_response()
}

/// Returns an [`OpenApiRouter`] with all private routes.
fn private_routes(
    additional_routes: Option<OpenApiRouter<ServiceState>>,
    service_state: ServiceState,
) -> OpenApiRouter<ServiceState> {
    let mut router = OpenApiRouter::new()
        .merge(accounts::routes(service_state.clone()))
        .merge(api_tokens::routes())
        .merge(projects::routes())
        .merge(project_invites::routes())
        .merge(project_members::routes())
        .merge(project_websocket::routes())
        .merge(documents::routes())
        .merge(document_comments::routes())
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
    _service_state: ServiceState,
    disable_authentication: bool,
) -> OpenApiRouter<ServiceState> {
    let mut router = OpenApiRouter::new();

    if !disable_authentication {
        router = router.merge(authentication::routes());
    }

    router = router.merge(monitors::routes());

    if let Some(additional) = additional_routes {
        router = router.merge(additional);
    }

    router
}

/// Returns an [`OpenApiRouter`] with all routes.
pub fn openapi_routes(
    mut routes: CustomRoutes,
    state: ServiceState,
) -> OpenApiRouter<ServiceState> {
    let require_authentication = from_fn_with_state(state.clone(), require_authentication);
    let refresh_token_middleware = from_fn_with_state(state.clone(), refresh_token_middleware);

    // Private routes.
    let mut private_router = private_routes(routes.private_routes.take(), state.clone());
    private_router = routes.map_private_before_middleware(private_router);
    private_router = private_router
        .route_layer(require_authentication)
        .route_layer(refresh_token_middleware);
    private_router = routes.map_private_after_middleware(private_router);

    // Public routes.
    let mut public_router = public_routes(
        routes.public_routes.take(),
        state,
        routes.disable_authentication,
    );
    public_router = routes.map_public_before_middleware(public_router);
    public_router = routes.map_public_after_middleware(public_router);

    OpenApiRouter::new()
        .merge(private_router)
        .merge(public_router)
        .fallback(handler)
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
