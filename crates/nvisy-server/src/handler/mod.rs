//! All `axum::`[`Router`]s with related `axum::`[`Handler`]s.
//!
//! [`Router`]: axum::routing::Router
//! [`Handler`]: axum::handler::Handler

mod accounts;
mod activities;
mod analytics;
mod assignments;
mod auth_oidc;
mod authentication;

pub(crate) use auth_oidc::consume_reauth_proof;
mod avatars;
mod catalog;
mod chat;
mod comments;
mod connection_oauth;
mod connection_syncs;
mod connections;
mod detection_audits;
mod detections;
mod files;
mod identities;
mod invites;
mod members;
mod monitors;
mod notifications;
mod pipelines;
mod policies;
mod providers;
mod redactions;
pub mod request;
pub mod response;
mod tokens;
mod utility;
mod webhooks;
mod workspaces;

use aide::axum::ApiRouter;
use axum::extract::FromRef;
use axum::http::{Method, Uri};
use axum::middleware::{from_fn, from_fn_with_state};
use axum::response::{IntoResponse, Response};
pub use invites::{CreatedInvite, InviteOutcome, create_invite};
pub use utility::CustomRoutes;

use crate::middleware::{csrf_protect, require_authentication, slide_session};
use crate::response::ErrorKind;
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

/// Returns an [`ApiRouter`] with all built-in private routes. Downstream routes
/// are merged separately by [`routes`], after this built-in router is erased to
/// the caller's state type.
fn private_routes(service_state: ServiceState) -> ApiRouter<ServiceState> {
    ApiRouter::new()
        .merge(accounts::routes(service_state.clone()))
        .merge(workspaces::routes())
        .merge(activities::routes())
        .merge(analytics::routes())
        .merge(members::routes())
        .merge(assignments::routes())
        .merge(comments::routes())
        .merge(connections::routes())
        .merge(providers::routes())
        .merge(connection_oauth::private_routes())
        .merge(chat::routes())
        .merge(connection_syncs::routes())
        .merge(files::routes(service_state.upload.max_file_body_bytes))
        .merge(pipelines::routes())
        .merge(detections::routes())
        .merge(detection_audits::routes())
        .merge(redactions::routes())
        .merge(policies::routes())
        .merge(catalog::routes())
        .merge(tokens::routes())
        .merge(notifications::routes())
        .merge(invites::routes())
        .merge(webhooks::routes())
        // Account identity management and OIDC step-up re-auth.
        .merge(identities::routes())
        .merge(auth_oidc::private_routes())
        // Logout revokes the caller's session (a cookie-driven state change), so
        // it sits behind the authentication and CSRF layers.
        .merge(authentication::authenticated_routes())
}

/// Returns an [`ApiRouter`] with all built-in public routes. Downstream routes
/// are merged separately by [`routes`].
fn public_routes() -> ApiRouter<ServiceState> {
    ApiRouter::new()
        // Public authentication: password login/signup plus OIDC sign-in. These
        // need no session — the caller has none yet and the provider's browser
        // redirect carries no Authorization header. Logout is authenticated (it
        // revokes a session), so it is in the private routes; the authenticated
        // OIDC link route is there too.
        .merge(authentication::public_routes())
        .merge(auth_oidc::public_routes())
        .merge(monitors::routes())
        // Avatar serving is public so images load directly in an `<img>` tag; it
        // is infrastructure shared by accounts and workspaces, always mounted.
        .merge(avatars::routes())
        // The cloud file OAuth callback is a provider browser redirect with no
        // Authorization header, so it is public; its single-use CSRF state guards
        // it.
        .merge(connection_oauth::public_routes())
}

/// Returns an [`ApiRouter`] with all routes, over any application state `S` from
/// which this crate's [`ServiceState`] can be extracted.
///
/// Built-in handlers extract their services from [`ServiceState`], so they are
/// assembled and layered as an `ApiRouter<ServiceState>`, then erased to
/// `ApiRouter<S>` with [`with_state`](ApiRouter::with_state) once the state is
/// baked in. A wrapping binary embeds `ServiceState` inside its own `S` (via a
/// [`FromRef`] impl) and contributes its own `ApiRouter<S>` routes through
/// [`CustomRoutes`], which are merged after the erasure — so built-in routes and
/// downstream routes sit side by side under one final state type.
///
/// For the common case the binary uses `ServiceState` directly (`S =
/// ServiceState`), which satisfies the bound via axum's reflexive `FromRef`.
pub fn routes<S>(mut routes: CustomRoutes<S>, state: S) -> ApiRouter<S>
where
    S: Clone + Send + Sync + 'static,
    ServiceState: FromRef<S>,
{
    let service_state = ServiceState::from_ref(&state);

    // Auth middleware extracts from `ServiceState`; the layer captures its own
    // state, independent of the router's `S`.
    let require_authentication = from_fn_with_state(service_state.clone(), require_authentication);
    let slide_session = from_fn_with_state(service_state.clone(), slide_session);

    // Built-in private routes are assembled and their map hooks applied while
    // still typed to `ServiceState`, then erased to `S` and merged with the
    // downstream's private routes. The auth `route_layer`s are applied to the
    // *combined* router, so custom private routes are authenticated too — a
    // `route_layer` only covers routes already present when it runs.
    let private_router = private_routes(service_state.clone());
    let mut private_router: ApiRouter<S> = private_router.with_state(service_state.clone());
    if let Some(additional) = routes.private_routes.take() {
        private_router = private_router.merge(additional);
    }
    // Layer order matters, and is security-relevant. `route_layer`s apply
    // bottom-up, so the LAST one added is the OUTERMOST (runs first). We want, per
    // request, in order:
    //   1. require_authentication — resolves and caches the verified `SessionToken`
    //      (which records the transport), rejecting an unauthenticated request,
    //   2. csrf_protect — enforces CSRF on cookie-authed state-changing requests,
    //      reading the transport cached above; rejects a forged request here,
    //   3. slide_session — extends the session (a DB write).
    // CSRF runs BEFORE the slide so a CSRF-forged cookie request is rejected
    // without performing the session-extending write. So `slide_session` is added
    // first (innermost, runs last), then `csrf_protect`, then
    // `require_authentication` outermost.
    private_router = private_router
        .route_layer(slide_session)
        .route_layer(from_fn(csrf_protect))
        .route_layer(require_authentication);

    // Built-in public routes, same erasure (no auth layers).
    let mut public_router: ApiRouter<S> = public_routes().with_state(service_state);
    if let Some(additional) = routes.public_routes.take() {
        public_router = public_router.merge(additional);
    }

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
    use crate::middleware::UploadConfig;
    use crate::response::CookieConfig;
    use crate::service::{
        CryptoConfig, EngineConfig, FileConnectorsConfig, HealthConfig, IntegrationConfig,
        OidcConfig, S3Config, ServiceState, SessionKeysConfig,
    };

    /// Builds the service sub-configs from the environment for integration tests.
    fn configs_from_env() -> anyhow::Result<(
        PgConfig,
        NatsConfig,
        SessionKeysConfig,
        CryptoConfig,
        S3Config,
    )> {
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

        let s3 = S3Config {
            bucket: var("S3_BUCKET")?,
            region: var("S3_REGION").unwrap_or_else(|_| "us-east-1".to_owned()),
            endpoint: var("S3_ENDPOINT").ok(),
            force_path_style: var("S3_FORCE_PATH_STYLE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(true),
            access_key_id: var("S3_ACCESS_KEY_ID").ok(),
            secret_access_key: var("S3_SECRET_ACCESS_KEY").ok(),
        };

        Ok((postgres, nats, session, crypto, s3))
    }

    /// Returns a new [`TestServer`] with the given router.
    pub async fn create_test_server_with_router(
        router: impl Fn(ServiceState) -> ApiRouter<ServiceState>,
    ) -> anyhow::Result<TestServer> {
        let (postgres, nats, session, crypto, s3) = configs_from_env()?;
        let webhook_service = ReqwestClient::default().into_service();
        let state = ServiceState::from_config(
            postgres,
            nats,
            session,
            crypto,
            EngineConfig::default(),
            HealthConfig::default(),
            IntegrationConfig::default(),
            FileConnectorsConfig::default(),
            OidcConfig::default(),
            webhook_service,
            UploadConfig::default(),
            CookieConfig::default(),
            s3,
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
    async fn custom_private_routes_require_authentication() -> anyhow::Result<()> {
        use aide::axum::routing::get_with;

        use crate::extract::Json;
        use crate::handler::response::InviteSent;

        // A custom private route mounted via `add_private_routes`. It must be
        // covered by the auth layers just like the built-in private routes: the
        // auth `route_layer`s are applied to the merged router, so an
        // unauthenticated request is rejected before reaching the handler.
        let custom = ApiRouter::new().api_route(
            "/custom/private/",
            get_with(
                || async { Json(InviteSent::new()) },
                |op| op.summary("custom private route"),
            ),
        );

        let server = create_test_server_with_router(move |state| {
            routes(
                CustomRoutes::new().add_private_routes(custom.clone()),
                state,
            )
        })
        .await?;

        // No Authorization header -> 401, proving the custom route is protected.
        let response = server.get("/custom/private/").await;
        response.assert_status_unauthorized();
        Ok(())
    }
}
