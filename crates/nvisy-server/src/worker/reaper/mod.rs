//! Blob reaper: reclaims stored objects once nothing references them.
//!
//! Every stored object — original documents, redacted outputs, audit blobs, and
//! intermediates — lives in a shared, ref-counted `workspace_blobs` row. A blob
//! becomes reclaimable only when its last referrer is gone (`ref_count = 0`),
//! which protects a blob shared by two identical uploads from being purged out
//! from under a still-live document. Retention is enforced on the referrers: each
//! releases its reference when its own window passes, or when it is deleted, and
//! the blob is reclaimed once none is left. Each tick runs three stages:
//!
//! - **Referrer cleanup**: documents, audits, and detection intermediates hold a
//!   reference for their whole life. This stage releases each one past its
//!   retention window (a document's own `expires_at`, an audit's or
//!   intermediate's blob expiry) by nulling the blob pointer (keeping the
//!   document, detection, and redaction rows, which are ledger records) and
//!   dropping the reference. A document keeps its bytes while a detection on it
//!   is still pending or executing.
//! - **Purge**: unreferenced blobs (`ref_count = 0`), whether released here or by
//!   a deletion. Each is claimed ([`purged_at`], committed before any object delete, so it
//!   leaves the dedup set atomically) and then its object is reclaimed.
//! - **Reconcile**: blobs claimed for purge whose object delete was never
//!   confirmed (`purged_at IS NOT NULL AND reclaimed_at IS NULL`) — a delete that
//!   failed or a crash between claim and delete. The object delete is retried
//!   (idempotent) until `reclaimed_at` is stamped, so a transient object-store
//!   outage self-heals and a claimed blob's bytes are never reused meanwhile.
//!
//! The crypto-free object-delete collaborator the sweeps run through (the
//! reclaimer) lives here too, with its only caller.
//!
//! [`purged_at`]: nvisy_postgres::model::Blob::purged_at

mod reclaimer;
mod worker;

pub use crate::worker::reaper::worker::BlobReaper;
