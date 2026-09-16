//! Blob reaper: reclaims stored objects once nothing references them.
//!
//! Every stored object — original documents, redacted outputs, audit blobs, and
//! intermediates — lives in a shared, ref-counted `workspace_blobs` row. A blob
//! becomes reclaimable only when its last referrer is gone (`ref_count = 0`),
//! which protects a blob shared by two identical uploads from being purged out
//! from under a still-live document. Retention is a time policy layered on top:
//! a blob is reclaimed once it is both unreferenced and past its window. Each
//! tick runs three stages:
//!
//! - **Referrer cleanup**: machine byproducts — audits and detection
//!   intermediates — hold a reference for their whole life, so their blobs never
//!   reach `ref_count = 0` on their own. This stage expires those referrers by
//!   their blob's retention window by nulling the blob pointer (keeping the audit
//!   and detection rows, which are ledger records) and drops the reference, so an
//!   expired byproduct blob becomes reclaimable by the Expire sweep.
//! - **Expire**: unreferenced blobs whose retention window has elapsed
//!   (`ref_count = 0 AND expires_at < now()`, from the per-blob retention rule).
//!   Each is claimed ([`purged_at`], committed before any object delete, so it
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
