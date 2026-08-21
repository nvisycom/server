//! Workspace analytics: aggregate queries over a workspace's files and pipeline
//! runs, for the analytics endpoint.
//!
//! Two entry points, each reading a consistent snapshot in one read-only
//! transaction: `snapshot` (storage, run health, token usage) and `runs_by_day`
//! (the daily time series). Both compose several grouped aggregates from private
//! `load_*` helpers. Rows come back sparse (one per group that has data, never
//! zero-filled here) — the handler maps them onto the full set of enum values,
//! filling absent groups with zero, so the response is stable regardless of what
//! data exists.

use std::future::Future;

use bigdecimal::BigDecimal;
use diesel::prelude::*;
use diesel::sql_types::Timestamptz;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::types::{FileKind, PipelineRunStatus};
use crate::{PgConnection, PgError, PgResult, schema};

/// Per-day run counts and durations, as loaded from the grouped run query.
#[derive(Debug, Clone, Queryable)]
struct RunDayCounts {
    day: jiff_diesel::Timestamp,
    runs: i64,
    // Conditional counts are `sum(CASE WHEN cond THEN 1 ELSE 0 END)`; Postgres
    // `sum` of bigint returns numeric, hence BigDecimal (nullable over no rows).
    // Converted to i64 when assembling the point.
    terminal: Option<BigDecimal>,
    failed: Option<BigDecimal>,
    // Duration in milliseconds, scaled and rounded to a bigint in SQL.
    avg_ms: Option<i64>,
    p95_ms: Option<i64>,
}

/// Per-day token totals, as loaded from the grouped usage query.
#[derive(Debug, Clone, Queryable)]
struct RunDayTokens {
    day: jiff_diesel::Timestamp,
    input: Option<BigDecimal>,
    output: Option<BigDecimal>,
    total: Option<BigDecimal>,
}

/// A day's token totals converted to `i64`, keyed by day while merging the run
/// and usage series.
#[derive(Debug, Clone, Copy, Default)]
struct DayTokens {
    input: Option<i64>,
    output: Option<i64>,
    total: Option<i64>,
}

/// One day of pipeline-run activity for a workspace. Only days with at least one
/// run are produced; the caller fills the gaps to a dense series over its window.
/// `day` is the UTC day (midnight) the run counts fall in.
#[derive(Debug, Clone)]
pub struct RunDayPoint {
    /// The day (UTC midnight) this point covers.
    pub day: jiff::Timestamp,
    /// Runs started on this day.
    pub runs: i64,
    /// Runs that reached a terminal state (`completed` or `failed`) this day.
    pub terminal: i64,
    /// Runs that failed this day.
    pub failed: i64,
    /// Mean duration (milliseconds) of runs that completed this day; null if none did.
    pub avg_ms: Option<i64>,
    /// 95th-percentile duration (milliseconds) of runs completed this day; null if none.
    pub p95_ms: Option<i64>,
    /// Input/prompt tokens across models used by this day's runs; null if none.
    pub input_tokens: Option<i64>,
    /// Output/completion tokens across models used by this day's runs; null if none.
    pub output_tokens: Option<i64>,
    /// Reported total tokens across models used by this day's runs; null if none.
    pub total_tokens: Option<i64>,
}

/// Per-model token totals as loaded; each `SUM(bigint)` is `numeric`, so it
/// arrives as `BigDecimal` and is narrowed to `i64` when building [`UsageByModel`].
#[derive(Debug, Clone, Queryable)]
struct UsageByModelRow {
    model: String,
    input_tokens: Option<BigDecimal>,
    output_tokens: Option<BigDecimal>,
    total_tokens: Option<BigDecimal>,
}

/// Inference token totals for one model across a workspace's runs. Each field is
/// independently nullable because a provider may report only some of them (a
/// `total` that is not `input + output`, or no breakdown at all).
#[derive(Debug, Clone)]
pub struct UsageByModel {
    /// The model these totals are for.
    pub model: String,
    /// Summed input/prompt tokens, or `None` if no run reported them for this model.
    pub input_tokens: Option<i64>,
    /// Summed output/completion tokens, or `None` if none reported.
    pub output_tokens: Option<i64>,
    /// Summed reported totals, or `None` if none reported.
    pub total_tokens: Option<i64>,
}

/// Live-file count and byte total for one `file_kind`, as loaded. `SUM(bigint)`
/// is `numeric`, so the byte total arrives as `BigDecimal` and is narrowed to
/// `i64` when building [`StorageByKind`].
#[derive(Debug, Clone, Queryable)]
struct StorageKindRow {
    file_kind: FileKind,
    file_count: i64,
    total_bytes: Option<BigDecimal>,
}

/// Live-file count and byte total for one `file_kind` in a workspace.
#[derive(Debug, Clone)]
pub struct StorageByKind {
    /// The file kind this row aggregates.
    pub file_kind: FileKind,
    /// Number of live files of this kind.
    pub file_count: i64,
    /// Total bytes of live files of this kind.
    pub total_bytes: i64,
}

/// Run count for one `status` in a workspace.
#[derive(Debug, Clone, Queryable)]
pub struct RunStatusCount {
    /// The run status this row aggregates.
    pub status: PipelineRunStatus,
    /// Number of runs in this status.
    pub count: i64,
}

/// Completed-run duration summary for a workspace, in milliseconds. Both are
/// `None` until at least one run has completed.
#[derive(Debug, Clone)]
pub struct RunDurations {
    /// Mean wall-clock duration of completed runs.
    pub avg_ms: Option<i64>,
    /// 95th-percentile duration of completed runs.
    pub p95_ms: Option<i64>,
}

/// A workspace's point-in-time analytics: storage, run health, and token usage
/// read together so the parts agree. Produced by
/// [`snapshot`](WorkspaceAnalyticsRepository::snapshot).
#[derive(Debug, Clone)]
pub struct AnalyticsSnapshot {
    /// Live-file counts and byte totals, one per `file_kind` present.
    pub storage: Vec<StorageByKind>,
    /// Run counts, one per `status` present.
    pub runs: Vec<RunStatusCount>,
    /// Completed-run duration summary.
    pub durations: RunDurations,
    /// Inference token totals, one per model used.
    pub usage: Vec<UsageByModel>,
}

/// Read-only aggregate queries backing the workspace analytics endpoint.
pub trait WorkspaceAnalyticsRepository {
    /// A workspace's point-in-time analytics — storage by kind, run counts by
    /// status, completed-run durations, and per-model token usage — read in one
    /// read-only, repeatable-read transaction so the parts reflect a single
    /// snapshot. Each breakdown lists only the groups that have data; the caller
    /// zero-fills the rest.
    fn snapshot(
        &mut self,
        workspace_id: Uuid,
    ) -> impl Future<Output = PgResult<AnalyticsSnapshot>> + Send;

    /// Daily pipeline-run activity for a workspace over `[from, to)` (UTC day
    /// boundaries; `to` exclusive). Returns one row per day that has at least one
    /// run — the series is sparse — so the caller gap-fills the window to a dense
    /// series (see `RunTimeSeries::from_window`). The window is bucketed by
    /// `date_trunc('day', started_at)`; runs are scoped through their live
    /// pipeline. The caller is responsible for bounding the window.
    ///
    /// Run counts, durations, and token totals are read in one transaction so the
    /// two grouped statements observe the same snapshot.
    fn runs_by_day(
        &mut self,
        workspace_id: Uuid,
        from: jiff::Timestamp,
        to: jiff::Timestamp,
    ) -> impl Future<Output = PgResult<Vec<RunDayPoint>>> + Send;
}

impl WorkspaceAnalyticsRepository for PgConnection {
    async fn snapshot(&mut self, workspace_id: Uuid) -> PgResult<AnalyticsSnapshot> {
        // The four reads run in one read-only, repeatable-read transaction so the
        // snapshot is internally consistent: a run cannot appear in the status
        // counts while its tokens are missing from the usage totals because a
        // write landed between two of the reads.
        self.build_transaction()
            .read_only()
            .repeatable_read()
            .run(async |conn| {
                Ok(AnalyticsSnapshot {
                    storage: load_storage_by_kind(conn, workspace_id).await?,
                    runs: load_runs_by_status(conn, workspace_id).await?,
                    durations: load_run_durations(conn, workspace_id).await?,
                    usage: load_usage_by_model(conn, workspace_id).await?,
                })
            })
            .await
    }

    async fn runs_by_day(
        &mut self,
        workspace_id: Uuid,
        from: jiff::Timestamp,
        to: jiff::Timestamp,
    ) -> PgResult<Vec<RunDayPoint>> {
        use std::collections::BTreeMap;

        let from = jiff_diesel::Timestamp::from(from);
        let to = jiff_diesel::Timestamp::from(to);

        // Read the run counts/durations and the token totals in one read-only,
        // repeatable-read transaction so both grouped statements observe the same
        // snapshot; otherwise a run written between them could show in one series
        // but not the other.
        let (run_rows, token_rows): (Vec<RunDayCounts>, Vec<RunDayTokens>) = self
            .build_transaction()
            .read_only()
            .repeatable_read()
            .run(async |conn| {
                let run_rows = load_run_day_counts(conn, workspace_id, from, to).await?;
                let token_rows = load_run_day_tokens(conn, workspace_id, from, to).await?;
                Ok::<_, PgError>((run_rows, token_rows))
            })
            .await?;

        // Merge the two per-day results by day.
        use bigdecimal::ToPrimitive;
        let to_i64 = |v: Option<BigDecimal>| v.and_then(|b| b.to_i64());
        let mut tokens: BTreeMap<jiff::Timestamp, DayTokens> = BTreeMap::new();
        for row in token_rows {
            tokens.insert(
                row.day.into(),
                DayTokens {
                    input: to_i64(row.input),
                    output: to_i64(row.output),
                    total: to_i64(row.total),
                },
            );
        }

        let points = run_rows
            .into_iter()
            .map(|row| {
                let day: jiff::Timestamp = row.day.into();
                let t = tokens.remove(&day).unwrap_or_default();
                RunDayPoint {
                    day,
                    runs: row.runs,
                    terminal: to_i64(row.terminal).unwrap_or(0),
                    failed: to_i64(row.failed).unwrap_or(0),
                    avg_ms: row.avg_ms,
                    p95_ms: row.p95_ms,
                    input_tokens: t.input,
                    output_tokens: t.output,
                    total_tokens: t.total,
                }
            })
            .collect();

        Ok(points)
    }
}

/// Live-file count and byte total per `file_kind`. Only kinds with a live file
/// appear; the caller zero-fills the rest.
async fn load_storage_by_kind(
    conn: &mut PgConnection,
    workspace_id: Uuid,
) -> PgResult<Vec<StorageByKind>> {
    use bigdecimal::ToPrimitive;
    use diesel::dsl::{count_star, sum};
    use schema::workspace_files::{self, dsl};

    let rows: Vec<StorageKindRow> = workspace_files::table
        .filter(dsl::workspace_id.eq(workspace_id))
        .filter(dsl::deleted_at.is_null())
        .group_by(dsl::file_kind)
        .select((dsl::file_kind, count_star(), sum(dsl::file_size_bytes)))
        .load(conn)
        .await
        .map_err(PgError::from)?;

    // Byte totals are converted to i64 at the boundary so callers do not depend
    // on BigDecimal. NULL (an empty group) and the physically impossible i64
    // overflow both fall back to 0, so a failed conversion never fabricates a
    // huge total that would poison the workspace sum.
    Ok(rows
        .into_iter()
        .map(|row| StorageByKind {
            file_kind: row.file_kind,
            file_count: row.file_count,
            total_bytes: row.total_bytes.and_then(|b| b.to_i64()).unwrap_or(0),
        })
        .collect())
}

/// Run count per `status`, scoped through the live pipeline. Only statuses with
/// a run appear.
async fn load_runs_by_status(
    conn: &mut PgConnection,
    workspace_id: Uuid,
) -> PgResult<Vec<RunStatusCount>> {
    use diesel::dsl::count_star;
    use schema::workspace_pipeline_runs::dsl as runs;
    use schema::workspace_pipelines::dsl as pipelines;
    use schema::{workspace_pipeline_runs, workspace_pipelines};

    workspace_pipeline_runs::table
        .inner_join(workspace_pipelines::table)
        .filter(pipelines::workspace_id.eq(workspace_id))
        .filter(pipelines::deleted_at.is_null())
        .group_by(runs::status)
        .select((runs::status, count_star()))
        .load(conn)
        .await
        .map_err(PgError::from)
}

/// Mean and 95th-percentile duration (milliseconds) of the workspace's completed
/// runs. Both `None` when no run has completed.
async fn load_run_durations(conn: &mut PgConnection, workspace_id: Uuid) -> PgResult<RunDurations> {
    use diesel::dsl::sql;
    use diesel::sql_types::{BigInt, Nullable};
    use schema::workspace_pipeline_runs::dsl as runs;
    use schema::workspace_pipelines::dsl as pipelines;
    use schema::{workspace_pipeline_runs, workspace_pipelines};

    // Duration in milliseconds: the interval's epoch-seconds are scaled by 1000
    // and rounded to a bigint in SQL, so the value crosses the boundary already in
    // the API unit and type. `avg` and `percentile_cont` (an ordered-set aggregate
    // with no Diesel builtin) both return NULL over no rows. Columns are
    // table-qualified so the join can never make them ambiguous.
    let avg_ms = sql::<Nullable<BigInt>>(
        "round(avg(EXTRACT(EPOCH FROM (workspace_pipeline_runs.completed_at \
         - workspace_pipeline_runs.started_at)) * 1000))::bigint",
    );
    let p95_ms = sql::<Nullable<BigInt>>(
        "round(percentile_cont(0.95) WITHIN GROUP \
         (ORDER BY EXTRACT(EPOCH FROM (workspace_pipeline_runs.completed_at \
         - workspace_pipeline_runs.started_at))) * 1000)::bigint",
    );

    let (avg_ms, p95_ms): (Option<i64>, Option<i64>) = workspace_pipeline_runs::table
        .inner_join(workspace_pipelines::table)
        .filter(pipelines::workspace_id.eq(workspace_id))
        .filter(pipelines::deleted_at.is_null())
        .filter(runs::completed_at.is_not_null())
        .select((avg_ms, p95_ms))
        .first(conn)
        .await
        .map_err(PgError::from)?;

    Ok(RunDurations { avg_ms, p95_ms })
}

/// Inference token totals per model across the workspace's runs, scoped through
/// the live pipeline. Only models actually used appear. Kept per-model rather
/// than summed to one total, since a run may mix models.
async fn load_usage_by_model(
    conn: &mut PgConnection,
    workspace_id: Uuid,
) -> PgResult<Vec<UsageByModel>> {
    use bigdecimal::ToPrimitive;
    use diesel::dsl::sum;
    use schema::workspace_pipeline_run_usage::dsl as usage;
    use schema::workspace_pipeline_runs::dsl as runs;
    use schema::workspace_pipelines::dsl as pipelines;
    use schema::{workspace_pipeline_run_usage, workspace_pipeline_runs, workspace_pipelines};

    let rows: Vec<UsageByModelRow> = workspace_pipeline_run_usage::table
        .inner_join(workspace_pipeline_runs::table.on(runs::id.eq(usage::run_id)))
        .inner_join(workspace_pipelines::table.on(pipelines::id.eq(runs::pipeline_id)))
        .filter(pipelines::workspace_id.eq(workspace_id))
        .filter(pipelines::deleted_at.is_null())
        .group_by(usage::model)
        .select((
            usage::model,
            sum(usage::input_tokens),
            sum(usage::output_tokens),
            sum(usage::total_tokens),
        ))
        .load(conn)
        .await
        .map_err(PgError::from)?;

    let to_i64 = |v: Option<BigDecimal>| v.and_then(|b| b.to_i64());
    Ok(rows
        .into_iter()
        .map(|row| UsageByModel {
            model: row.model,
            input_tokens: to_i64(row.input_tokens),
            output_tokens: to_i64(row.output_tokens),
            total_tokens: to_i64(row.total_tokens),
        })
        .collect())
}

/// The day bucket a run's start falls in. Shared by both daily queries so the
/// truncation text cannot drift — they must group on the same expression for the
/// per-day merge to line up. The column is table-qualified so the join to
/// pipelines can never make it ambiguous. Returns a fresh fragment per call, as
/// the builder consumes it in both `group_by` and `select`.
fn run_day() -> diesel::expression::SqlLiteral<Timestamptz> {
    diesel::dsl::sql::<Timestamptz>("date_trunc('day', workspace_pipeline_runs.started_at)")
}

/// Per-day run counts and durations over `[from, to)`, scoped through the live
/// pipeline. Sparse: only days that have a run. Shared with the token query so
/// both observe the same snapshot inside one transaction.
async fn load_run_day_counts(
    conn: &mut PgConnection,
    workspace_id: Uuid,
    from: jiff_diesel::Timestamp,
    to: jiff_diesel::Timestamp,
) -> PgResult<Vec<RunDayCounts>> {
    use diesel::dsl::{case_when, count_star, sql, sum};
    use diesel::sql_types::{BigInt, Nullable as SqlNullable};
    use schema::workspace_pipeline_runs::dsl as runs;
    use schema::workspace_pipelines::dsl as pipelines;
    use schema::{workspace_pipeline_runs, workspace_pipelines};

    // Durations in milliseconds (epoch-seconds scaled by 1000, rounded to bigint),
    // so the value crosses the boundary already in the API unit and type. The
    // percentile is an ordered-set aggregate with no Diesel builtin, so it and the
    // mean are `sql` fragments; the counts and predicates are fully typed.
    // Conditional counts are `sum(CASE WHEN cond THEN 1 ELSE 0 END)` since Diesel's
    // aggregate FILTER is not available on `count(*)`. Columns are table-qualified
    // so the join to pipelines can never make them ambiguous.
    let avg_ms = sql::<SqlNullable<BigInt>>(
        "round(avg(EXTRACT(EPOCH FROM (workspace_pipeline_runs.completed_at \
         - workspace_pipeline_runs.started_at))) \
         FILTER (WHERE workspace_pipeline_runs.completed_at IS NOT NULL) * 1000)::bigint",
    );
    let p95_ms = sql::<SqlNullable<BigInt>>(
        "round(percentile_cont(0.95) WITHIN GROUP \
         (ORDER BY EXTRACT(EPOCH FROM (workspace_pipeline_runs.completed_at \
         - workspace_pipeline_runs.started_at))) \
         FILTER (WHERE workspace_pipeline_runs.completed_at IS NOT NULL) * 1000)::bigint",
    );

    workspace_pipeline_runs::table
        .inner_join(workspace_pipelines::table)
        .filter(pipelines::workspace_id.eq(workspace_id))
        .filter(pipelines::deleted_at.is_null())
        .filter(runs::started_at.ge(from))
        .filter(runs::started_at.lt(to))
        .group_by(run_day())
        .select((
            run_day(),
            count_star(),
            sum(
                case_when::<_, _, BigInt>(runs::status.eq_any(PipelineRunStatus::OUTCOMES), 1i64)
                    .otherwise(0i64),
            ),
            sum(
                case_when::<_, _, BigInt>(runs::status.eq(PipelineRunStatus::Failed), 1i64)
                    .otherwise(0i64),
            ),
            avg_ms,
            p95_ms,
        ))
        .load(conn)
        .await
        .map_err(PgError::from)
}

/// Per-day inference token totals over `[from, to)`, scoped through the live
/// pipeline. Usage is per-model-per-run, so each token field is summed per run
/// first (a correlated subquery) before day-grouping, otherwise a run's multiple
/// model rows would multiply the day totals. Sparse: only days with a run (token
/// sums are null on days with no usage).
async fn load_run_day_tokens(
    conn: &mut PgConnection,
    workspace_id: Uuid,
    from: jiff_diesel::Timestamp,
    to: jiff_diesel::Timestamp,
) -> PgResult<Vec<RunDayTokens>> {
    use diesel::dsl::sum;
    use schema::workspace_pipeline_run_usage::dsl as usage;
    use schema::workspace_pipeline_runs::dsl as runs;
    use schema::workspace_pipelines::dsl as pipelines;
    use schema::{workspace_pipeline_run_usage, workspace_pipeline_runs, workspace_pipelines};

    let per_run_input = workspace_pipeline_run_usage::table
        .filter(usage::run_id.eq(runs::id))
        .select(sum(usage::input_tokens))
        .single_value();
    let per_run_output = workspace_pipeline_run_usage::table
        .filter(usage::run_id.eq(runs::id))
        .select(sum(usage::output_tokens))
        .single_value();
    let per_run_total = workspace_pipeline_run_usage::table
        .filter(usage::run_id.eq(runs::id))
        .select(sum(usage::total_tokens))
        .single_value();

    workspace_pipeline_runs::table
        .inner_join(workspace_pipelines::table)
        .filter(pipelines::workspace_id.eq(workspace_id))
        .filter(pipelines::deleted_at.is_null())
        .filter(runs::started_at.ge(from))
        .filter(runs::started_at.lt(to))
        .group_by(run_day())
        .select((
            run_day(),
            sum(per_run_input),
            sum(per_run_output),
            sum(per_run_total),
        ))
        .load(conn)
        .await
        .map_err(PgError::from)
}
