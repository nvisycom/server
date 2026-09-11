//! The retention-backfill pipeline: reprojecting stored files' `expires_at` when
//! a workspace's retention settings or a pipeline's override changes.
//!
//! The settings/override handler commits a scope-only job (transactionally with
//! the change) and wakes the [`RetentionBackfillCoordinator`]. The
//! [`RetentionBackfillDrainer`] claims pending jobs and reprojects each affected
//! file's `expires_at` from the file's own `created_at` under the current policy,
//! in bounded keyset-paged batches — keeping the unbounded reprojection off the
//! request path.

mod coordinator;
mod drainer;

pub use coordinator::RetentionBackfillCoordinator;
pub use drainer::RetentionBackfillDrainer;
