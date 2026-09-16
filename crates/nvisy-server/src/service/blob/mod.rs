//! Pipeline-run blob I/O.
//!
//! [`ArtifactWriter`] stages a run's outputs and [`ArtifactReader`] loads them
//! back, both in the platform's first-party S3-compatible blob store (the
//! `Documents`, `Audits`, and `Intermediates` [`Bucket`]s), handling
//! per-workspace encryption and the blob bookkeeping each one needs. They are
//! distinct from [`ExternalObjectStore`], which bridges external tenant object
//! stores. Staging and loading are disjoint concerns over the same two clients,
//! so they are two services rather than one: most callers touch only one side.
//!
//! Bytes are content-addressed: each `stage_*` method writes the (encrypted)
//! object and returns a [`NewBlob`] describing it, which the caller resolves
//! through [`find_or_create_blob`] (directly or via
//! [`create_workspace_document`]) so identical content is stored once and shared.
//!
//! The crypto-free GC counterpart lives with its only caller, the reaper
//! (`worker::reaper`): reclaiming an object needs no workspace key.
//!
//! [`Bucket`]: nvisy_s3::Bucket
//! [`ExternalObjectStore`]: crate::service::ExternalObjectStore
//! [`NewBlob`]: nvisy_postgres::model::NewBlob
//! [`find_or_create_blob`]: nvisy_postgres::query::WorkspaceBlobRepository::find_or_create_blob
//! [`create_workspace_document`]: nvisy_postgres::query::WorkspaceDocumentRepository::create_workspace_document

mod reader;
mod writer;

pub use crate::service::blob::reader::ArtifactReader;
pub use crate::service::blob::writer::ArtifactWriter;
