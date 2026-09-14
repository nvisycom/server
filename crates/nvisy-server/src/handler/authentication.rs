//! Authentication handlers for user login and registration.
//!
//! This module provides secure authentication endpoints including user login,
//! registration (signup), and logout functionality. All authentication operations
//! follow security best practices including:

use aide::axum::ApiRouter;
use aide::transform::TransformOperation;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use super::request::{Login, Signup};
use crate::extract::{AuthState, Json, SecurityContext, ValidateJson};
use crate::response::{ClearedSession, CookieConfig, ErrorResponse, Result, WebSession};
use crate::service::{ServiceState, SignInService};

/// Tracing target for authentication operations.
const TRACING_TARGET: &str = "nvisy_server::handler::authentication";

/// Creates a new account API token (login).
#[tracing::instrument(skip_all)]
async fn login(
    State(sign_in): State<SignInService>,
    State(cookie): State<CookieConfig>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<Login>,
) -> Result<WebSession> {
    tracing::debug!(target: TRACING_TARGET, "Login attempt");

    let jwt = sign_in.login(request, security).await?;
    Ok(WebSession::new(jwt, cookie))
}

fn login_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Login")
        .description(
            "Authenticates a user and starts a browser session: sets an HttpOnly session cookie \
             and a CSRF cookie, returning no body. (Programmatic clients authenticate with an API \
             token created via the tokens endpoint, not this one.)",
        )
        .response::<204, ()>()
        .response::<400, Json<ErrorResponse>>()
        .response::<401, Json<ErrorResponse>>()
}

/// Creates a new account and API token (signup).
#[tracing::instrument(skip_all)]
async fn signup(
    State(sign_in): State<SignInService>,
    State(cookie): State<CookieConfig>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<Signup>,
) -> Result<WebSession> {
    tracing::debug!(target: TRACING_TARGET, "Signing up");

    let jwt = sign_in.signup(request, security).await?;
    Ok(WebSession::new(jwt, cookie))
}

fn signup_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Signup")
        .description(
            "Creates a new account and starts a browser session: sets an HttpOnly session cookie \
             and a CSRF cookie, returning no body.",
        )
        .response::<204, ()>()
        .response::<400, Json<ErrorResponse>>()
        .response::<409, Json<ErrorResponse>>()
}

/// Deletes an API token by its ID (logout).
#[tracing::instrument(
    skip_all,
    fields(
        account_id = %auth_state.account_id,
        token_id = %auth_state.token_id,
    )
)]
async fn logout(
    State(sign_in): State<SignInService>,
    State(cookie): State<CookieConfig>,
    auth_state: AuthState,
) -> Result<Response> {
    tracing::debug!(target: TRACING_TARGET, "Logging out");

    // Revoke the session server-side (the token row is the authority). A logout on
    // a token that no longer exists is still a success — there is nothing to
    // revoke — so the outcome does not change the response.
    sign_in.logout(auth_state.token_id).await?;

    // Whatever the outcome, clear the browser session and CSRF cookies. A bearer
    // client simply has no cookies to clear and ignores them; a cookie client is
    // logged out on the client side too.
    let cleared = ClearedSession::new(cookie).into_jar();
    Ok((StatusCode::OK, cleared).into_response())
}

fn logout_docs(op: TransformOperation) -> TransformOperation {
    op.summary("Logout")
        .description("Invalidates the current session and clears session cookies.")
        .response_with::<200, (), _>(|res| res.description("Logged out."))
        .response::<401, Json<ErrorResponse>>()
}

/// Public authentication routes: login and signup, which a caller with no
/// session reaches before authenticating.
pub fn public_routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::post_with;

    ApiRouter::new()
        .api_route("/auth/login/", post_with(login, login_docs))
        .api_route("/auth/signup/", post_with(signup, signup_docs))
        .with_path_items(|item| item.tag("Authentication"))
}

/// Authenticated authentication routes: logout, which revokes the caller's
/// session and so must sit behind the authentication and CSRF layers (it is a
/// cookie-driven state change).
pub fn private_routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::post_with;

    ApiRouter::new()
        .api_route("/auth/logout/", post_with(logout, logout_docs))
        .with_path_items(|item| item.tag("Authentication"))
}

#[cfg(test)]
mod tests {
    use jiff::{Span, Timestamp};
    use nvisy_postgres::model::{NewAccount, NewAccountApiToken, UpdateAccountApiToken};
    use nvisy_postgres::query::{AccountApiTokenRepository, AccountRepository};
    use nvisy_postgres::types::{ApiTokenType, Handle, session};
    use nvisy_postgres::{JiffTimestamp, PgClient, PgConfig, PgConn};
    use uuid::Uuid;

    /// A throwaway session token on a throwaway account, for exercising the
    /// session-lifecycle checks against a live database. Cleaned up with
    /// [`Self::cleanup`].
    struct SessionFixture {
        conn: PgConn,
        account_id: Uuid,
        token_id: Uuid,
    }

    impl SessionFixture {
        /// Creates the fixture: a fresh account and a not-remembered `web` session
        /// token.
        async fn create() -> anyhow::Result<Self> {
            Self::create_with(ApiTokenType::Web, session::initial_expires_at(false)).await
        }

        /// Like [`create`](Self::create) but for an `app` token with the given
        /// idle/expiry bound — a native-app session.
        async fn create_app(expired_at: Timestamp) -> anyhow::Result<Self> {
            Self::create_with(ApiTokenType::App, expired_at).await
        }

        async fn create_with(
            session_type: ApiTokenType,
            expired_at: Timestamp,
        ) -> anyhow::Result<Self> {
            dotenvy::dotenv().ok();
            let pg = PgClient::new(PgConfig::new(std::env::var("POSTGRES_URL")?))?;
            let mut conn = pg.get_connection().await?;

            let suffix = Uuid::now_v7().simple().to_string();
            let account = conn
                .create_account(NewAccount {
                    username: Handle::parse(format!("sesstest-{}", &suffix[..8]))?,
                    display_name: None,
                    email_address: format!("sesstest-{suffix}@example.test"),
                    avatar_url: None,
                    timezone: None,
                    locale: None,
                })
                .await?;
            let token = conn
                .create_account_api_token(NewAccountApiToken {
                    account_id: account.id,
                    display_name: "session test".to_owned(),
                    is_remembered: Some(false),
                    session_type: Some(session_type),
                    expired_at: Some(JiffTimestamp::from(expired_at)),
                    ..Default::default()
                })
                .await?;

            Ok(Self {
                conn,
                account_id: account.id,
                token_id: token.id,
            })
        }

        /// Overwrites the token's time columns to simulate elapsed time.
        async fn set_times(
            &mut self,
            issued_at: Timestamp,
            expired_at: Timestamp,
            last_used_at: Option<Timestamp>,
        ) -> anyhow::Result<()> {
            self.conn
                .update_account_api_token(
                    self.token_id,
                    UpdateAccountApiToken {
                        issued_at: Some(JiffTimestamp::from(issued_at)),
                        expired_at: Some(Some(JiffTimestamp::from(expired_at))),
                        last_used_at: Some(last_used_at.map(JiffTimestamp::from)),
                        ..Default::default()
                    },
                )
                .await?;
            Ok(())
        }

        async fn is_active(&mut self) -> anyhow::Result<bool> {
            Ok(self
                .conn
                .account_api_token_is_active(self.token_id, self.account_id, session::MAX_AGE)
                .await?)
        }

        async fn slide(&mut self) -> anyhow::Result<bool> {
            Ok(self
                .conn
                .slide_account_api_token(self.token_id, session::SlidingWindow::standard())
                .await?)
        }

        async fn cleanup(mut self) -> anyhow::Result<()> {
            self.conn.delete_account_api_token(self.token_id).await?;
            self.conn.delete_account(self.account_id).await?;
            Ok(())
        }
    }

    fn days(d: i64) -> Span {
        Span::new().hours(d * 24)
    }

    /// A fresh session (issued now, idle bound in the future) is active; one past
    /// its idle bound is not, even though issued recently and not revoked.
    #[tokio::test]
    #[ignore = "requires database and key files"]
    async fn idle_bound_governs_activity() -> anyhow::Result<()> {
        let mut fixture = SessionFixture::create().await?;
        let now = Timestamp::now();

        fixture.set_times(now, now + days(1), None).await?;
        assert!(fixture.is_active().await?, "an unexpired session is active");

        fixture.set_times(now, now - days(1), None).await?;
        assert!(
            !fixture.is_active().await?,
            "a session past its idle bound is inactive"
        );

        fixture.cleanup().await
    }

    /// A session issued longer ago than the absolute cap is rejected regardless of
    /// a healthy (future) idle bound.
    #[tokio::test]
    #[ignore = "requires database and key files"]
    async fn absolute_cap_governs_activity() -> anyhow::Result<()> {
        let mut fixture = SessionFixture::create().await?;
        let now = Timestamp::now();

        let beyond_cap = now - Span::new().seconds(session::MAX_AGE_SECS + 3600);
        fixture.set_times(beyond_cap, now + days(1), None).await?;
        assert!(
            !fixture.is_active().await?,
            "a session past the absolute age cap is inactive even if not idle"
        );

        fixture.cleanup().await
    }

    /// A recently-used session does not slide (throttled); a session last used
    /// before the throttle window slides forward, advancing its idle bound.
    #[tokio::test]
    #[ignore = "requires database and key files"]
    async fn slide_is_throttled_then_advances() -> anyhow::Result<()> {
        let mut fixture = SessionFixture::create().await?;
        let now = Timestamp::now();

        fixture.set_times(now, now + days(1), Some(now)).await?;
        assert!(
            !fixture.slide().await?,
            "a session used within the throttle window does not slide"
        );

        let stale = now - Span::new().seconds(session::SLIDE_THROTTLE_SECS + 60);
        fixture
            .set_times(now, now + Span::new().hours(1), Some(stale))
            .await?;
        assert!(fixture.slide().await?, "a stale session slides");

        let after = fixture
            .conn
            .find_account_api_token_by_id(fixture.token_id)
            .await?
            .expect("token exists");
        let after_expired: Timestamp = after.expired_at.expect("has idle bound").into();
        assert!(
            after_expired > now + Span::new().hours(1),
            "the idle bound advances past its prior value after a slide"
        );

        fixture.cleanup().await
    }

    /// An `app` (desktop) token does not slide and is not subject to the browser
    /// absolute cap: it stays valid on its own long `expired_at` even when it was
    /// issued far longer ago than the web `MAX_AGE` and never used.
    #[tokio::test]
    #[ignore = "requires database and key files"]
    async fn app_token_does_not_slide_and_ignores_absolute_cap() -> anyhow::Result<()> {
        let now = Timestamp::now();
        // Idle bound a year out, as `mint_app_token` sets.
        let mut fixture = SessionFixture::create_app(now + days(365)).await?;

        // Issued longer ago than the browser absolute cap, with a far-future idle
        // bound and no recent use: a `web` session would be rejected by the cap and
        // would slide, but an `app` token is exempt from both.
        let long_ago = now - days(400);
        fixture.set_times(long_ago, now + days(365), None).await?;

        assert!(
            fixture.is_active().await?,
            "an app token past the web absolute cap is still active on its own expiry"
        );
        assert!(!fixture.slide().await?, "an app token must not slide");

        // The idle bound is unchanged (no slide wrote a shorter window over it).
        let after = fixture
            .conn
            .find_account_api_token_by_id(fixture.token_id)
            .await?
            .expect("token exists");
        let after_expired: Timestamp = after.expired_at.expect("has expiry").into();
        assert!(
            after_expired > now + days(300),
            "the app token's long expiry is not overwritten by a slide"
        );

        fixture.cleanup().await
    }

    /// `prune_app_tokens` keeps only the newest `keep` live app tokens for an
    /// account and revokes the rest, and never touches web tokens.
    #[tokio::test]
    #[ignore = "requires database and key files"]
    async fn prune_app_tokens_keeps_newest_and_spares_web() -> anyhow::Result<()> {
        dotenvy::dotenv().ok();
        let pg = PgClient::new(PgConfig::new(std::env::var("POSTGRES_URL")?))?;
        let mut conn = pg.get_connection().await?;

        let suffix = Uuid::now_v7().simple().to_string();
        let account = conn
            .create_account(NewAccount {
                username: Handle::parse(format!("pruntest-{}", &suffix[..8]))?,
                display_name: None,
                email_address: format!("pruntest-{suffix}@example.test"),
                avatar_url: None,
                timezone: None,
                locale: None,
            })
            .await?;

        let now = Timestamp::now();
        // Five app tokens with staggered issue times (newest last), plus one web
        // session that must survive pruning untouched.
        let mut app_ids = Vec::new();
        for i in 0..5 {
            let token = conn
                .create_account_api_token(NewAccountApiToken {
                    account_id: account.id,
                    display_name: format!("app {i}"),
                    session_type: Some(ApiTokenType::App),
                    expired_at: Some(JiffTimestamp::from(now + days(365))),
                    ..Default::default()
                })
                .await?;
            // Backdate issued_at so ordering is deterministic (i=0 oldest).
            conn.update_account_api_token(
                token.id,
                UpdateAccountApiToken {
                    issued_at: Some(JiffTimestamp::from(now - days(5 - i))),
                    ..Default::default()
                },
            )
            .await?;
            app_ids.push(token.id);
        }
        let web = conn
            .create_account_api_token(NewAccountApiToken {
                account_id: account.id,
                display_name: "web".to_owned(),
                session_type: Some(ApiTokenType::Web),
                expired_at: Some(session::initial_expires_at(false).into()),
                ..Default::default()
            })
            .await?;

        // Keep the 2 newest app tokens; the 3 oldest are revoked.
        let revoked = conn.prune_app_tokens(account.id, 2).await?;
        assert_eq!(revoked, 3, "the three oldest app tokens are revoked");

        // The two newest app tokens survive; the oldest three are gone.
        for (i, id) in app_ids.iter().enumerate() {
            let alive = conn.find_account_api_token_by_id(*id).await?.is_some();
            assert_eq!(alive, i >= 3, "app token {i} liveness after prune");
        }
        // The web session is untouched by app-token pruning.
        assert!(
            conn.find_account_api_token_by_id(web.id).await?.is_some(),
            "pruning app tokens must not revoke web sessions"
        );

        conn.delete_all_account_api_tokens(account.id).await?;
        conn.delete_account(account.id).await?;
        Ok(())
    }
}
