//! Workspace analytics: aggregate queries over a workspace's files and pipeline
//! runs, for the analytics endpoint.
//!
//! Each method is a single grouped aggregate. The rows are returned as-is (one
//! per group, and never zero-filled here) — the handler maps them onto the full
//! set of enum values, filling absent groups with zero, so the response is stable
//! regardless of what data exists.

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
    avg_seconds: Option<f64>,
    p95_seconds: Option<f64>,
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
    /// Mean duration (seconds) of runs that completed this day; null if none did.
    pub avg_seconds: Option<f64>,
    /// 95th-percentile duration (seconds) of runs completed this day; null if none.
    pub p95_seconds: Option<f64>,
    /// Input/prompt tokens across models used by this day's runs; null if none.
    pub input_tokens: Option<i64>,
    /// Output/completion tokens across models used by this day's runs; null if none.
    pub output_tokens: Option<i64>,
    /// Reported total tokens across models used by this day's runs; null if none.
    pub total_tokens: Option<i64>,
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

/// Completed-run duration summary for a workspace, in seconds. Both are `None`
/// until at least one run has completed.
#[derive(Debug, Clone, Queryable)]
pub struct RunDurations {
    /// Mean wall-clock duration of completed runs.
    pub avg_seconds: Option<f64>,
    /// 95th-percentile duration of completed runs.
    pub p95_seconds: Option<f64>,
}

/// Read-only aggregate queries backing the workspace analytics endpoint.
pub trait WorkspaceAnalyticsRepository {
    /// Live-file count and byte total per `file_kind` for a workspace. Only kinds
    /// with at least one live file appear; the caller zero-fills the rest.
    fn storage_by_kind(
        &mut self,
        workspace_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<StorageByKind>>> + Send;

    /// Run count per `status` for a workspace (runs scoped through their live
    /// pipeline). Only statuses with at least one run appear.
    fn runs_by_status(
        &mut self,
        workspace_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<RunStatusCount>>> + Send;

    /// Mean and 95th-percentile duration of a workspace's completed runs, in
    /// seconds. Both `None` when no run has completed.
    fn run_durations(
        &mut self,
        workspace_id: Uuid,
    ) -> impl Future<Output = PgResult<RunDurations>> + Send;

    /// Inference token totals per model across a workspace's runs (scoped through
    /// their live pipeline). Only models that were actually used appear. Kept
    /// per-model rather than summed to a single total, since a run may mix models.
    fn usage_by_model(
        &mut self,
        workspace_id: Uuid,
    ) -> impl Future<Output = PgResult<Vec<UsageByModel>>> + Send;

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
    async fn storage_by_kind(&mut self, workspace_id: Uuid) -> PgResult<Vec<StorageByKind>> {
        use diesel::dsl::{count_star, sum};
        use schema::workspace_files::{self, dsl};

        let rows = workspace_files::table
            .filter(dsl::workspace_id.eq(workspace_id))
            .filter(dsl::deleted_at.is_null())
            .group_by(dsl::file_kind)
            .select((dsl::file_kind, count_star(), sum(dsl::file_size_bytes)))
            .load::<(FileKind, i64, Option<BigDecimal>)>(self)
            .await
            .map_err(PgError::from)?;

        // Byte totals are converted to i64 at the boundary so callers do not
        // depend on BigDecimal. NULL (an empty group) and the physically
        // impossible i64 overflow both fall back to 0, so a failed conversion
        // never fabricates a huge total that would poison the workspace sum.
        use bigdecimal::ToPrimitive;
        Ok(rows
            .into_iter()
            .map(|(file_kind, file_count, total_bytes)| StorageByKind {
                file_kind,
                file_count,
                total_bytes: total_bytes.and_then(|b| b.to_i64()).unwrap_or(0),
            })
            .collect())
    }

    async fn runs_by_status(&mut self, workspace_id: Uuid) -> PgResult<Vec<RunStatusCount>> {
        use diesel::dsl::count_star;
        use schema::workspace_pipeline_runs::dsl as runs;
        use schema::workspace_pipelines::dsl as pipelines;
        use schema::{workspace_pipeline_runs, workspace_pipelines};

        let rows = workspace_pipeline_runs::table
            .inner_join(workspace_pipelines::table)
            .filter(pipelines::workspace_id.eq(workspace_id))
            .filter(pipelines::deleted_at.is_null())
            .group_by(runs::status)
            .select((runs::status, count_star()))
            .load::<(PipelineRunStatus, i64)>(self)
            .await
            .map_err(PgError::from)?;

        Ok(rows
            .into_iter()
            .map(|(status, count)| RunStatusCount { status, count })
            .collect())
    }

    async fn run_durations(&mut self, workspace_id: Uuid) -> PgResult<RunDurations> {
        use diesel::dsl::{avg, sql};
        use diesel::sql_types::{Double, Nullable};
        use schema::workspace_pipeline_runs::dsl as runs;
        use schema::workspace_pipelines::dsl as pipelines;
        use schema::{workspace_pipeline_runs, workspace_pipelines};

        // Duration in seconds; `avg` and `percentile_cont` (an ordered-set
        // aggregate with no Diesel builtin) both return NULL over no rows. Columns
        // are table-qualified so the join can never make them ambiguous.
        let duration_secs = sql::<Nullable<Double>>(
            "EXTRACT(EPOCH FROM (workspace_pipeline_runs.completed_at \
             - workspace_pipeline_runs.started_at))",
        );
        let p95 = sql::<Nullable<Double>>(
            "percentile_cont(0.95) WITHIN GROUP \
             (ORDER BY EXTRACT(EPOCH FROM (workspace_pipeline_runs.completed_at \
             - workspace_pipeline_runs.started_at)))",
        );

        let (avg_seconds, p95_seconds) = workspace_pipeline_runs::table
            .inner_join(workspace_pipelines::table)
            .filter(pipelines::workspace_id.eq(workspace_id))
            .filter(pipelines::deleted_at.is_null())
            .filter(runs::completed_at.is_not_null())
            .select((avg(duration_secs), p95))
            .first::<(Option<f64>, Option<f64>)>(self)
            .await
            .map_err(PgError::from)?;

        Ok(RunDurations {
            avg_seconds,
            p95_seconds,
        })
    }

    async fn usage_by_model(&mut self, workspace_id: Uuid) -> PgResult<Vec<UsageByModel>> {
        use diesel::dsl::sum;
        use schema::workspace_pipeline_run_usage::dsl as usage;
        use schema::workspace_pipeline_runs::dsl as runs;
        use schema::workspace_pipelines::dsl as pipelines;
        use schema::{workspace_pipeline_run_usage, workspace_pipeline_runs, workspace_pipelines};

        let rows = workspace_pipeline_run_usage::table
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
            .load::<(
                String,
                Option<BigDecimal>,
                Option<BigDecimal>,
                Option<BigDecimal>,
            )>(self)
            .await
            .map_err(PgError::from)?;

        use bigdecimal::ToPrimitive;
        let to_i64 = |v: Option<BigDecimal>| v.and_then(|b| b.to_i64());
        Ok(rows
            .into_iter()
            .map(|(model, input, output, total)| UsageByModel {
                model,
                input_tokens: to_i64(input),
                output_tokens: to_i64(output),
                total_tokens: to_i64(total),
            })
            .collect())
    }

    async fn runs_by_day(
        &mut self,
        workspace_id: Uuid,
        from: jiff::Timestamp,
        to: jiff::Timestamp,
    ) -> PgResult<Vec<RunDayPoint>> {
        use std::collections::BTreeMap;

        use diesel_async::AsyncConnection;

        let from = jiff_diesel::Timestamp::from(from);
        let to = jiff_diesel::Timestamp::from(to);

        // Read the run counts/durations and the token totals in one transaction so
        // both grouped statements observe the same snapshot; otherwise a run
        // written between them could show in one series but not the other.
        let (run_rows, token_rows): (Vec<RunDayCounts>, Vec<RunDayTokens>) = self
            .transaction(async |conn| {
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
                    avg_seconds: row.avg_seconds,
                    p95_seconds: row.p95_seconds,
                    input_tokens: t.input,
                    output_tokens: t.output,
                    total_tokens: t.total,
                }
            })
            .collect();

        Ok(points)
    }
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
    use diesel::sql_types::{BigInt, Double, Nullable as SqlNullable};
    use schema::workspace_pipeline_runs::dsl as runs;
    use schema::workspace_pipelines::dsl as pipelines;
    use schema::{workspace_pipeline_runs, workspace_pipelines};

    // The day bucket a run's start falls in — the group key, also selected. This
    // and the ordered-set percentile lack a Diesel builtin, so they are `sql`
    // fragments; the counts and predicates are fully typed. Conditional counts are
    // `sum(CASE WHEN cond THEN 1 ELSE 0 END)` since Diesel's aggregate FILTER is
    // not available on `count(*)`. Columns are table-qualified so the join to
    // pipelines can never make them ambiguous, even if that table later gains a
    // same-named column.
    let day = || sql::<Timestamptz>("date_trunc('day', workspace_pipeline_runs.started_at)");
    let avg_secs = sql::<SqlNullable<Double>>(
        "avg(EXTRACT(EPOCH FROM (workspace_pipeline_runs.completed_at \
         - workspace_pipeline_runs.started_at))) \
         FILTER (WHERE workspace_pipeline_runs.completed_at IS NOT NULL)",
    );
    let p95_secs = sql::<SqlNullable<Double>>(
        "percentile_cont(0.95) WITHIN GROUP \
         (ORDER BY EXTRACT(EPOCH FROM (workspace_pipeline_runs.completed_at \
         - workspace_pipeline_runs.started_at))) \
         FILTER (WHERE workspace_pipeline_runs.completed_at IS NOT NULL)",
    );

    workspace_pipeline_runs::table
        .inner_join(workspace_pipelines::table)
        .filter(pipelines::workspace_id.eq(workspace_id))
        .filter(pipelines::deleted_at.is_null())
        .filter(runs::started_at.ge(from))
        .filter(runs::started_at.lt(to))
        .group_by(day())
        .select((
            day(),
            count_star(),
            sum(
                case_when::<_, _, BigInt>(runs::status.eq_any(PipelineRunStatus::OUTCOMES), 1i64)
                    .otherwise(0i64),
            ),
            sum(
                case_when::<_, _, BigInt>(runs::status.eq(PipelineRunStatus::Failed), 1i64)
                    .otherwise(0i64),
            ),
            avg_secs,
            p95_secs,
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
    use diesel::dsl::{sql, sum};
    use schema::workspace_pipeline_run_usage::dsl as usage;
    use schema::workspace_pipeline_runs::dsl as runs;
    use schema::workspace_pipelines::dsl as pipelines;
    use schema::{workspace_pipeline_run_usage, workspace_pipeline_runs, workspace_pipelines};

    let day = || sql::<Timestamptz>("date_trunc('day', workspace_pipeline_runs.started_at)");
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
        .group_by(day())
        .select((
            day(),
            sum(per_run_input),
            sum(per_run_output),
            sum(per_run_total),
        ))
        .load(conn)
        .await
        .map_err(PgError::from)
}
