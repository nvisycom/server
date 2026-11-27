//! Workspace purge worker: tears down workspaces past their soft-delete grace.
//!
//! Deleting a workspace is a soft delete — its rows are hidden but kept, so the
//! deletion is recoverable during a grace window. After the window elapses this
//! worker tears the workspace down for good, in two idempotent, crash-safe steps
//! that deal only in rows and blob *references* (never blob bytes or objects):
//!
//! 1. **Release references**: soft-delete the workspace's blob-holding entities
//!    (documents, detections, redactions) and drop the blob reference each holds.
//!    The [blob reaper] then reclaims the freed bytes like any other unreferenced
//!    blob; object deletion stays entirely with the reaper.
//! 2. **Finalize**: once the workspace's blobs are all gone (the reaper has
//!    reclaimed them), hard-delete the workspace row; the FK cascade removes every
//!    remaining child row.
//!
//! Depending on the reaper only through eventual database state (the blobs
//! disappear) keeps object deletion on one path and lets this worker run on its
//! own grace-scale cadence rather than the reaper's hourly one.
//!
//! [blob reaper]: crate::worker::reaper

mod config;
mod worker;

pub use crate::worker::purge::config::{DEFAULT_PURGE_GRACE, PurgeConfig};
pub use crate::worker::purge::worker::WorkspacePurgeWorker;
