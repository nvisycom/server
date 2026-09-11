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

    /// Seeds an account and a workspace it owns — the FK parents most
    /// workspace-scoped rows require.
    ///
    /// # Panics
    ///
    /// Panics if either insert fails.
    pub async fn seed_account_and_workspace(&self) -> SeededWorkspace {
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

        SeededWorkspace {
            account_id,
            workspace_id: workspace.id,
        }
    }

    /// Seeds an account, a workspace, a pipeline, and an input file — the FK
    /// parents a detection (and, through it, a redaction) requires.
    ///
    /// # Panics
    ///
    /// Panics if any insert fails.
    pub async fn seed_pipeline_and_file(&self) -> SeededPipeline {
        let SeededWorkspace {
            account_id,
            workspace_id,
        } = self.seed_account_and_workspace().await;
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

        SeededPipeline {
            account_id,
            workspace_id,
            pipeline_id: pipeline.id,
            file_id: file.id,
        }
    }
}

/// An account and a workspace it owns, seeded for a test.
#[derive(Debug, Clone, Copy)]
pub struct SeededWorkspace {
    /// The account that owns the workspace.
    pub account_id: Uuid,
    /// The workspace.
    pub workspace_id: Uuid,
}

/// An account, workspace, pipeline, and input file, seeded for a test — the FK
/// parents a detection and redaction need.
#[derive(Debug, Clone, Copy)]
pub struct SeededPipeline {
    /// The account that owns everything.
    pub account_id: Uuid,
    /// The workspace.
    pub workspace_id: Uuid,
    /// The pipeline.
    pub pipeline_id: Uuid,
    /// The input file.
    pub file_id: Uuid,
}

/// Test-only helpers that backdate a single row's timestamp column(s).
///
/// Production `create_*` methods stamp timestamps like `created_at`/`started_at`
/// from the database clock and never expose them, so a test cannot otherwise
/// build a row that is already old, expired, or strictly ordered against another.
/// This module is the one place tests reach past the repository to set such a
/// column, kept here (feature-gated) rather than as raw SQL scattered through the
/// test modules, and off the production `New*` structs so it can never affect a
/// non-test build.
pub mod backdate {
    use diesel::prelude::*;
    use diesel_async::RunQueryDsl;
    use jiff::Timestamp;
    use uuid::Uuid;

    use crate::{PgConn, Result};

    /// Generates a backdate setter that updates a table's timestamp column(s) on
    /// the row with the given `id`. Each table's columns are distinct Diesel
    /// types, so the body is generated per (table, column) rather than made
    /// generic. The invocation names the table and each column, and the generated
    /// function's parameters are named after those columns, so both the definition
    /// and the call site show which value goes to which column.
    ///
    /// - `fn = table.column` sets one non-null column.
    /// - `fn = table.(a, b)` sets two together (`b` nullable), for spans where a
    ///   two-step update would trip a `b > a` / `b >= a` check.
    macro_rules! backdate {
        ($fn:ident = $table:ident . $col:ident) => {
            #[doc = concat!("Sets `", stringify!($table), ".", stringify!($col), "`.")]
            pub async fn $fn(conn: &mut PgConn, id: Uuid, $col: Timestamp) -> Result<()> {
                use crate::schema::$table::dsl;
                diesel::update(dsl::$table.filter(dsl::id.eq(id)))
                    .set(dsl::$col.eq(jiff_diesel::Timestamp::from($col)))
                    .execute(conn)
                    .await
                    .map_err(crate::Error::from)?;
                Ok(())
            }
        };
        ($fn:ident = $table:ident . ($col_a:ident, $col_b:ident)) => {
            #[doc = concat!(
                            "Sets `", stringify!($table), ".", stringify!($col_a), "` and `",
                            stringify!($col_b), "` (nullable), together."
                        )]
            pub async fn $fn(
                conn: &mut PgConn,
                id: Uuid,
                $col_a: Timestamp,
                $col_b: Timestamp,
            ) -> Result<()> {
                use crate::schema::$table::dsl;
                diesel::update(dsl::$table.filter(dsl::id.eq(id)))
                    .set((
                        dsl::$col_a.eq(jiff_diesel::Timestamp::from($col_a)),
                        dsl::$col_b.eq(Some(jiff_diesel::Timestamp::from($col_b))),
                    ))
                    .execute(conn)
                    .await
                    .map_err(crate::Error::from)?;
                Ok(())
            }
        };
    }

    backdate!(activity_created_at = workspace_activities.created_at);
    backdate!(notification_created_at = account_notifications.created_at);
    backdate!(policy_created_at = workspace_policies.created_at);
    backdate!(redaction_created_at = workspace_redactions.created_at);
    backdate!(sync_started_at = workspace_connection_syncs.started_at);

    // Two-column spans: set both at once so `expires_at > created_at` /
    // `completed_at >= started_at` holds (a two-step update would momentarily
    // violate it).
    backdate!(notification_span = account_notifications.(created_at, expires_at));
    backdate!(file_span = workspace_files.(created_at, expires_at));
    backdate!(detection_span = workspace_detections.(started_at, completed_at));
}
