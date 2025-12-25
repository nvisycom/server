//! All `axum::`[`Router`]s with related `axum::`[`Handler`]s.
//!
//! [`Router`]: axum::routing::Router
//! [`Handler`]: axum::handler::Handler

mod accounts;
mod authentication;
mod comments;
mod documents;
mod error;
mod files;
mod integrations;
mod invites;
mod members;
mod monitors;
mod pipelines;
mod projects;
pub mod request;
pub mod response;
mod templates;
mod tokens;
mod utility;
mod webhooks;
mod websocket;

use aide::axum::ApiRouter;
use axum::middleware::from_fn_with_state;
use axum::response::{IntoResponse, Response};
pub use error::{Error, ErrorKind, Result};
pub use utility::{CustomRoutes, RouterMapFn};

use crate::middleware::{refresh_token_middleware, require_authentication};
use crate::service::ServiceState;

#[inline]
async fn handler() -> Response {
    ErrorKind::NotFound.into_response()
}

/// Returns an [`ApiRouter`] with all private routes.
fn private_routes(
    additional_routes: Option<ApiRouter<ServiceState>>,
    service_state: ServiceState,
) -> ApiRouter<ServiceState> {
    let mut router = ApiRouter::new()
        .merge(accounts::routes(service_state.clone()))
        .merge(tokens::routes())
        .merge(projects::routes())
        .merge(integrations::routes())
        .merge(invites::routes())
        .merge(members::routes())
        .merge(pipelines::routes())
        .merge(templates::routes())
        .merge(webhooks::routes())
        .merge(websocket::routes())
        .merge(files::routes())
        .merge(documents::routes())
        .merge(comments::routes());

    if let Some(additional) = additional_routes {
        router = router.merge(additional);
    }

    router
}

/// Returns an [`ApiRouter`] with all public routes.
fn public_routes(
    additional_routes: Option<ApiRouter<ServiceState>>,
    _service_state: ServiceState,
    disable_authentication: bool,
) -> ApiRouter<ServiceState> {
    let mut router = ApiRouter::new();

    if !disable_authentication {
        router = router.merge(authentication::routes());
    }

    router = router.merge(monitors::routes());

    if let Some(additional) = additional_routes {
        router = router.merge(additional);
    }

    router
}

/// Returns an [`ApiRouter`] with all routes.
pub fn routes(mut routes: CustomRoutes, state: ServiceState) -> ApiRouter<ServiceState> {
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

    ApiRouter::new()
        .merge(private_router)
        .merge(public_router)
        .fallback(handler)
}

#[cfg(test)]
mod test {
    use aide::axum::ApiRouter;
    use axum::Router;
    use axum_test::TestServer;

    use crate::handler::{CustomRoutes, routes};
    use crate::service::{ServiceConfig, ServiceState};

    /// Returns a new [`TestServer`] with the given router.
    pub async fn create_test_server_with_router(
        router: impl Fn(ServiceState) -> ApiRouter<ServiceState>,
    ) -> anyhow::Result<TestServer> {
        let config = ServiceConfig::default();
        let mock_services = nvisy_test::create_mock_services();
        let state = ServiceState::from_config(config, mock_services).await?;
        let router = router(state.clone());
        create_test_server_with_state(router, state).await
    }

    /// Returns a new [`TestServer`] with the given router and state.
    pub async fn create_test_server_with_state(
        router: ApiRouter<ServiceState>,
        state: ServiceState,
    ) -> anyhow::Result<TestServer> {
        let app = router.with_state(state);
        let app = Into::<Router>::into(app);
        let server = TestServer::new(app)?;
        Ok(server)
    }

    /// Returns a new [`TestServer`] with the default router and state.
    pub async fn create_test_server() -> anyhow::Result<TestServer> {
        let config = ServiceConfig::default();
        let ai_services = nvisy_test::create_mock_services();
        let state = ServiceState::from_config(config, ai_services).await?;
        let router = routes(CustomRoutes::new(), state.clone());
        create_test_server_with_state(router, state).await
    }

    #[tokio::test]
    async fn handlers() -> anyhow::Result<()> {
        let server = create_test_server().await?;
        assert!(server.is_running());
        Ok(())
    }
}
