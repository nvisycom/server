//! Test-only support for query-layer integration tests.
//!
//! Starts an ephemeral PostgreSQL container, applies the crate's embedded
//! migrations, and hands out a connected [`PgClient`] — so the query repositories
//! can be exercised against a real database without any external setup. Gated on
//! the `test_util` feature, so it never ships in a default build.

use std::sync::OnceLock;

use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::client::PgClientMigrationExt;
use crate::model::{NewAccount, NewWorkspace, NewWorkspaceFile, NewWorkspacePipeline};
use crate::query::{
    AccountRepository, WorkspaceFileRepository, WorkspacePipelineRepository, WorkspaceRepository,
};
use crate::{PgClient, PgConfig};

/// Caps how many Postgres containers may boot at once, across the whole test
/// binary.
///
/// Each test spins up its own container for isolation, but the readiness probe
/// inside `start()` is startup-sensitive: booting all of them at once stresses
/// the Docker daemon enough that some probes time out and the test flakes. This
/// semaphore serializes the boot (only the boot — the container runs and the test
/// executes fully in parallel once it is up), so there is never a boot storm.
///
/// The permit count defaults to 1 (fully serialized boots, deterministic on any
/// runner) and can be raised with the `NVISY_TEST_BOOT_CONCURRENCY` environment
/// variable to trade determinism for faster startup on a capable machine.
fn boot_semaphore() -> &'static Semaphore {
    static BOOT: OnceLock<Semaphore> = OnceLock::new();
    BOOT.get_or_init(|| {
        let permits = std::env::var("NVISY_TEST_BOOT_CONCURRENCY")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|&n| n >= 1)
            .unwrap_or(1);
        Semaphore::new(permits)
    })
}

/// A running Postgres container paired with a migrated [`PgClient`].
///
/// Keep the whole `TestDatabase` alive for the duration of a test: dropping it
/// stops and removes the container. The client's pool points at the container's
/// mapped port.
pub struct TestDatabase {
    /// Held only to keep the container alive; the client talks to it by URL.
    _container: ContainerAsync<Postgres>,
    /// A migrated client connected to the container.
    pub client: PgClient,
}

impl TestDatabase {
    /// Starts a fresh Postgres container, connects a [`PgClient`], and applies the
    /// crate's embedded migrations.
    ///
    /// Pins the image to the Postgres major version the schema targets, so the
    /// container matches production rather than the module's older default.
    ///
    /// # Panics
    ///
    /// Panics if the container cannot start, the client cannot connect, or the
    /// migrations fail — a test cannot proceed without a working database.
    pub async fn start() -> Self {
        // Serialize the boot (and the readiness probe inside `start()`) so many
        // tests do not overwhelm the Docker daemon at once; the permit is released
        // as soon as the container is up, so tests still run in parallel from there.
        let (container, port) = {
            let _permit = boot_semaphore()
                .acquire()
                .await
                .expect("boot semaphore is never closed");
            let container = Postgres::default()
                .with_tag("18-alpine")
                .start()
                .await
                .expect("failed to start postgres container");
            let port = container
                .get_host_port_ipv4(5432)
                .await
                .expect("failed to resolve container port");
            (container, port)
        };
        let url = format!("postgresql://postgres:postgres@127.0.0.1:{port}/postgres");

        let client = PgClient::new_with_test(PgConfig::new(url))
            .await
            .expect("failed to connect to the test database");
        client
            .run_pending_migrations()
            .await
            .expect("failed to apply migrations to the test database");

        Self {
            _container: container,
            client,
        }
    }

    /// Seeds an account, returning its id — the FK parent a workspace requires.
    ///
    /// Each call uses a fresh unique handle, so repeated seeding in one test does
    /// not collide on the unique constraints.
    ///
    /// # Panics
    ///
    /// Panics if the insert fails.
    pub async fn seed_account(&self) -> Uuid {
        let mut conn = self
            .client
            .get_connection()
            .await
            .expect("failed to get a connection");
        conn.create_account(NewAccount::test())
            .await
            .expect("failed to seed account")
            .id
    }

    /// Seeds an account and a workspace it owns, returning `(account_id,
    /// workspace_id)` — the FK parents most workspace-scoped rows require.
    ///
    /// # Panics
    ///
    /// Panics if either insert fails.
    pub async fn seed_account_and_workspace(&self) -> (Uuid, Uuid) {
        let account_id = self.seed_account().await;
        let mut conn = self
            .client
            .get_connection()
            .await
            .expect("failed to get a connection");
        let workspace = conn
            .create_workspace(NewWorkspace::test(account_id))
            .await
            .expect("failed to seed workspace");

        (account_id, workspace.id)
    }

    /// Seeds an account, a workspace, a pipeline, and an input file, returning
    /// `(account_id, workspace_id, pipeline_id, file_id)` — the FK parents a
    /// detection (and, through it, a redaction) requires.
    ///
    /// # Panics
    ///
    /// Panics if any insert fails.
    pub async fn seed_pipeline_and_file(&self) -> (Uuid, Uuid, Uuid, Uuid) {
        let (account_id, workspace_id) = self.seed_account_and_workspace().await;
        let mut conn = self
            .client
            .get_connection()
            .await
            .expect("failed to get a connection");
        let pipeline = conn
            .create_workspace_pipeline(NewWorkspacePipeline::test(workspace_id, account_id))
            .await
            .expect("failed to seed pipeline");
        let file = conn
            .create_workspace_file(NewWorkspaceFile::test(workspace_id, account_id))
            .await
            .expect("failed to seed file");

        (account_id, workspace_id, pipeline.id, file.id)
    }
}
