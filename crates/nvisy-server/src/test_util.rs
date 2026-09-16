//! Test-only harness for authenticated HTTP-handler integration tests.
//!
//! [`TestApp`] boots the platform's infrastructure in containers ([Postgres],
//! [NATS], and an S3-compatible [store]), generates ephemeral signing and
//! encryption keys, and assembles a real [`ServiceState`] behind an
//! [`axum_test::TestServer`]. It then hands out pre-authenticated [`Actor`]s: a
//! seeded account plus a signed bearer token accepted by the auth middleware.
//!
//! Bearer transport is exempt from CSRF, so a state-changing request needs no
//! CSRF header. A token is accepted only when its `jti` is a live
//! `account_api_tokens` row, so [`TestApp::actor`] seeds that row before signing.
//!
//! Gated on the `test_util` feature; it never ships in a default build.
//!
//! [Postgres]: nvisy_postgres::test_util::TestDatabase
//! [NATS]: TestNats
//! [store]: TestBlobStore

use axum_test::TestServer;
use nvisy_nats::TestNats;
use nvisy_postgres::PgConn;
use nvisy_postgres::model::{Account, NewAccount, NewAccountApiToken, NewWorkspace};
use nvisy_postgres::query::{AccountApiTokenRepository, AccountRepository, WorkspaceRepository};
use nvisy_postgres::test_util::TestDatabase;
use nvisy_postgres::types::{ApiTokenType, WorkspaceRole};
use nvisy_s3::TestBlobStore;
use nvisy_webhook::reqwest::ReqwestClient;
use uuid::Uuid;

use crate::ServiceArgs;
use crate::extract::AuthClaims;
use crate::handler::{CustomRoutes, routes};
use crate::middleware::UploadConfig;
use crate::response::CookieConfig;
use crate::service::{
    AuthKeys, AuthKeysConfig, CryptoConfig, CryptoService, EngineConfig, FileConnectorsConfig,
    HealthConfig, IntegrationConfig, OidcConfig, ServiceState,
};
use crate::worker::purge::PurgeConfig;

/// A matching Ed25519 keypair (PKCS#8 PEM), used to sign and verify session
/// tokens in tests. Ephemeral in spirit — a fixed test-only pair is fine because
/// nothing outside a test run ever trusts it.
const TEST_PRIVATE_KEY: &str = r"-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VwBCIEIDQtFc/jcCECuwR6cQqh9Xy3y8pcryWDn/HVN5fPSwm+
-----END PRIVATE KEY-----";
const TEST_PUBLIC_KEY: &str = r"-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEAMveirBCUUpVI8TCv4W5jAZqtkEzfA7eIvozsugFbvDU=
-----END PUBLIC KEY-----";

/// A seeded account and a signed bearer token for it, accepted by the auth
/// middleware. Pass [`Actor::jwt`] to `.authorization_bearer(..)` on a request.
pub struct Actor {
    /// The seeded account's id.
    pub account_id: Uuid,
    /// A signed JWT whose `jti` is a live `account_api_tokens` row.
    pub jwt: String,
}

/// The three infrastructure containers backing a [`TestApp`]. Held together so
/// they live for the whole test; [`TestInfra::db`] also seeds through its client.
struct TestInfra {
    /// Also used for the membership seeder; the container stays alive with it.
    db: TestDatabase,
    _nats: TestNats,
    _s3: TestBlobStore,
}

/// A running server over containerized infrastructure, plus the pieces tests
/// seed against. Keep it alive for the whole test: dropping it stops the
/// containers.
///
/// `state` is boxed so the whole `TestApp` stays small enough to hold across a
/// request `.await` in a test without producing an oversized future.
pub struct TestApp {
    server: TestServer,
    state: Box<ServiceState>,
    keys: AuthKeys,
    infra: TestInfra,
}

impl TestApp {
    /// Boots the three infrastructure containers, assembles a real
    /// [`ServiceState`] over them with in-memory ephemeral keys, and returns a
    /// running [`TestServer`] for the full built-in router.
    ///
    /// # Panics
    ///
    /// Panics if any container fails to start or the service state cannot be
    /// assembled — a test cannot proceed otherwise.
    pub async fn spawn() -> Self {
        // The build future is large (it constructs the whole `ServiceArgs` and
        // `ServiceState` across several awaits); box it so a test holding a
        // `TestApp::spawn().await` does not inherit that size.
        Box::pin(Self::spawn_inner()).await
    }

    async fn spawn_inner() -> Self {
        let db = TestDatabase::start().await;
        let nats = TestNats::start().await;
        let s3 = TestBlobStore::start().await;

        // Ephemeral keys, built entirely in memory (no files): a fixed Ed25519
        // pair for session tokens and a fixed 32-byte master key. The same
        // `AuthKeys` both signs the mint's tokens and verifies them in the
        // server, so a seeded token is accepted.
        let keys = AuthKeys::from_pem(TEST_PUBLIC_KEY.as_bytes(), TEST_PRIVATE_KEY.as_bytes())
            .expect("build ephemeral session keys");
        let crypto = CryptoService::from_key_bytes(&[0u8; 32]).expect("build ephemeral crypto");

        // The key configs go unused by `assemble` (the built services above are
        // injected), so their defaults are placeholders.
        let args = ServiceArgs {
            postgres: db.client.config().clone(),
            nats: nats.client.config().clone(),
            s3: s3.config.clone(),
            session_keys: AuthKeysConfig::default(),
            crypto: CryptoConfig::default(),
            engine: EngineConfig::default(),
            health: HealthConfig::default(),
            integration: IntegrationConfig::default(),
            purge: PurgeConfig::default(),
            file_service: FileConnectorsConfig::default(),
            oidc: OidcConfig::default(),
            upload: UploadConfig::default(),
            cookie: CookieConfig::default(),
        };

        let webhook = ReqwestClient::default().into_service();
        let state = ServiceState::assemble(args, webhook, crypto, keys.clone())
            .await
            .expect("assemble the service state");

        let router = routes(CustomRoutes::new(), state.clone());
        let app = router.with_state(state.clone());
        let server = TestServer::new(axum::Router::from(app));

        Self {
            server,
            state: Box::new(state),
            keys,
            infra: TestInfra {
                db,
                _nats: nats,
                _s3: s3,
            },
        }
    }

    /// The running test server; issue requests through it.
    #[must_use]
    pub fn server(&self) -> &TestServer {
        &self.server
    }

    /// Seeds an account and a live `app` api-token row, and signs a bearer JWT
    /// for it. The returned [`Actor::jwt`] is accepted by the auth middleware.
    ///
    /// # Panics
    ///
    /// Panics if seeding or signing fails.
    pub async fn actor(&self) -> Actor {
        let mut conn = self.connection().await;
        let account = conn
            .create_account(NewAccount::test())
            .await
            .expect("seed account");
        let jwt = self.sign_for(&mut conn, &account).await;
        Actor {
            account_id: account.id,
            jwt,
        }
    }

    /// Seeds an [`Actor`] and a workspace they **own** (with the owner
    /// membership), returning both. The membership is what authorizes the actor
    /// on workspace-scoped routes.
    ///
    /// # Panics
    ///
    /// Panics if any seed fails.
    pub async fn workspace_owner(&self) -> (Actor, Uuid) {
        self.workspace_member(WorkspaceRole::Owner).await
    }

    /// Seeds an [`Actor`], a workspace, and the actor's membership at `role`.
    ///
    /// # Panics
    ///
    /// Panics if any seed fails.
    pub async fn workspace_member(&self, role: WorkspaceRole) -> (Actor, Uuid) {
        let actor = self.actor().await;
        let mut conn = self.connection().await;
        let workspace = conn
            .create_workspace(NewWorkspace::test(actor.account_id))
            .await
            .expect("seed workspace");
        // Reuse the postgres seeder so the membership matches production shape.
        self.infra
            .db
            .seed_workspace_member(workspace.id, actor.account_id, role)
            .await;
        (actor, workspace.id)
    }

    /// A pooled connection to the shared test database.
    async fn connection(&self) -> PgConn {
        self.state
            .infra
            .postgres
            .get_connection()
            .await
            .expect("get a database connection")
    }

    /// Seeds an `app` token row for `account` and signs a JWT whose `jti` is that
    /// row's id, so [`AuthState`](crate::extract::AuthState) accepts it.
    async fn sign_for(&self, conn: &mut PgConn, account: &Account) -> String {
        let token = conn
            .create_account_api_token(NewAccountApiToken::test(account.id, ApiTokenType::App))
            .await
            .expect("seed api token");
        AuthClaims::new(account, &token)
            .into_string(self.keys.encoding_key())
            .expect("sign the bearer token")
    }
}
