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
mod utils;
mod websocket;

use axum::middleware::from_fn_with_state;
use axum::response::{IntoResponse, Response};
use utoipa_axum::router::OpenApiRouter;

pub use crate::extract::Permission;
pub use crate::handler::error::{Error, ErrorKind, Result};
pub use crate::handler::request::Pagination;
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
        .merge(tokens::routes())
        .merge(projects::routes())
        .merge(integrations::routes())
        .merge(invites::routes())
        .merge(members::routes())
        .merge(pipelines::routes())
        .merge(templates::routes())
        .merge(websocket::routes())
        .merge(files::routes())
        .merge(documents::routes())
        .merge(comments::routes());

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
mod test_utils {
    //! Test utilities for handler tests.
    //!
    //! This module provides mock implementations of services defined in nvisy-core
    //! for use in unit and integration tests.

    use nvisy_core::emb::{
        EmbeddingData, EmbeddingProvider, EmbeddingRequest, EmbeddingResponse, EmbeddingService,
    };
    use nvisy_core::ocr::{
        OcrProvider, OcrService, Request as OcrRequest, Response as OcrResponse,
    };
    use nvisy_core::vlm::{
        BoxedStream, Request as VlmRequest, Response as VlmResponse, VlmProvider, VlmService,
    };
    use nvisy_core::{Result, ServiceHealth};

    /// Create a mock embedding service for testing.
    pub fn mock_emb_service() -> EmbeddingService {
        EmbeddingService::new(Box::new(MockEmbeddingProvider))
    }

    /// Create a mock OCR service for testing.
    pub fn mock_ocr_service() -> OcrService {
        OcrService::new(Box::new(MockOcrProvider))
    }

    /// Create a mock VLM service for testing.
    pub fn mock_vlm_service() -> VlmService {
        VlmService::new(Box::new(MockVlmProvider))
    }

    /// Mock embedding provider for testing.
    #[derive(Clone, Default)]
    pub struct MockEmbeddingProvider;

    #[async_trait::async_trait]
    impl EmbeddingProvider<(), ()> for MockEmbeddingProvider {
        async fn generate_embedding(
            &self,
            request: &EmbeddingRequest,
        ) -> Result<EmbeddingResponse> {
            let data = vec![EmbeddingData::new(vec![0.1, 0.2, 0.3], 0)];
            Ok(
                EmbeddingResponse::new(request.request_id, "mock-model".to_string())
                    .with_data(data),
            )
        }

        async fn health_check(&self) -> Result<ServiceHealth> {
            Ok(ServiceHealth::healthy())
        }
    }

    /// Mock OCR provider for testing.
    #[derive(Clone, Default)]
    pub struct MockOcrProvider;

    #[async_trait::async_trait]
    impl<Req, Resp> OcrProvider<Req, Resp> for MockOcrProvider
    where
        Req: Send + Sync + 'static,
        Resp: Send + Sync + Default + 'static,
    {
        async fn process_ocr(&self, request: OcrRequest<Req>) -> Result<OcrResponse<Resp>> {
            Ok(OcrResponse {
                response_id: uuid::Uuid::new_v4(),
                request_id: request.request_id,
                payload: Resp::default(),
                processing_time_ms: Some(100),
                timestamp: time::OffsetDateTime::now(),
                usage: Default::default(),
                metadata: Default::default(),
            })
        }

        async fn process_ocr_stream(
            &self,
            _request: OcrRequest<Req>,
        ) -> Result<nvisy_core::ocr::BoxedStream<OcrResponse<Resp>>> {
            Ok(Box::new(futures::stream::empty()))
        }

        async fn health_check(&self) -> Result<ServiceHealth> {
            Ok(ServiceHealth::healthy())
        }
    }

    /// Mock VLM provider for testing.
    #[derive(Clone, Default)]
    pub struct MockVlmProvider;

    #[async_trait::async_trait]
    impl<Req, Resp> VlmProvider<Req, Resp> for MockVlmProvider
    where
        Req: Send + Sync + 'static,
        Resp: Send + Sync + Default + 'static,
    {
        async fn process_vlm(&self, _request: &VlmRequest<Req>) -> Result<VlmResponse<Resp>> {
            Ok(VlmResponse {
                content: "Mock VLM response".to_string(),
                usage: None,
                finish_reason: Some("stop".to_string()),
                created: std::time::SystemTime::now(),
                confidence: Some(0.95),
                visual_analysis: None,
                metadata: Default::default(),
                payload: Resp::default(),
            })
        }

        async fn process_vlm_stream(
            &self,
            _request: &VlmRequest<Req>,
        ) -> Result<BoxedStream<VlmResponse<Resp>>> {
            Ok(Box::new(futures::stream::empty()))
        }

        async fn health_check(&self) -> Result<ServiceHealth> {
            Ok(ServiceHealth::healthy())
        }
    }
}

#[cfg(test)]
mod test {
    use axum_test::TestServer;
    use utoipa_axum::router::OpenApiRouter;

    use super::test_utils;
    use crate::handler::{CustomRoutes, openapi_routes};
    use crate::service::{ServiceConfig, ServiceState};

    /// Returns a new [`TestServer`] with the given router.
    pub async fn create_test_server_with_router(
        router: impl Fn(ServiceState) -> OpenApiRouter<ServiceState>,
    ) -> anyhow::Result<TestServer> {
        let config = ServiceConfig::default();
        let ai_services = nvisy_core::AiServices::new(
            test_utils::mock_emb_service(),
            test_utils::mock_ocr_service(),
            test_utils::mock_vlm_service(),
        );
        let state = ServiceState::from_config(config, ai_services).await?;
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
        let ai_services = nvisy_core::AiServices::new(
            test_utils::mock_emb_service(),
            test_utils::mock_ocr_service(),
            test_utils::mock_vlm_service(),
        );
        let state = ServiceState::from_config(config, ai_services).await?;
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
