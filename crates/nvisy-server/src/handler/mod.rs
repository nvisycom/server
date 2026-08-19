//! All `axum::`[`Router`]s with related `axum::`[`Handler`]s.
//!
//! [`Router`]: axum::routing::Router
//! [`Handler`]: axum::handler::Handler

mod accounts;
mod authentication;
mod avatars;
mod catalog;
mod chat;
mod connection_syncs;
mod connections;
mod error;
mod files;
mod invites;
mod members;
mod monitors;
mod notifications;
mod pipeline_audits;
mod pipeline_runs;
mod pipelines;
mod policies;
pub mod request;
pub mod response;
mod tokens;
mod utility;
mod webhooks;
mod workspaces;

use std::collections::HashSet;

use aide::axum::ApiRouter;
use axum::http::{Method, Uri};
use axum::middleware::from_fn_with_state;
use axum::response::{IntoResponse, Response};
pub use error::{Error, ErrorKind, Result};
pub use invites::{CreatedInvite, InviteOutcome, create_invite};
pub use utility::{BuiltinModule, CustomRoutes, RouterMapFn};

use crate::middleware::{require_authentication, validate_token_middleware};
use crate::service::ServiceState;

/// Tracing target for unmatched-route fallbacks.
const TRACING_TARGET_FALLBACK: &str = "nvisy_server::handler::fallback";

/// Fallback for requests that match no route.
///
/// Logs the method and path at debug so unmatched requests are diagnosable when
/// debugging, without warning on routine client probes, and echoes the path in
/// the response context.
async fn handler(method: Method, uri: Uri) -> Response {
    tracing::debug!(
        target: TRACING_TARGET_FALLBACK,
        %method,
        path = %uri.path(),
        "No route matched request"
    );

    ErrorKind::NotFound
        .with_resource("route")
        .with_context(format!("No route matches {method} {}", uri.path()))
        .into_response()
}

/// Returns an [`ApiRouter`] with all private routes, minus any excluded module.
fn private_routes(
    additional_routes: Option<ApiRouter<ServiceState>>,
    excluded: &HashSet<BuiltinModule>,
    service_state: ServiceState,
) -> ApiRouter<ServiceState> {
    let mut router = ApiRouter::new();

    // Only a few modules are toggleable, letting a wrapping binary replace them
    // via `CustomRoutes` without a route collision. The rest are core to the
    // platform and always mounted.
    let is_included = |module| !excluded.contains(&module);

    // Always-wired core modules.
    router = router
        .merge(accounts::routes(service_state.clone()))
        .merge(workspaces::routes())
        .merge(members::routes())
        .merge(connections::routes())
        .merge(chat::routes())
        .merge(connection_syncs::routes())
        .merge(files::routes())
        .merge(pipelines::routes())
        .merge(pipeline_runs::routes())
        .merge(pipeline_audits::routes())
        .merge(policies::routes())
        .merge(catalog::routes());

    // Toggleable modules.
    if is_included(BuiltinModule::Tokens) {
        router = router.merge(tokens::routes());
    }
    if is_included(BuiltinModule::Notifications) {
        router = router.merge(notifications::routes());
    }
    if is_included(BuiltinModule::Invites) {
        router = router.merge(invites::routes());
    }
    if is_included(BuiltinModule::Webhooks) {
        router = router.merge(webhooks::routes());
    }

    if let Some(additional) = additional_routes {
        router = router.merge(additional);
    }

    router
}

/// Returns an [`ApiRouter`] with all public routes, minus any excluded module.
fn public_routes(
    additional_routes: Option<ApiRouter<ServiceState>>,
    excluded: &HashSet<BuiltinModule>,
    _service_state: ServiceState,
    disable_authentication: bool,
) -> ApiRouter<ServiceState> {
    let mut router = ApiRouter::new();

    if !disable_authentication && !excluded.contains(&BuiltinModule::Authentication) {
        router = router.merge(authentication::routes());
    }

    router = router.merge(monitors::routes());

    // Avatar serving is public so images load directly in an `<img>` tag; it is
    // infrastructure shared by accounts and workspaces, always mounted.
    router = router.merge(avatars::routes());

    if let Some(additional) = additional_routes {
        router = router.merge(additional);
    }

    router
}

/// Returns an [`ApiRouter`] with all routes.
pub fn routes(mut routes: CustomRoutes, state: ServiceState) -> ApiRouter<ServiceState> {
    let require_authentication = from_fn_with_state(state.clone(), require_authentication);
    let validate_token_middleware = from_fn_with_state(state.clone(), validate_token_middleware);

    let excluded = std::mem::take(&mut routes.excluded_modules);

    // Private routes.
    let mut private_router = private_routes(routes.private_routes.take(), &excluded, state.clone());
    private_router = routes.map_private_before_middleware(private_router);
    private_router = private_router
        .route_layer(require_authentication)
        .route_layer(validate_token_middleware);
    private_router = routes.map_private_after_middleware(private_router);

    // Public routes.
    let mut public_router = public_routes(
        routes.public_routes.take(),
        &excluded,
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
    use nvisy_nats::NatsConfig;
    use nvisy_postgres::PgConfig;
    use nvisy_webhook::reqwest::ReqwestClient;

    use crate::handler::{CustomRoutes, routes};
    use crate::service::{
        CryptoConfig, EngineConfig, HealthConfig, ServiceState, SessionKeysConfig, SyncConfig,
    };

    /// Builds the service sub-configs from the environment for integration tests.
    fn configs_from_env() -> anyhow::Result<(PgConfig, NatsConfig, SessionKeysConfig, CryptoConfig)>
    {
        dotenvy::dotenv().ok();
        let var = std::env::var;

        let mut postgres = PgConfig::new(var("POSTGRES_URL")?);
        if let Ok(v) = var("POSTGRES_MAX_CONNECTIONS") {
            postgres = postgres.with_max_connections(v.parse()?);
        }

        let nats = NatsConfig::new(var("NATS_URL")?, var("NATS_TOKEN").unwrap_or_default());

        let session = SessionKeysConfig {
            decoding_key: var("AUTH_PUBLIC_PEM_FILEPATH")?.into(),
            encoding_key: var("AUTH_PRIVATE_PEM_FILEPATH")?.into(),
        };

        let crypto = CryptoConfig {
            key_path: var("ENCRYPTION_KEY_FILEPATH")?.into(),
        };

        Ok((postgres, nats, session, crypto))
    }

    /// Returns a new [`TestServer`] with the given router.
    pub async fn create_test_server_with_router(
        router: impl Fn(ServiceState) -> ApiRouter<ServiceState>,
    ) -> anyhow::Result<TestServer> {
        let (postgres, nats, session, crypto) = configs_from_env()?;
        let webhook_service = ReqwestClient::default().into_service();
        let state = ServiceState::from_config(
            postgres,
            nats,
            session,
            crypto,
            EngineConfig::default(),
            HealthConfig::default(),
            SyncConfig::default(),
            webhook_service,
        )
        .await?;
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
        Ok(TestServer::new(app))
    }

    /// Returns a new [`TestServer`] with the default router and state.
    pub async fn create_test_server() -> anyhow::Result<TestServer> {
        create_test_server_with_router(|state| routes(CustomRoutes::new(), state)).await
    }

    #[tokio::test]
    #[ignore = "requires database and key files"]
    async fn handlers() -> anyhow::Result<()> {
        let server = create_test_server().await?;
        assert!(server.is_running());
        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires database and key files"]
    async fn excluding_a_module_frees_its_path_for_a_replacement() -> anyhow::Result<()> {
        use aide::axum::routing::get_with;

        use crate::extract::Json;
        use crate::handler::BuiltinModule;
        use crate::handler::response::InviteSent;

        // A custom router that reuses one of the built-in invite paths. Merging
        // this alongside the built-in invites module would panic on the route
        // collision; excluding the module first must make it succeed.
        let custom = ApiRouter::new().api_route(
            "/workspaces/{workspaceSlug}/invites/",
            get_with(
                || async { Json(InviteSent::new()) },
                |op| op.summary("custom invites"),
            ),
        );

        let server = create_test_server_with_router(move |state| {
            routes(
                CustomRoutes::new()
                    .exclude(BuiltinModule::Invites)
                    .add_private_routes(custom.clone()),
                state,
            )
        })
        .await?;

        assert!(server.is_running());
        Ok(())
    }

    #[test]
    fn exclude_marks_only_the_named_module() {
        use crate::handler::BuiltinModule;

        let routes = CustomRoutes::new().exclude(BuiltinModule::Invites);
        assert!(routes.is_excluded(BuiltinModule::Invites));
        assert!(!routes.is_excluded(BuiltinModule::Tokens));
        assert!(!routes.is_excluded(BuiltinModule::Webhooks));
    }
}
