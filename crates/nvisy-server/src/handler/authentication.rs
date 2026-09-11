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
use nvisy_postgres::model::{NewAccount, NewAccountIdentity};
use nvisy_postgres::query::{
    AccountApiTokenRepository, AccountIdentityRepository, AccountRepository,
};
use nvisy_postgres::types::IdentityProvider;
use nvisy_postgres::{AsyncConnection, Error as PgError, PgClient};

use super::request::{Login, Signup};
use crate::extract::{AuthState, Json, SecurityContext, ValidateJson};
use crate::handler::utility::build_password_user_inputs;
use crate::response::{ClearedSession, CookieConfig, ErrorKind, ErrorResponse, Result, WebSession};
use crate::service::{AuthIssuer, PasswordService, ServiceState};

/// Tracing target for authentication operations.
const TRACING_TARGET: &str = "nvisy_server::handler::authentication";

/// Tracing target for authentication cleanup operations.
const TRACING_TARGET_CLEANUP: &str = "nvisy_server::handler::authentication::cleanup";

/// Creates a new account API token (login).
#[tracing::instrument(skip_all)]
async fn login(
    State(pg_client): State<PgClient>,
    State(password): State<PasswordService>,
    State(issuer): State<AuthIssuer>,
    State(cookie): State<CookieConfig>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<Login>,
) -> Result<WebSession> {
    tracing::debug!(target: TRACING_TARGET, "Login attempt");

    let mut conn = pg_client.get_connection().await?;
    let account = conn.find_account_by_identifier(&request.identifier).await?;

    // The password hash lives on the account's password identity, not the account.
    // An account with no password identity (OIDC-only) cannot log in by password.
    let password_secret = match &account {
        Some(acc) => conn
            .find_account_identity(acc.id, IdentityProvider::Password)
            .await?
            .and_then(|identity| identity.secret),
        None => None,
    };

    // Always perform a hash verification (a dummy when there is no account or no
    // password identity) to keep timing constant and prevent account enumeration.
    let password_valid = match &password_secret {
        Some(secret) => password.verify(&request.password, secret).is_ok(),
        None => password.verify_dummy(&request.password),
    };

    // Check for login failures and return appropriate errors
    let account = match account {
        None => {
            tracing::warn!(target: TRACING_TARGET, reason = "account_not_found", "Login failed");
            return Err(ErrorKind::Unauthorized
                .with_resource("credentials")
                .with_message("Invalid credentials"));
        }
        Some(_) if !password_valid => {
            tracing::warn!(target: TRACING_TARGET, reason = "invalid_password", "Login failed");
            return Err(ErrorKind::Unauthorized
                .with_resource("credentials")
                .with_message("Invalid credentials"));
        }
        Some(acc) if acc.is_suspended() => {
            tracing::warn!(target: TRACING_TARGET, reason = "account_suspended", "Login failed");
            return Err(ErrorKind::Forbidden
                .with_resource("account")
                .with_message("Account is suspended"));
        }
        Some(acc) if acc.is_deleted() => {
            tracing::warn!(target: TRACING_TARGET, reason = "account_deleted", "Login failed");
            return Err(ErrorKind::Forbidden
                .with_resource("account")
                .with_message("Account has been deleted"));
        }
        Some(acc) => acc,
    };

    let jwt = issuer
        .issue_web_session(&mut conn, &account, request.remember_me, security)
        .await?;

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
    State(pg_client): State<PgClient>,
    State(password): State<PasswordService>,
    State(issuer): State<AuthIssuer>,
    State(cookie): State<CookieConfig>,
    security: SecurityContext,
    ValidateJson(request): ValidateJson<Signup>,
) -> Result<WebSession> {
    tracing::debug!(target: TRACING_TARGET, "Signing up");

    // Validate password strength and hash
    let user_inputs = build_password_user_inputs(
        request.username.as_str(),
        request.display_name.as_deref(),
        &request.email_address,
    );
    let password_hash = password.validate_and_hash(&request.password, &user_inputs)?;

    let mut conn = pg_client.get_connection().await?;

    // Reject duplicate email or username before insert for a clear error;
    // the unique indexes remain the race-safe backstop.
    if conn.email_exists(&request.email_address).await? {
        tracing::warn!(target: TRACING_TARGET, "Signup failed: email already exists");
        return Err(ErrorKind::Conflict.with_message("Email is already registered"));
    }
    if conn.username_exists(&request.username).await? {
        tracing::warn!(target: TRACING_TARGET, "Signup failed: username already taken");
        return Err(ErrorKind::Conflict.with_message("Handle is already taken"));
    }

    let new_account = NewAccount {
        username: request.username,
        display_name: request.display_name,
        email_address: request.email_address,
        avatar_url: None,
        timezone: None,
        locale: None,
    };

    // Create the account and its password identity together: an account must
    // never exist without a way to authenticate, and the password hash lives on
    // the identity, not the account.
    let account = conn
        .transaction(async |conn| {
            let account = conn.create_account(new_account).await?;
            conn.create_account_identity(NewAccountIdentity::password(account.id, password_hash))
                .await?;
            Ok::<_, PgError>(account)
        })
        .await?;

    tracing::info!(
        target: TRACING_TARGET,
        account_id = %account.id,
        "Account created",
    );

    let jwt = issuer
        .issue_web_session(&mut conn, &account, request.remember_me, security)
        .await?;

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
    State(pg_client): State<PgClient>,
    State(cookie): State<CookieConfig>,
    auth_state: AuthState,
) -> Result<Response> {
    tracing::debug!(target: TRACING_TARGET, "Logging out");

    let mut conn = pg_client.get_connection().await?;

    // Verify API token exists before attempting to delete
    let token_exists = conn
        .find_account_api_token_by_id(auth_state.token_id)
        .await?
        .is_some();

    // Whatever the outcome, clear the browser session and CSRF cookies. A bearer
    // client simply has no cookies to clear and ignores them; a cookie client is
    // logged out on the client side too. Revocation is authoritative server-side
    // via the token soft-delete below.
    let cleared = ClearedSession::new(cookie).into_jar();

    if !token_exists {
        tracing::warn!(target: TRACING_TARGET, "Logout attempted on non-existent token");
        // Consider it successful if the token doesn't exist.
        return Ok((StatusCode::OK, cleared).into_response());
    }

    // Delete the API token (revocation: the row is the session authority).
    let deleted = conn.delete_account_api_token(auth_state.token_id).await?;

    if deleted {
        tracing::info!(target: TRACING_TARGET, "Logout successful");
    } else {
        tracing::warn!(target: TRACING_TARGET, "Logout completed but token was not found");
    }

    // Opportunistically clean up expired sessions for this account. Run inline on
    // the request's connection so a pooled slot is not pinned to a detached task;
    // this is best-effort, so a failure is only logged.
    if let Err(e) = conn.cleanup_expired_account_api_tokens().await {
        tracing::debug!(
            target: TRACING_TARGET_CLEANUP,
            error = %e,
            "Failed to cleanup expired sessions during logout"
        );
    }

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
    use aide::axum::routing::*;

    ApiRouter::new()
        .api_route("/auth/login/", post_with(login, login_docs))
        .api_route("/auth/signup/", post_with(signup, signup_docs))
        .with_path_items(|item| item.tag("Authentication"))
}

/// Authenticated authentication routes: logout, which revokes the caller's
/// session and so must sit behind the authentication and CSRF layers (it is a
/// cookie-driven state change).
pub fn private_routes() -> ApiRouter<ServiceState> {
    use aide::axum::routing::*;

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

        let beyond_cap = now - Span::new().seconds(session::MAX_AGE.as_secs() as i64 + 3600);
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

        let stale = now - Span::new().seconds(session::SLIDE_THROTTLE.as_secs() as i64 + 60);
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
    /// issued far longer ago than the web MAX_AGE and never used.
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
