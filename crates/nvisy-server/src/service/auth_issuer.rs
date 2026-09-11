//! Session-token issuance: the one place that turns an account into a signed JWT.
//!
//! [`AuthIssuer`] owns the outbound half of authentication — signing an
//! [`AuthClaims`] into a JWT with the server's [`SessionKeys`], and the session
//! flavors that create an `account_api_tokens` row and sign a credential for it
//! (browser `web` sessions and native-app `app` tokens). Every sign-in path and
//! the API-token endpoint funnel through it, so there is a single implementation
//! of "mint a credential" rather than the same claim-build-and-sign repeated per
//! handler.
//!
//! It does not create the `account_api_tokens` row for the API-token endpoint —
//! that row is shaped from the request there — but it signs the credential for
//! it via [`sign`](AuthIssuer::sign), so the signing step is never duplicated.

use jiff::{Span, Timestamp};
use nvisy_postgres::PgConn;
use nvisy_postgres::model::{Account, AccountApiToken, NewAccountApiToken};
use nvisy_postgres::query::AccountApiTokenRepository;
use nvisy_postgres::types::{ApiTokenType, session};

use crate::extract::{AuthClaims, SecurityContext};
use crate::response::Result;
use crate::service::{SessionKeys, UserAgentParser};

/// Tracing target for token issuance.
const TRACING_TARGET: &str = "nvisy_server::authentication";

/// Signs authentication credentials and mints session tokens.
///
/// Composed from the server's [`SessionKeys`] (JWT signing) and the
/// [`UserAgentParser`] (session display names). Cheap to clone — both are
/// `Arc`-backed handles — so it is resolved per request from [`ServiceState`].
///
/// [`ServiceState`]: crate::service::ServiceState
#[derive(Clone)]
pub struct AuthIssuer {
    session_keys: SessionKeys,
    user_agent_parser: UserAgentParser,
}

impl AuthIssuer {
    /// Composes the issuer from its collaborators.
    #[inline]
    pub const fn new(session_keys: SessionKeys, user_agent_parser: UserAgentParser) -> Self {
        Self {
            session_keys,
            user_agent_parser,
        }
    }

    /// Signs a JWT credential for `token`, belonging to `account`.
    ///
    /// The single signing step behind every credential the server issues — a
    /// session cookie, a native-app token, or an API token — so the claims are
    /// built and signed in exactly one place.
    ///
    /// # Errors
    ///
    /// Propagates a JWT encoding failure.
    pub fn sign(&self, account: &Account, token: &AccountApiToken) -> Result<String> {
        AuthClaims::new(account, token).into_string(self.session_keys.encoding_key())
    }

    /// Mints a `web` browser session for `account`: creates the session row and
    /// signs its JWT.
    ///
    /// The idle bound follows `remember_me`; the session then slides forward on
    /// use up to the absolute cap. Password login, signup, and OIDC web sign-in
    /// all go through it, so the browser session shape is identical across the
    /// three paths. The caller delivers the returned JWT to the browser as a
    /// session cookie (a `WebSession`).
    ///
    /// # Errors
    ///
    /// Propagates database and JWT-signing failures.
    pub async fn issue_web_session(
        &self,
        conn: &mut PgConn,
        account: &Account,
        remember_me: bool,
        security: SecurityContext,
    ) -> Result<String> {
        self.issue_session(
            conn,
            account,
            ApiTokenType::Web,
            remember_me,
            session::initial_expires_at(remember_me).into(),
            security,
        )
        .await
    }

    /// Mints a native-app (desktop) session token for `account`: a long-lived
    /// `app` token (see [`session::APP_TOKEN_LIFETIME`]) that does not slide and
    /// is exempt from the browser absolute cap. The desktop app stores it and
    /// sends it as a Bearer credential; it is never a cookie.
    ///
    /// Caps live `app` tokens per account (best-effort): repeated desktop logins
    /// do not accumulate unbounded long-lived credentials — the oldest beyond the
    /// limit are evicted, and a pruning failure is logged, not propagated.
    ///
    /// # Errors
    ///
    /// Propagates database and JWT-signing failures from the mint itself.
    pub async fn issue_app_token(
        &self,
        conn: &mut PgConn,
        account: &Account,
        security: SecurityContext,
    ) -> Result<String> {
        let expired_at =
            Timestamp::now() + Span::new().seconds(session::APP_TOKEN_LIFETIME.as_secs() as i64);
        let jwt = self
            .issue_session(
                conn,
                account,
                ApiTokenType::App,
                false,
                expired_at.into(),
                security,
            )
            .await?;

        if let Err(error) = conn
            .prune_app_tokens(account.id, session::MAX_APP_TOKENS_PER_ACCOUNT)
            .await
        {
            tracing::warn!(
                target: TRACING_TARGET,
                error = %error,
                account_id = %account.id,
                "failed to prune old app tokens after minting",
            );
        }

        Ok(jwt)
    }

    /// Shared session mint: persists an `account_api_tokens` row of `session_type`
    /// and signs its JWT. The caller is responsible for gating the account's
    /// status (suspended/deleted) before minting.
    async fn issue_session(
        &self,
        conn: &mut PgConn,
        account: &Account,
        session_type: ApiTokenType,
        is_remembered: bool,
        expired_at: nvisy_postgres::JiffTimestamp,
        security: SecurityContext,
    ) -> Result<String> {
        // The session's display name is derived from the user agent; the client IP
        // and raw user agent are recorded on the row for the account's session list
        // and audit trail.
        let display_name = self
            .user_agent_parser
            .parse(security.user_agent.as_deref().unwrap_or_default());
        let new_token = NewAccountApiToken {
            account_id: account.id,
            display_name,
            ip_address: security.ip_address,
            user_agent: security.user_agent,
            is_remembered: Some(is_remembered),
            session_type: Some(session_type),
            expired_at: Some(expired_at),
        };
        let token = conn.create_account_api_token(new_token).await?;
        tracing::info!(
            target: TRACING_TARGET,
            token_id = %token.id,
            account_id = %account.id,
            session_type = ?session_type,
            "Minted session token",
        );
        self.sign(account, &token)
    }
}
