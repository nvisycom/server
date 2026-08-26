//! Workspace detections repository for managing analysis instances.

use std::future::Future;

use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::model::{
    NewWorkspaceDetection, NewWorkspaceDetectionUsage, UpdateWorkspaceDetection,
    WorkspaceDetection, WorkspacePipeline,
};
use crate::types::{
    AccountRefRow, CursorPage, CursorPagination, DetectionFilter, DetectionStatus, Handle,
};
use crate::{PgConnection, PgError, PgResult, schema};

/// Resolved display name of a detection's input file.
///
/// `None` when the file has been removed (e.g. by retention). Redacted outputs
/// belong to redactions, not the detection, so they are not resolved here.
#[derive(Debug, Default, Clone)]
pub struct DetectionFiles {
    /// Display name of the input document the detection analyzes.
    pub input: Option<String>,
}

/// One row of a detection listing: the detection plus the context a response
/// needs to render it without follow-up lookups — the triggering account, the
/// owning pipeline's slug, and the input file's display name (`None` if the file
/// was removed).
#[derive(Debug, Clone)]
pub struct DetectionListRow {
    /// The detection.
    pub detection: WorkspaceDetection,
    /// The account that triggered the detection.
    pub account: AccountRefRow,
    /// Slug of the detection's owning pipeline.
    pub pipeline_slug: Handle,
    /// Display name of the detection's input document, if still present.
    pub input_file_name: Option<String>,
}

/// Repository for workspace detection database operations.
///
/// Handles detection lifecycle management including creation, status updates,
/// completion tracking, and queries.
pub trait WorkspaceDetectionRepository {
    /// Creates a new workspace detection record.
    fn create_workspace_detection(
        &mut self,
        new_detection: NewWorkspaceDetection,
    ) -> impl Future<Output = PgResult<WorkspaceDetection>> + Send;

    /// Finds a detection by its opaque id, scoped to a workspace, returning the
    /// detection and its owning pipeline.
    ///
    /// The detection is addressed by its own id (behind `/detections/{detectionId}`);
    /// scoping through the owning pipeline keeps it workspace-bounded and hides
    /// detections of soft-deleted pipelines.
    fn find_workspace_detection_by_id(
        &mut self,
        workspace_id: Uuid,
        detection_id: Uuid,
    ) -> impl Future<Output = PgResult<Option<(WorkspaceDetection, WorkspacePipeline)>>> + Send;

    /// Finds a detection by its `(pipeline, idempotency key)` pair, for detect
    /// replay.
    fn find_detection_by_idempotency_key(
        &mut self,
        pipeline_id: Uuid,
        idempotency_key: &str,
    ) -> impl Future<Output = PgResult<Option<WorkspaceDetection>>> + Send;

    /// Lists a specific pipeline's detections with cursor pagination. `filter`
    /// narrows by status and/or file (its `pipeline_id` is ignored — the listing
    /// is already pipeline-scoped).
    fn cursor_list_pipeline_detections(
        &mut self,
        pipeline_id: Uuid,
        pagination: CursorPagination,
        filter: &DetectionFilter,
    ) -> impl Future<Output = PgResult<CursorPage<DetectionListRow>>> + Send;

    /// Lists all detections across a workspace's pipelines with cursor
    /// pagination.
    ///
    /// Detections carry no workspace reference of their own, so this joins through
    /// the owning pipeline and filters on its workspace. `filter` narrows by
    /// status, file, and/or owning pipeline; use [`cursor_list_pipeline_detections`]
    /// for a single pipeline.
    ///
    /// [`cursor_list_pipeline_detections`]: Self::cursor_list_pipeline_detections
    fn cursor_list_workspace_detections(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        filter: &DetectionFilter,
    ) -> impl Future<Output = PgResult<CursorPage<DetectionListRow>>> + Send;

    /// Atomically claims a detection, transitioning it to `Executing`.
    ///
    /// Succeeds (returning the claimed detection) only when it is still `Pending`,
    /// or is `Executing` but its previous claim has gone stale (`claimed_at`
    /// older than `stale_before` — a worker that died mid-analysis). A detection
    /// already executing under a fresh claim, or past the detect phase, yields
    /// `None` so a redelivered job skips it instead of analyzing twice.
    ///
    /// The claim stamps `claimed_at = now()`, so the lease renews on each
    /// (re)claim. Callers pass `stale_before = now - lease` computed against the
    /// same clock the DB uses closely enough for a lease measured in minutes.
    fn claim_detection(
        &mut self,
        detection_id: Uuid,
        stale_before: jiff::Timestamp,
    ) -> impl Future<Output = PgResult<Option<WorkspaceDetection>>> + Send;

    /// Resolves the display name of a detection's input file.
    ///
    /// An indexed lookup by id; a file removed (e.g. by retention) yields `None`.
    /// Used to name a single detection's input file in its response without
    /// threading a join through the shared detection lookup.
    fn detection_file_names(
        &mut self,
        workspace_id: Uuid,
        detection: &WorkspaceDetection,
    ) -> impl Future<Output = PgResult<DetectionFiles>> + Send;

    /// Updates a workspace detection with new data.
    fn update_workspace_detection(
        &mut self,
        detection_id: Uuid,
        updates: UpdateWorkspaceDetection,
    ) -> impl Future<Output = PgResult<WorkspaceDetection>> + Send;

    /// Transitions a detection to `Complete` only while the caller still holds
    /// its claim — the detection is still `Executing` and its `claimed_at` matches
    /// the value stamped when the caller claimed it. Returns `true` on success,
    /// `false` if the claim has gone stale (another worker re-claimed the
    /// detection after the lease expired), so the caller can abort without
    /// stamping over the new owner's work. `updates` carries the analyze results
    /// (audit file, metadata); status and the claim guard are applied here.
    fn finalize_detection(
        &mut self,
        detection_id: Uuid,
        claimed_at: jiff::Timestamp,
        updates: UpdateWorkspaceDetection,
    ) -> impl Future<Output = PgResult<bool>> + Send;

    /// Transitions a detection to `Failed` only while the caller still holds its
    /// claim — the detection is still `Executing` and its `claimed_at` matches the
    /// value stamped when the caller claimed it. Returns `true` on success,
    /// `false` if the claim has gone stale (another worker re-claimed the
    /// detection). Mirrors [`finalize_detection`](Self::finalize_detection) for
    /// the failure path, so a worker whose lease expired mid-analysis cannot fail
    /// a detection another worker now owns. `updates` carries the failure reason
    /// and completion time; status and the claim guard are applied here.
    fn fail_detection(
        &mut self,
        detection_id: Uuid,
        claimed_at: jiff::Timestamp,
        updates: UpdateWorkspaceDetection,
    ) -> impl Future<Output = PgResult<bool>> + Send;

    /// Records a detection's per-model inference usage. A no-op for an empty slice
    /// (a deterministic detection spends no tokens). Inserted once, at analyze
    /// time.
    fn record_detection_usage(
        &mut self,
        usage: &[NewWorkspaceDetectionUsage],
    ) -> impl Future<Output = PgResult<()>> + Send;
}

impl WorkspaceDetectionRepository for PgConnection {
    async fn create_workspace_detection(
        &mut self,
        new_detection: NewWorkspaceDetection,
    ) -> PgResult<WorkspaceDetection> {
        use schema::workspace_detections;

        let detection = diesel::insert_into(workspace_detections::table)
            .values(&new_detection)
            .returning(WorkspaceDetection::as_returning())
            .get_result(self)
            .await
            .map_err(PgError::from)?;

        Ok(detection)
    }

    async fn find_workspace_detection_by_id(
        &mut self,
        workspace_id: Uuid,
        detection_id: Uuid,
    ) -> PgResult<Option<(WorkspaceDetection, WorkspacePipeline)>> {
        use schema::workspace_detections::dsl as detections;
        use schema::{workspace_detections, workspace_pipelines};

        // Detections carry no workspace column; scope through the owning pipeline
        // so the id resolves only within its workspace, and only while that
        // pipeline is live (a soft-deleted pipeline hides its detections). The
        // pipeline is returned alongside so callers need no second lookup.
        let detection = workspace_detections::table
            .inner_join(workspace_pipelines::table)
            .filter(detections::id.eq(detection_id))
            .filter(workspace_pipelines::workspace_id.eq(workspace_id))
            .filter(workspace_pipelines::deleted_at.is_null())
            .select((
                WorkspaceDetection::as_select(),
                WorkspacePipeline::as_select(),
            ))
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(detection)
    }

    async fn find_detection_by_idempotency_key(
        &mut self,
        pipeline_id: Uuid,
        idempotency_key: &str,
    ) -> PgResult<Option<WorkspaceDetection>> {
        use schema::workspace_detections::{self, dsl};

        let detection = workspace_detections::table
            .filter(dsl::pipeline_id.eq(pipeline_id))
            .filter(dsl::idempotency_key.eq(idempotency_key))
            .select(WorkspaceDetection::as_select())
            .first(self)
            .await
            .optional()
            .map_err(PgError::from)?;

        Ok(detection)
    }

    async fn cursor_list_pipeline_detections(
        &mut self,
        pipeline_id: Uuid,
        pagination: CursorPagination,
        filter: &DetectionFilter,
    ) -> PgResult<CursorPage<DetectionListRow>> {
        use schema::workspace_detections::dsl;
        use schema::{accounts, workspace_detections, workspace_files, workspace_pipelines};

        // Build base query with filters. The listing is already scoped to one
        // pipeline, so `filter.pipeline_id` is not applied here.
        let mut base_query = workspace_detections::table
            .filter(dsl::pipeline_id.eq(pipeline_id))
            .into_boxed();

        if let Some(status) = filter.status {
            base_query = base_query.filter(dsl::status.eq(status));
        }
        if let Some(file_id) = filter.input_file_id {
            base_query = base_query.filter(dsl::input_file_id.eq(file_id));
        }
        if let Some(account_id) = filter.account_id {
            base_query = base_query.filter(dsl::account_id.eq(account_id));
        }
        if let Some(trigger_type) = filter.trigger_type {
            base_query = base_query.filter(dsl::trigger_type.eq(trigger_type));
        }

        let total = if pagination.include_count {
            Some(
                base_query
                    .count()
                    .get_result::<i64>(self)
                    .await
                    .map_err(PgError::from)?,
            )
        } else {
            None
        };

        // Rebuild query for fetching items. Join the owning pipeline (for its
        // slug) and the input file (to name the detection's analyzed document) so
        // a row is self-contained; a LEFT JOIN on the file tolerates one removed
        // by retention, yielding a null name.
        let mut query = workspace_detections::table
            .inner_join(accounts::table)
            .inner_join(workspace_pipelines::table)
            .left_join(workspace_files::table.on(dsl::input_file_id.eq(workspace_files::id)))
            .filter(dsl::pipeline_id.eq(pipeline_id))
            .into_boxed();

        if let Some(status) = filter.status {
            query = query.filter(dsl::status.eq(status));
        }
        if let Some(file_id) = filter.input_file_id {
            query = query.filter(dsl::input_file_id.eq(file_id));
        }
        if let Some(account_id) = filter.account_id {
            query = query.filter(dsl::account_id.eq(account_id));
        }
        if let Some(trigger_type) = filter.trigger_type {
            query = query.filter(dsl::trigger_type.eq(trigger_type));
        }

        let limit = pagination.fetch_limit();
        let selection = (
            WorkspaceDetection::as_select(),
            (
                accounts::username,
                accounts::display_name,
                accounts::avatar_url,
            ),
            workspace_pipelines::slug,
            workspace_files::display_name.nullable(),
        );

        let rows: Vec<(WorkspaceDetection, AccountRefRow, Handle, Option<String>)> =
            if let Some(cursor) = &pagination.after {
                let cursor_time = jiff_diesel::Timestamp::from(cursor.timestamp);

                query
                    .filter(
                        dsl::started_at
                            .lt(&cursor_time)
                            .or(dsl::started_at.eq(&cursor_time).and(dsl::id.lt(cursor.id))),
                    )
                    .select(selection)
                    .order((dsl::started_at.desc(), dsl::id.desc()))
                    .limit(limit)
                    .load(self)
                    .await
                    .map_err(PgError::from)?
            } else {
                query
                    .select(selection)
                    .order((dsl::started_at.desc(), dsl::id.desc()))
                    .limit(limit)
                    .load(self)
                    .await
                    .map_err(PgError::from)?
            };

        let items = rows
            .into_iter()
            .map(
                |(detection, account, pipeline_slug, input_file_name)| DetectionListRow {
                    detection,
                    account,
                    pipeline_slug,
                    input_file_name,
                },
            )
            .collect();

        Ok(CursorPage::new(items, total, pagination.limit, |row| {
            (row.detection.started_at.into(), row.detection.id)
        }))
    }

    async fn cursor_list_workspace_detections(
        &mut self,
        workspace_id: Uuid,
        pagination: CursorPagination,
        filter: &DetectionFilter,
    ) -> PgResult<CursorPage<DetectionListRow>> {
        use schema::accounts::dsl as accounts;
        use schema::workspace_detections::dsl as detections;
        use schema::workspace_files::dsl as files;
        use schema::workspace_pipelines::dsl as pipelines;

        // Detections have no workspace column; scope them through the owning
        // pipeline. The owning pipeline's slug, the triggering account, and the
        // input file's name are selected alongside each detection so the
        // cross-pipeline response can name its pipeline, trigger, and analyzed
        // document without a per-row lookup. The input file is LEFT-joined so a
        // file removed by retention yields a null name rather than dropping the
        // detection.
        let scoped = || {
            let mut query = detections::workspace_detections
                .inner_join(pipelines::workspace_pipelines)
                .inner_join(accounts::accounts)
                .left_join(files::workspace_files.on(detections::input_file_id.eq(files::id)))
                .filter(pipelines::workspace_id.eq(workspace_id))
                .into_boxed();
            if let Some(status) = filter.status {
                query = query.filter(detections::status.eq(status));
            }
            if let Some(file_id) = filter.input_file_id {
                query = query.filter(detections::input_file_id.eq(file_id));
            }
            if let Some(pipeline_id) = filter.pipeline_id {
                query = query.filter(detections::pipeline_id.eq(pipeline_id));
            }
            if let Some(account_id) = filter.account_id {
                query = query.filter(detections::account_id.eq(account_id));
            }
            if let Some(trigger_type) = filter.trigger_type {
                query = query.filter(detections::trigger_type.eq(trigger_type));
            }
            query
        };

        let total = if pagination.include_count {
            Some(
                scoped()
                    .count()
                    .get_result::<i64>(self)
                    .await
                    .map_err(PgError::from)?,
            )
        } else {
            None
        };

        let limit = pagination.fetch_limit();
        let selection = (
            WorkspaceDetection::as_select(),
            pipelines::slug,
            (
                accounts::username,
                accounts::display_name,
                accounts::avatar_url,
            ),
            files::display_name.nullable(),
        );

        let rows: Vec<(WorkspaceDetection, Handle, AccountRefRow, Option<String>)> =
            if let Some(cursor) = &pagination.after {
                let cursor_time = jiff_diesel::Timestamp::from(cursor.timestamp);

                scoped()
                    .filter(
                        detections::started_at
                            .lt(&cursor_time)
                            .or(detections::started_at
                                .eq(&cursor_time)
                                .and(detections::id.lt(cursor.id))),
                    )
                    .select(selection)
                    .order((detections::started_at.desc(), detections::id.desc()))
                    .limit(limit)
                    .load(self)
                    .await
                    .map_err(PgError::from)?
            } else {
                scoped()
                    .select(selection)
                    .order((detections::started_at.desc(), detections::id.desc()))
                    .limit(limit)
                    .load(self)
                    .await
                    .map_err(PgError::from)?
            };

        let items = rows
            .into_iter()
            .map(
                |(detection, pipeline_slug, account, input_file_name)| DetectionListRow {
                    detection,
                    account,
                    pipeline_slug,
                    input_file_name,
                },
            )
            .collect();

        Ok(CursorPage::new(items, total, pagination.limit, |row| {
            (row.detection.started_at.into(), row.detection.id)
        }))
    }

    async fn claim_detection(
        &mut self,
        detection_id: Uuid,
        stale_before: jiff::Timestamp,
    ) -> PgResult<Option<WorkspaceDetection>> {
        use schema::workspace_detections::{self, dsl};

        let stale_before = jiff_diesel::Timestamp::from(stale_before);

        // Claim only if still pending, or executing under a claim that has gone
        // stale (a dead worker). The WHERE clause makes the transition atomic:
        // two concurrent deliveries race on the same row and exactly one flips
        // it to `executing`; the loser matches no row and gets `None`.
        let claimed = diesel::update(
            workspace_detections::table
                .filter(dsl::id.eq(detection_id))
                .filter(
                    dsl::status.eq(DetectionStatus::Pending).or(dsl::status
                        .eq(DetectionStatus::Executing)
                        .and(dsl::claimed_at.lt(stale_before))),
                ),
        )
        .set((
            dsl::status.eq(DetectionStatus::Executing),
            dsl::claimed_at.eq(diesel::dsl::now),
        ))
        .returning(WorkspaceDetection::as_returning())
        .get_result(self)
        .await
        .optional()
        .map_err(PgError::from)?;

        Ok(claimed)
    }

    async fn detection_file_names(
        &mut self,
        workspace_id: Uuid,
        detection: &WorkspaceDetection,
    ) -> PgResult<DetectionFiles> {
        use schema::workspace_files::{self, dsl};

        // Select the display name only, scoped to the workspace and excluding
        // soft-deleted files, so a file removed by retention resolves to `None`.
        async fn name_of(
            conn: &mut PgConnection,
            workspace_id: Uuid,
            file_id: Uuid,
        ) -> PgResult<Option<String>> {
            workspace_files::table
                .filter(dsl::id.eq(file_id))
                .filter(dsl::workspace_id.eq(workspace_id))
                .filter(dsl::deleted_at.is_null())
                .select(dsl::display_name)
                .first::<String>(conn)
                .await
                .optional()
                .map_err(PgError::from)
        }

        let input = name_of(self, workspace_id, detection.input_file_id).await?;

        Ok(DetectionFiles { input })
    }

    async fn update_workspace_detection(
        &mut self,
        detection_id: Uuid,
        updates: UpdateWorkspaceDetection,
    ) -> PgResult<WorkspaceDetection> {
        use schema::workspace_detections::{self, dsl};

        let detection =
            diesel::update(workspace_detections::table.filter(dsl::id.eq(detection_id)))
                .set(&updates)
                .returning(WorkspaceDetection::as_returning())
                .get_result(self)
                .await
                .map_err(PgError::from)?;

        Ok(detection)
    }

    async fn finalize_detection(
        &mut self,
        detection_id: Uuid,
        claimed_at: jiff::Timestamp,
        mut updates: UpdateWorkspaceDetection,
    ) -> PgResult<bool> {
        use schema::workspace_detections::{self, dsl};

        // Force the terminal transition here; the guard makes it a no-op unless we
        // still own the claim.
        updates.status = Some(DetectionStatus::Complete);
        let claimed_at = jiff_diesel::Timestamp::from(claimed_at);

        // Guard on the claim we hold: same detection, still `Executing`, and the
        // exact `claimed_at` our claim stamped. A worker that re-claimed a stale
        // detection renews `claimed_at`, so a lost claim matches no row and
        // returns false.
        let updated = diesel::update(
            workspace_detections::table
                .filter(dsl::id.eq(detection_id))
                .filter(dsl::status.eq(DetectionStatus::Executing))
                .filter(dsl::claimed_at.eq(claimed_at)),
        )
        .set(&updates)
        .execute(self)
        .await
        .map_err(PgError::from)?;

        Ok(updated == 1)
    }

    async fn fail_detection(
        &mut self,
        detection_id: Uuid,
        claimed_at: jiff::Timestamp,
        mut updates: UpdateWorkspaceDetection,
    ) -> PgResult<bool> {
        use schema::workspace_detections::{self, dsl};

        updates.status = Some(DetectionStatus::Failed);
        let claimed_at = jiff_diesel::Timestamp::from(claimed_at);

        // Same claim guard as the complete finalize: only our still-live claim
        // (detection `Executing`, `claimed_at` unchanged) may fail the detection.
        let updated = diesel::update(
            workspace_detections::table
                .filter(dsl::id.eq(detection_id))
                .filter(dsl::status.eq(DetectionStatus::Executing))
                .filter(dsl::claimed_at.eq(claimed_at)),
        )
        .set(&updates)
        .execute(self)
        .await
        .map_err(PgError::from)?;

        Ok(updated == 1)
    }

    async fn record_detection_usage(
        &mut self,
        usage: &[NewWorkspaceDetectionUsage],
    ) -> PgResult<()> {
        use schema::workspace_detection_usage;

        if usage.is_empty() {
            return Ok(());
        }

        diesel::insert_into(workspace_detection_usage::table)
            .values(usage)
            .execute(self)
            .await
            .map_err(PgError::from)?;

        Ok(())
    }
}
