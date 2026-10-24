//! Workspace analytics: aggregate queries over a workspace's files and
//! detections, for the analytics endpoint.
//!
//! Two entry points, each reading a consistent snapshot in one read-only
//! transaction: `snapshot` (storage, detection health, inference usage) and
//! `detections_by_day` (the daily time series). Both compose several grouped
//! aggregates from private `load_*` helpers. Rows come back sparse (one per group
//! that has data, never zero-filled here) — the handler maps them onto the full
//! set of enum values, filling absent groups with zero, so the response is stable
//! regardless of what data exists.

use std::collections::{BTreeMap, HashMap};
use std::future::Future;

use bigdecimal::BigDecimal;
use diesel::prelude::*;
use diesel::sql_types::Timestamptz;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::types::{DetectionStatus, DocumentKind, UsageMeter};
use crate::{Error, PgConnection, Result, schema};

/// Per-day detection counts and durations, as loaded from the grouped detection
/// query.
#[derive(Debug, Clone, Queryable)]
struct DetectionDayCounts {
    day: jiff_diesel::Timestamp,
    detections: i64,
    // Conditional counts are `sum(CASE WHEN cond THEN 1 ELSE 0 END)`; Postgres
    // `sum` of bigint returns numeric, hence BigDecimal (nullable over no rows).
    // Converted to i64 when assembling the point.
    terminal: Option<BigDecimal>,
    failed: Option<BigDecimal>,
    // Duration in milliseconds, scaled and rounded to a bigint in SQL.
    avg_ms: Option<i64>,
    p95_ms: Option<i64>,
}

/// One `(day, meter)` summed amount, as loaded from the grouped usage query.
///
/// Loaded by name (`sql_query`) because the grouping mixes a `date_trunc` day
/// expression with a typed enum column, which the typed query builder's
/// group-by machinery cannot express together.
#[derive(Debug, Clone, QueryableByName)]
struct DetectionDayMeter {
    #[diesel(sql_type = Timestamptz)]
    day: jiff_diesel::Timestamp,
    #[diesel(sql_type = crate::schema::sql_types::UsageMeter)]
    meter: UsageMeter,
    #[diesel(sql_type = diesel::sql_types::Nullable<diesel::sql_types::Numeric>)]
    amount: Option<BigDecimal>,
}

/// One day of detection activity for a workspace. Only days with at least one
/// detection are produced; the caller fills the gaps to a dense series over its
/// window. `day` is the UTC day (midnight) the detection counts fall in.
#[derive(Debug, Clone)]
pub struct DetectionDayPoint {
    /// The day (UTC midnight) this point covers.
    pub day: jiff::Timestamp,
    /// Detections started on this day.
    pub detections: i64,
    /// Detections that reached a terminal state (`complete` or `failed`) this day.
    pub terminal: i64,
    /// Detections that failed this day.
    pub failed: i64,
    /// Mean duration (milliseconds) of detections that completed this day; null if none did.
    pub avg_ms: Option<i64>,
    /// 95th-percentile duration (milliseconds) of detections completed this day; null if none.
    pub p95_ms: Option<i64>,
    /// Summed amount per meter across this day's detections; empty on a day with
    /// no metered usage. A value may be fractional (seconds), hence `f64`.
    pub meters: HashMap<UsageMeter, f64>,
}

/// One `(model, meter)` summed amount as loaded; `SUM(numeric)` arrives as
/// `BigDecimal` and is narrowed to `f64` (a meter may be fractional seconds).
#[derive(Debug, Clone, Queryable)]
struct UsageMeterRow {
    model: String,
    meter: UsageMeter,
    amount: Option<BigDecimal>,
}

/// One model's call and error counts across a workspace's detections, as loaded.
/// The error count is a `SUM(CASE …)` so it arrives as `numeric` (`BigDecimal`).
#[derive(Debug, Clone, Queryable)]
struct UsageCallRow {
    model: String,
    calls: i64,
    errors: Option<BigDecimal>,
}

/// Inference usage for one model across a workspace's detections: how much of
/// each billing meter it spent, and how many calls it made (and how many failed).
///
/// `meters` carries only the meters this model actually reported; a value may be
/// fractional (seconds), hence `f64`.
#[derive(Debug, Clone)]
pub struct UsageByModel {
    /// The model these totals are for.
    pub model: String,
    /// Summed amount per meter the model reported.
    pub meters: HashMap<UsageMeter, f64>,
    /// How many calls to this model the workspace's detections made.
    pub calls: i64,
    /// How many of those calls failed (outcome other than `ok`).
    pub errors: i64,
}

/// Live-document count and byte total for one document `kind`, as loaded.
/// `SUM(bigint)` is `numeric`, so the byte total arrives as `BigDecimal` and is
/// narrowed to `i64` when building [`StorageByKind`].
#[derive(Debug, Clone, Queryable)]
struct StorageKindRow {
    kind: DocumentKind,
    file_count: i64,
    total_bytes: Option<BigDecimal>,
}

/// Live-document count and byte total for one document `kind` in a workspace.
#[derive(Debug, Clone)]
pub struct StorageByKind {
    /// The document kind this row aggregates.
    pub kind: DocumentKind,
    /// Number of live documents of this kind.
    pub file_count: i64,
    /// Total bytes of live documents of this kind.
    pub total_bytes: i64,
}

/// Detection count for one `status` in a workspace.
#[derive(Debug, Clone, Queryable)]
pub struct DetectionStatusCount {
    /// The detection status this row aggregates.
    pub status: DetectionStatus,
    /// Number of detections in this status.
    pub count: i64,
}

/// Completed-detection duration summary for a workspace, in milliseconds. Both
/// are `None` until at least one detection has completed.
#[derive(Debug, Clone)]
pub struct DetectionDurations {
    /// Mean wall-clock duration of completed detections.
    pub avg_ms: Option<i64>,
    /// 95th-percentile duration of completed detections.
    pub p95_ms: Option<i64>,
}

/// A workspace's point-in-time analytics: storage, detection health, and
/// inference usage read together so the parts agree. Produced by
/// [`snapshot`](WorkspaceAnalyticsRepository::snapshot).
#[derive(Debug, Clone)]
pub struct AnalyticsSnapshot {
    /// Live-document counts and byte totals, one per document `kind` present.
    pub storage: Vec<StorageByKind>,
    /// Detection counts, one per `status` present.
    pub detections: Vec<DetectionStatusCount>,
    /// Completed-detection duration summary.
    pub durations: DetectionDurations,
    /// Inference usage, one entry per model used.
    pub usage: Vec<UsageByModel>,
}

/// Read-only aggregate queries backing the workspace analytics endpoint.
pub trait WorkspaceAnalyticsRepository {
    /// A workspace's point-in-time analytics — storage by kind, detection counts
    /// by status, completed-detection durations, and per-model inference usage —
    /// read in one read-only, repeatable-read transaction so the parts reflect a single
    /// snapshot. Each breakdown lists only the groups that have data; the caller
    /// zero-fills the rest.
    fn snapshot(
        &mut self,
        workspace_id: Uuid,
    ) -> impl Future<Output = Result<AnalyticsSnapshot>> + Send;

    /// Daily detection activity for a workspace over `[from, to)` (UTC day
    /// boundaries; `to` exclusive). Returns one row per day that has at least one
    /// detection — the series is sparse — so the caller gap-fills the window to a
    /// dense series (see `DetectionTimeSeries::from_window`). The window is
    /// bucketed by `date_trunc('day', started_at)`; detections are scoped through
    /// the detection. The caller is responsible for bounding the window.
    ///
    /// Detection counts, durations, and per-meter totals are read in one
    /// transaction so the two grouped statements observe the same snapshot.
    fn detections_by_day(
        &mut self,
        workspace_id: Uuid,
        from: jiff::Timestamp,
        to: jiff::Timestamp,
    ) -> impl Future<Output = Result<Vec<DetectionDayPoint>>> + Send;
}

impl WorkspaceAnalyticsRepository for PgConnection {
    async fn snapshot(&mut self, workspace_id: Uuid) -> Result<AnalyticsSnapshot> {
        // The four reads run in one read-only, repeatable-read transaction so the
        // snapshot is internally consistent: a detection cannot appear in the
        // status counts while its metered usage is missing from the usage totals
        // because a write landed between two of the reads.
        self.build_transaction()
            .read_only()
            .repeatable_read()
            .run(async |conn| {
                Ok(AnalyticsSnapshot {
                    storage: load_storage_by_kind(conn, workspace_id).await?,
                    detections: load_detections_by_status(conn, workspace_id).await?,
                    durations: load_detection_durations(conn, workspace_id).await?,
                    usage: load_usage_by_model(conn, workspace_id).await?,
                })
            })
            .await
    }

    async fn detections_by_day(
        &mut self,
        workspace_id: Uuid,
        from: jiff::Timestamp,
        to: jiff::Timestamp,
    ) -> Result<Vec<DetectionDayPoint>> {
        use std::collections::BTreeMap;

        use bigdecimal::ToPrimitive;

        let from = jiff_diesel::Timestamp::from(from);
        let to = jiff_diesel::Timestamp::from(to);

        // Read the detection counts/durations and the per-meter totals in one
        // read-only, repeatable-read transaction so both grouped statements
        // observe the same snapshot; otherwise a detection written between them
        // could show in one series but not the other.
        let (detection_rows, meter_rows): (Vec<DetectionDayCounts>, Vec<DetectionDayMeter>) = self
            .build_transaction()
            .read_only()
            .repeatable_read()
            .run(async |conn| {
                let detection_rows =
                    load_detection_day_counts(conn, workspace_id, from, to).await?;
                let meter_rows = load_detection_day_meters(conn, workspace_id, from, to).await?;
                Ok::<_, Error>((detection_rows, meter_rows))
            })
            .await?;

        // Merge the two per-day results by day: fold the (day, meter) rows into a
        // per-day meter map, then attach it to each detection day.
        let to_i64 = |v: Option<BigDecimal>| v.and_then(|b| b.to_i64());
        let mut meters: BTreeMap<jiff::Timestamp, HashMap<UsageMeter, f64>> = BTreeMap::new();
        for row in meter_rows {
            if let Some(amount) = row.amount.as_ref().and_then(BigDecimal::to_f64) {
                meters
                    .entry(row.day.into())
                    .or_default()
                    .insert(row.meter, amount);
            }
        }

        let points = detection_rows
            .into_iter()
            .map(|row| {
                let day: jiff::Timestamp = row.day.into();
                DetectionDayPoint {
                    day,
                    detections: row.detections,
                    terminal: to_i64(row.terminal).unwrap_or(0),
                    failed: to_i64(row.failed).unwrap_or(0),
                    avg_ms: row.avg_ms,
                    p95_ms: row.p95_ms,
                    meters: meters.remove(&day).unwrap_or_default(),
                }
            })
            .collect();

        Ok(points)
    }
}

/// Live-document count and byte total per document `kind`. Bytes live on the
/// blob, so each document is joined to its blob for the size. Only kinds with a
/// live document appear; the caller zero-fills the rest.
async fn load_storage_by_kind(
    conn: &mut PgConnection,
    workspace_id: Uuid,
) -> Result<Vec<StorageByKind>> {
    use bigdecimal::ToPrimitive;
    use diesel::dsl::{count_star, sum};
    use schema::workspace_blobs::dsl as blobs;
    use schema::workspace_documents::dsl as documents;
    use schema::{workspace_blobs, workspace_documents};

    let rows: Vec<StorageKindRow> = workspace_documents::table
        .inner_join(workspace_blobs::table)
        .filter(documents::workspace_id.eq(workspace_id))
        .filter(documents::deleted_at.is_null())
        .group_by(documents::kind)
        .select((documents::kind, count_star(), sum(blobs::file_size_bytes)))
        .load(conn)
        .await
        .map_err(Error::from)?;

    // Byte totals are converted to i64 at the boundary so callers do not depend
    // on BigDecimal. NULL (an empty group) and the physically impossible i64
    // overflow both fall back to 0, so a failed conversion never fabricates a
    // huge total that would poison the workspace sum.
    Ok(rows
        .into_iter()
        .map(|row| StorageByKind {
            kind: row.kind,
            file_count: row.file_count,
            total_bytes: row.total_bytes.and_then(|b| b.to_i64()).unwrap_or(0),
        })
        .collect())
}

/// Detection count per `status`, Only statuses
/// with a detection appear.
async fn load_detections_by_status(
    conn: &mut PgConnection,
    workspace_id: Uuid,
) -> Result<Vec<DetectionStatusCount>> {
    use diesel::dsl::count_star;
    use schema::workspace_detections;
    use schema::workspace_detections::dsl as detections;

    workspace_detections::table
        .filter(detections::workspace_id.eq(workspace_id))
        .group_by(detections::status)
        .select((detections::status, count_star()))
        .load(conn)
        .await
        .map_err(Error::from)
}

/// Mean and 95th-percentile duration (milliseconds) of the workspace's completed
/// detections. Both `None` when no detection has completed.
async fn load_detection_durations(
    conn: &mut PgConnection,
    workspace_id: Uuid,
) -> Result<DetectionDurations> {
    use diesel::dsl::sql;
    use diesel::sql_types::{BigInt, Nullable};
    use schema::workspace_detections;
    use schema::workspace_detections::dsl as detections;

    // Duration in milliseconds: the interval's epoch-seconds are scaled by 1000
    // and rounded to a bigint in SQL, so the value crosses the boundary already in
    // the API unit and type. `avg` and `percentile_cont` (an ordered-set aggregate
    // with no Diesel builtin) both return NULL over no rows. Columns are
    // table-qualified so the join can never make them ambiguous.
    let avg_ms = sql::<Nullable<BigInt>>(
        "round(avg(EXTRACT(EPOCH FROM (workspace_detections.completed_at \
         - workspace_detections.started_at)) * 1000))::bigint",
    );
    let p95_ms = sql::<Nullable<BigInt>>(
        "round(percentile_cont(0.95) WITHIN GROUP \
         (ORDER BY EXTRACT(EPOCH FROM (workspace_detections.completed_at \
         - workspace_detections.started_at))) * 1000)::bigint",
    );

    let (avg_ms, p95_ms): (Option<i64>, Option<i64>) = workspace_detections::table
        .filter(detections::workspace_id.eq(workspace_id))
        // Durations describe successful analysis only. `completed_at` is stamped
        // on failure too, so filter on the terminal `Complete` status, not merely
        // on the timestamp being present.
        .filter(detections::status.eq(DetectionStatus::Complete))
        .select((avg_ms, p95_ms))
        .first(conn)
        .await
        .map_err(Error::from)?;

    Ok(DetectionDurations { avg_ms, p95_ms })
}

/// Inference usage per model across the workspace's detections — summed per
/// meter, plus call and error counts — Only
/// models actually used appear. Kept per-model rather than summed to one total,
/// since a detection may mix models.
async fn load_usage_by_model(
    conn: &mut PgConnection,
    workspace_id: Uuid,
) -> Result<Vec<UsageByModel>> {
    use bigdecimal::ToPrimitive;
    use diesel::dsl::{case_when, count_star, sum};
    use diesel::sql_types::BigInt;
    use schema::workspace_detection_call::dsl as call;
    use schema::workspace_detection_call_meter::dsl as meter;
    use schema::workspace_detections::dsl as detections;
    use schema::{workspace_detection_call, workspace_detection_call_meter, workspace_detections};

    // Two reads merged by model rather than one join: joining calls to their
    // meters fans each call out by its meter count, which would inflate the
    // call/error counts. So sum meters grouped by (model, meter) in one query and
    // count calls grouped by model in another.
    let meter_rows: Vec<UsageMeterRow> = workspace_detection_call_meter::table
        .inner_join(workspace_detection_call::table.on(call::id.eq(meter::call_id)))
        .inner_join(workspace_detections::table.on(detections::id.eq(call::detection_id)))
        .filter(detections::workspace_id.eq(workspace_id))
        .group_by((call::model, meter::meter))
        .select((call::model, meter::meter, sum(meter::amount)))
        .load(conn)
        .await
        .map_err(Error::from)?;

    // Errors as `SUM(CASE WHEN outcome <> 'ok' THEN 1 ELSE 0 END)` (this diesel
    // has no filtered count), alongside the total call count.
    let error_count = sum(case_when::<_, _, BigInt>(call::outcome.ne("ok"), 1i64).otherwise(0i64));
    let call_rows: Vec<UsageCallRow> = workspace_detection_call::table
        .inner_join(workspace_detections::table.on(detections::id.eq(call::detection_id)))
        .filter(detections::workspace_id.eq(workspace_id))
        .group_by(call::model)
        .select((call::model, count_star(), error_count))
        .load(conn)
        .await
        .map_err(Error::from)?;

    // Merge both by model, preserving model order from the call query (every model
    // that has meters also has at least one call row).
    let mut by_model: BTreeMap<String, UsageByModel> = BTreeMap::new();
    for row in call_rows {
        by_model.insert(
            row.model.clone(),
            UsageByModel {
                model: row.model,
                meters: HashMap::new(),
                calls: row.calls,
                errors: row.errors.and_then(|b| b.to_i64()).unwrap_or(0),
            },
        );
    }
    for row in meter_rows {
        if let Some(amount) = row.amount.as_ref().and_then(BigDecimal::to_f64)
            && let Some(entry) = by_model.get_mut(&row.model)
        {
            entry.meters.insert(row.meter, amount);
        }
    }

    Ok(by_model.into_values().collect())
}

/// The day bucket a detection's start falls in. Shared by both daily queries so
/// the truncation text cannot drift — they must group on the same expression for
/// the per-day merge to line up. The column is table-qualified so the join to
/// Returns a fresh fragment per call, as
/// the builder consumes it in both `group_by` and `select`.
fn detection_day() -> diesel::expression::SqlLiteral<Timestamptz> {
    diesel::dsl::sql::<Timestamptz>("date_trunc('day', workspace_detections.started_at)")
}

/// Per-day detection counts and durations over `[from, to)`. Sparse: only days that
/// have a detection. Shared with the per-meter query so both observe the same
/// snapshot inside one transaction.
async fn load_detection_day_counts(
    conn: &mut PgConnection,
    workspace_id: Uuid,
    from: jiff_diesel::Timestamp,
    to: jiff_diesel::Timestamp,
) -> Result<Vec<DetectionDayCounts>> {
    use diesel::dsl::{case_when, count_star, sql, sum};
    use diesel::sql_types::{BigInt, Nullable as SqlNullable};
    use schema::workspace_detections;
    use schema::workspace_detections::dsl as detections;

    // Durations in milliseconds (epoch-seconds scaled by 1000, rounded to bigint),
    // so the value crosses the boundary already in the API unit and type. The
    // percentile is an ordered-set aggregate with no Diesel builtin, so it and the
    // mean are `sql` fragments; the counts and predicates are fully typed.
    // Conditional counts are `sum(CASE WHEN cond THEN 1 ELSE 0 END)` since Diesel's
    // aggregate FILTER is not available on `count(*)`. Columns are table-qualified
    //
    // Durations describe successful analysis only, so both aggregates filter on
    // the terminal `complete` status: `completed_at` is stamped on failure too, so
    // a presence check alone would fold failed detections into avg/p95. The FILTER
    // clause is shared so the two fragments cannot drift apart.
    const COMPLETE_FILTER: &str = " FILTER (WHERE workspace_detections.status = 'complete')";
    let avg_ms = sql::<SqlNullable<BigInt>>(&format!(
        "round(avg(EXTRACT(EPOCH FROM (workspace_detections.completed_at \
         - workspace_detections.started_at))){COMPLETE_FILTER} * 1000)::bigint"
    ));
    let p95_ms = sql::<SqlNullable<BigInt>>(&format!(
        "round(percentile_cont(0.95) WITHIN GROUP \
         (ORDER BY EXTRACT(EPOCH FROM (workspace_detections.completed_at \
         - workspace_detections.started_at))){COMPLETE_FILTER} * 1000)::bigint"
    ));

    workspace_detections::table
        .filter(detections::workspace_id.eq(workspace_id))
        .filter(detections::started_at.ge(from))
        .filter(detections::started_at.lt(to))
        .group_by(detection_day())
        .select((
            detection_day(),
            count_star(),
            sum(case_when::<_, _, BigInt>(
                detections::status.eq_any(DetectionStatus::TERMINAL),
                1i64,
            )
            .otherwise(0i64)),
            sum(
                case_when::<_, _, BigInt>(detections::status.eq(DetectionStatus::Failed), 1i64)
                    .otherwise(0i64),
            ),
            avg_ms,
            p95_ms,
        ))
        .load(conn)
        .await
        .map_err(Error::from)
}

/// Per-day inference usage over `[from, to)`, as `(day, meter, summed amount)`
/// rows, Each meter row belongs to exactly one
/// call and so one detection and day, so summing `amount` over the join never
/// double-counts — no per-detection dedup is needed (unlike detection counts).
/// Only days with metered usage appear; the caller gap-fills the window.
async fn load_detection_day_meters(
    conn: &mut PgConnection,
    workspace_id: Uuid,
    from: jiff_diesel::Timestamp,
    to: jiff_diesel::Timestamp,
) -> Result<Vec<DetectionDayMeter>> {
    use diesel::sql_types::{Timestamptz, Uuid as SqlUuid};

    // Raw SQL because the day expression and the meter enum column are grouped
    // together, which the typed builder's group-by cannot express. The join and
    // window mirror `load_detection_day_counts`; `[from, to)` is half-open.
    diesel::sql_query(
        "SELECT date_trunc('day', d.started_at) AS day, \
                m.meter AS meter, \
                SUM(m.amount) AS amount \
         FROM workspace_detection_call_meter m \
         JOIN workspace_detection_call c ON c.id = m.call_id \
         JOIN workspace_detections d ON d.id = c.detection_id \
         WHERE d.workspace_id = $1 AND d.started_at >= $2 AND d.started_at < $3 \
         GROUP BY day, m.meter",
    )
    .bind::<SqlUuid, _>(workspace_id)
    .bind::<Timestamptz, _>(from)
    .bind::<Timestamptz, _>(to)
    .load(conn)
    .await
    .map_err(Error::from)
}

#[cfg(test)]
mod tests {
    use jiff::{Span, Timestamp};

    use super::*;
    use crate::PgConn;
    use crate::model::{
        DetectionCallRecord, NewBlob, NewWorkspaceDetection, NewWorkspaceDetectionCall,
        NewWorkspaceDocument,
    };
    use crate::query::{
        WorkspaceAnalyticsRepository, WorkspaceDetectionRepository, WorkspaceDocumentRepository,
    };
    use crate::test_util::{TestDatabase, backdate};

    /// A successful token-billed model call on `detection_id`, reporting the given
    /// input/output/total token meters.
    fn token_call(
        detection_id: Uuid,
        model: &str,
        input: u64,
        output: u64,
        total: u64,
    ) -> DetectionCallRecord {
        DetectionCallRecord {
            call: NewWorkspaceDetectionCall::test(detection_id, model),
            meters: vec![
                (UsageMeter::TokensInput, BigDecimal::from(input)),
                (UsageMeter::TokensOutput, BigDecimal::from(output)),
                (UsageMeter::TokensTotal, BigDecimal::from(total)),
            ],
        }
    }

    /// Inserts a `Complete` detection that started `started_ago` in the past and
    /// finished `duration` later, returning its id — for the duration and
    /// per-day aggregates.
    async fn completed_detection(
        conn: &mut PgConn,
        workspace_id: Uuid,
        account_id: Uuid,
        input_document_id: Uuid,
        started_ago: Span,
        duration: Span,
    ) -> anyhow::Result<Uuid> {
        let started = Timestamp::now() - started_ago;
        let mut new = NewWorkspaceDetection::test(workspace_id, account_id, input_document_id);
        new.status = Some(DetectionStatus::Complete);
        let detection = conn.create_workspace_detection(new).await?;
        // Backdate the run span so the day-bucket and duration aggregates see a
        // known started/completed pair.
        backdate::detection_span(conn, detection.id, started, started + duration).await?;
        Ok(detection.id)
    }

    #[tokio::test]
    async fn snapshot_aggregates_storage_detections_and_usage_by_group() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_document().await;
        let mut conn = db.client.get_connection().await?;

        // Storage: the seeded original document plus a second original and a
        // redacted, each backed by its own blob.
        let mut redacted =
            NewWorkspaceDocument::test(seeded.workspace_id, seeded.account_id, Uuid::nil());
        redacted.kind = Some(DocumentKind::Redacted);
        let _ = conn
            .create_workspace_document(redacted, NewBlob::test(seeded.workspace_id))
            .await?;
        let _ = conn
            .create_workspace_document(
                NewWorkspaceDocument::test(seeded.workspace_id, seeded.account_id, Uuid::nil()),
                NewBlob::test(seeded.workspace_id),
            )
            .await?;

        // Detections: one Complete (with a duration) and one Pending.
        let complete = completed_detection(
            &mut conn,
            seeded.workspace_id,
            seeded.account_id,
            seeded.document_id,
            Span::new().hours(2),
            Span::new().seconds(10),
        )
        .await?;
        let _pending = conn
            .create_workspace_detection(NewWorkspaceDetection::test(
                seeded.workspace_id,
                seeded.account_id,
                seeded.document_id,
            ))
            .await?;

        // Usage: two model calls on the completed detection, each with token
        // meters.
        conn.record_detection_usage(vec![
            token_call(complete, "gpt-4o", 100, 20, 120),
            token_call(complete, "claude", 200, 40, 240),
        ])
        .await?;

        let snapshot = conn.snapshot(seeded.workspace_id).await?;

        // Storage: 2 original documents + 1 redacted, grouped by kind.
        let original = snapshot
            .storage
            .iter()
            .find(|s| s.kind == DocumentKind::Original)
            .expect("original kind present");
        assert_eq!(original.file_count, 2);
        assert!(
            snapshot
                .storage
                .iter()
                .any(|s| s.kind == DocumentKind::Redacted)
        );

        // Detections: one Complete, one Pending, grouped by status.
        let complete_count = snapshot
            .detections
            .iter()
            .find(|d| d.status == DetectionStatus::Complete)
            .map(|d| d.count);
        assert_eq!(complete_count, Some(1));
        assert!(
            snapshot
                .detections
                .iter()
                .any(|d| d.status == DetectionStatus::Pending && d.count == 1)
        );

        // Durations: a completed detection exists, so the summary is populated.
        assert_eq!(snapshot.durations.avg_ms, Some(10_000));

        // Usage: two models, token meters summed per model, one call each.
        let gpt = snapshot
            .usage
            .iter()
            .find(|u| u.model == "gpt-4o")
            .expect("gpt-4o usage present");
        assert_eq!(gpt.meters.get(&UsageMeter::TokensInput), Some(&100.0));
        assert_eq!(gpt.meters.get(&UsageMeter::TokensTotal), Some(&120.0));
        assert_eq!(gpt.calls, 1);
        assert_eq!(gpt.errors, 0);
        assert!(snapshot.usage.iter().any(|u| u.model == "claude"));
        Ok(())
    }

    #[tokio::test]
    async fn snapshot_is_scoped_to_the_workspace() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;

        // A different workspace with a detection.
        let other = db.seed_document().await;
        let mut conn = db.client.get_connection().await?;
        let _ = conn
            .create_workspace_detection(NewWorkspaceDetection::test(
                other.workspace_id,
                other.account_id,
                other.document_id,
            ))
            .await?;

        // The unrelated workspace sees none of it.
        let snapshot = conn.snapshot(seeded.workspace_id).await?;
        assert!(snapshot.detections.is_empty());
        assert!(snapshot.storage.is_empty());
        assert!(snapshot.usage.is_empty());

        // The owning workspace does.
        assert!(
            !conn
                .snapshot(other.workspace_id)
                .await?
                .detections
                .is_empty()
        );
        Ok(())
    }

    #[tokio::test]
    async fn detections_by_day_buckets_within_the_window() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_document().await;
        let mut conn = db.client.get_connection().await?;

        // Two detections started ~1 day ago (same UTC day), one ~3 days ago.
        let recent = completed_detection(
            &mut conn,
            seeded.workspace_id,
            seeded.account_id,
            seeded.document_id,
            Span::new().hours(25),
            Span::new().seconds(5),
        )
        .await?;
        let _recent2 = completed_detection(
            &mut conn,
            seeded.workspace_id,
            seeded.account_id,
            seeded.document_id,
            Span::new().hours(26),
            Span::new().seconds(5),
        )
        .await?;
        let _old = completed_detection(
            &mut conn,
            seeded.workspace_id,
            seeded.account_id,
            seeded.document_id,
            Span::new().hours(72),
            Span::new().seconds(5),
        )
        .await?;
        conn.record_detection_usage(vec![token_call(recent, "gpt-4o", 50, 10, 60)])
            .await?;

        // A window covering only the last two days excludes the 3-day-old one.
        let from = Timestamp::now() - Span::new().hours(48);
        let to = Timestamp::now() + Span::new().hours(1);
        let points = conn
            .detections_by_day(seeded.workspace_id, from, to)
            .await?;

        // One bucket (the two recent detections share a UTC day), 2 detections.
        let total: i64 = points.iter().map(|p| p.detections).sum();
        assert_eq!(total, 2, "the 3-day-old detection is outside the window");
        // The token meter from the call is merged onto that day.
        assert!(
            points
                .iter()
                .any(|p| p.meters.get(&UsageMeter::TokensInput) == Some(&50.0))
        );
        Ok(())
    }
}
