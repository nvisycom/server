//! Write side of the pipeline-run blob store: staging a run's outputs.

use std::io::Cursor;

use bytes::Bytes;
use elide_pipeline::{ArtifactSet, Audit};
use nvisy_postgres::model::{NewBlob, WorkspaceDocument};
use nvisy_postgres::types::{Retention, RetentionOverride, RetentionScope, RetentionSettings};
use nvisy_s3::{AuditKey, DocumentKey, IntermediateKey, ObjectKey};
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::delete_object;
use crate::response::{Error, ErrorKind, Result};
use crate::service::{CryptoService, Infra};

/// Tracing target for blob staging.
const TRACING_TARGET: &str = "nvisy_server::service::blob::writer";

/// A staged object: its blob, not yet inserted, and when the referrer that will
/// point at it releases the bytes.
///
/// The expiry belongs to the referrer (a document, detection, or redaction row),
/// not the blob, since identical bytes share one blob across referrers with
/// different retention.
#[derive(Debug, Clone)]
pub struct StagedBlob {
    /// The blob to resolve (share or insert) in the committing transaction.
    pub blob: NewBlob,
    /// When the referrer's retention window for these bytes ends.
    pub expires_at: jiff::Timestamp,
}

/// Stages a pipeline run's outputs into the first-party blob store: encrypts and
/// writes documents, audits, and enrichment intermediates, returning the
/// [`NewBlob`] the caller resolves into a row. Its read counterpart is
/// [`ArtifactReader`].
///
/// Cloneable and cheap to pass around: it holds the shared [`Infra`] clients
/// (all `Arc`-backed) and takes the per-request database connection as a method
/// argument. Not to be confused with [`ExternalObjectStore`], which bridges
/// *external* tenant object stores; this operates on the platform's own
/// S3-compatible store, routing to the `Documents`, `Audits`, and
/// `Intermediates` [`Bucket`] prefixes.
///
/// [`ArtifactReader`]: crate::service::ArtifactReader
/// [`ExternalObjectStore`]: crate::service::ExternalObjectStore
/// [`NewBlob`]: nvisy_postgres::model::NewBlob
/// [`Bucket`]: nvisy_s3::Bucket
#[derive(Clone)]
#[must_use = "service does nothing unless you use it"]
pub struct ArtifactWriter {
    pub(super) infra: Infra,
    pub(super) crypto: CryptoService,
}

impl ArtifactWriter {
    /// Creates a new [`ArtifactWriter`] over the internal object store and crypto.
    pub fn new(infra: Infra, crypto: CryptoService) -> Self {
        Self { infra, crypto }
    }

    /// Encrypts an audit — a detection's analysis or a redaction's review audit —
    /// writes it to the audit bucket, and builds the [`StagedBlob`] that will
    /// hold it, under the audit-logs retention scope.
    ///
    /// The audit is the map of detected PII, so it is encrypted with the
    /// workspace key before it leaves the process. A detection points at its
    /// analysis (`audit_blob_id`) and a redaction at its review audit
    /// (`review_audit_blob_id`); both share this scope.
    ///
    /// # Errors
    ///
    /// - `InternalServerError` if serializing or encrypting the audit fails.
    /// - A storage error if the object write fails.
    pub async fn stage_audit(
        &self,
        workspace_id: Uuid,
        retention_override: Option<&RetentionOverride>,
        workspace_settings: &RetentionSettings,
        audit: &Audit,
    ) -> Result<StagedBlob> {
        let plaintext = to_json(audit)?;
        let retention = workspace_settings.resolve(RetentionScope::AuditLogs, retention_override);
        self.stage_encrypted(
            AuditKey::generate(workspace_id),
            workspace_id,
            &plaintext,
            retention,
            Some(audit.digest().as_bytes().to_vec()),
        )
        .await
    }

    /// Encrypts a detection's enrichment intermediates (OCR layout, transcript),
    /// writes them to the intermediates store, and builds the [`StagedBlob`] that
    /// will hold them, under their own retention scope.
    ///
    /// # Errors
    ///
    /// - `InternalServerError` if serializing or encrypting `artifacts` fails.
    /// - A storage error if writing the object fails.
    pub async fn stage_intermediates(
        &self,
        workspace_id: Uuid,
        retention_override: Option<&RetentionOverride>,
        workspace_settings: &RetentionSettings,
        artifacts: &ArtifactSet,
    ) -> Result<StagedBlob> {
        let plaintext = to_json(artifacts)?;
        let retention =
            workspace_settings.resolve(RetentionScope::Intermediates, retention_override);
        self.stage_encrypted(
            IntermediateKey::generate(workspace_id),
            workspace_id,
            &plaintext,
            retention,
            Some(artifacts.digest().as_bytes().to_vec()),
        )
        .await
    }

    /// Encrypts redacted bytes, writes them to the documents bucket, and builds the
    /// [`StagedBlob`] that will hold them together with the redacted output's display
    /// name. The redacted document is a first-class document (a sibling of the
    /// source), downloadable through the normal document endpoints.
    ///
    /// # Errors
    ///
    /// - `InternalServerError` if encrypting the redacted bytes fails.
    /// - A storage error if writing the object fails.
    pub async fn stage_redacted_document(
        &self,
        source_document: &WorkspaceDocument,
        retention_override: Option<&RetentionOverride>,
        workspace_settings: &RetentionSettings,
        bytes: Bytes,
    ) -> Result<(StagedBlob, String)> {
        let workspace_id = source_document.workspace_id;
        let retention =
            workspace_settings.resolve(RetentionScope::RedactedDocuments, retention_override);
        let staged = self
            .stage_encrypted(
                DocumentKey::generate(workspace_id),
                workspace_id,
                &bytes,
                retention,
                None,
            )
            .await?;
        let redacted_name = redacted_display_name(
            &source_document.display_name,
            &source_document.file_extension,
        );
        Ok((staged, redacted_name))
    }

    /// Deletes staged objects whose blobs were never resolved, best effort.
    ///
    /// The `stage_*` methods write an object before its `workspace_blobs` row; if
    /// the committing transaction rolls back (or never runs), the object has no
    /// blob and the blob-driven reaper can never find it. Callers pass every
    /// object they staged on that path so the orphans are removed immediately
    /// instead of accumulating. A failed delete only leaves that object for a
    /// later manual sweep, so it is logged and the rest still go.
    pub async fn discard_staged(&self, staged: &[&StagedBlob]) {
        for staged in staged {
            let blob = &staged.blob;
            if let Err(err) =
                delete_object(&self.infra.blobs, &blob.storage_bucket, &blob.storage_path).await
            {
                tracing::warn!(
                    target: TRACING_TARGET,
                    error = %err,
                    storage_path = %blob.storage_path,
                    "Failed to reclaim orphaned staged object; left for a later sweep",
                );
            }
        }
    }

    /// Encrypts `plaintext` with the workspace key, writes it at `key`, and builds
    /// the [`StagedBlob`] describing it (hashed and sized over the plaintext, so
    /// identical content shares a blob).
    ///
    /// The object write is not transactional, so it stays out of the caller's
    /// database transaction: the bytes are written first, then the returned blob
    /// is resolved with the run's other writes. A rollback therefore leaves at
    /// worst an orphan object, never a blob pointing at bytes that were never
    /// written; the caller reclaims that orphan via
    /// [`discard_staged`](Self::discard_staged).
    async fn stage_encrypted<K: ObjectKey>(
        &self,
        key: K,
        workspace_id: Uuid,
        plaintext: &[u8],
        retention: Retention,
        audit_digest: Option<Vec<u8>>,
    ) -> Result<StagedBlob> {
        let content_hash = Sha256::digest(plaintext).to_vec();
        let file_size_bytes = i64::try_from(plaintext.len()).unwrap_or(i64::MAX);
        let ciphertext = self
            .crypto
            .encrypt(workspace_id, plaintext)
            .map_err(|err| {
                ErrorKind::InternalServerError
                    .with_message("Failed to encrypt staged content")
                    .with_context(err.to_string())
            })?;
        self.infra.blobs.put(&key, Cursor::new(ciphertext)).await?;
        // The retention window starts once the bytes exist, so upload time does
        // not count against it.
        let expires_at = retention.expires_at(jiff::Timestamp::now());

        Ok(StagedBlob {
            blob: NewBlob {
                workspace_id,
                content_hash,
                file_size_bytes,
                storage_path: key.to_string(),
                storage_bucket: K::BUCKET.name().to_owned(),
                audit_digest,
            },
            expires_at,
        })
    }
}

/// Serializes an audit or artifact set for staging.
fn to_json(value: &impl Serialize) -> Result<Vec<u8>> {
    serde_json::to_vec(value).map_err(|err| -> Error {
        ErrorKind::InternalServerError
            .with_message("Failed to serialize analysis")
            .with_context(err.to_string())
    })
}

/// Builds the redacted document's display name by inserting a `redacted` marker
/// before the extension: `report.pdf` becomes `report.redacted.pdf`.
///
/// The stem is taken by stripping a trailing `.{extension}` (case-insensitive)
/// from the display name; a name that does not end in its own extension (or has
/// none) simply gains a `.redacted` suffix.
fn redacted_display_name(display_name: &str, extension: &str) -> String {
    // Split off a trailing `.{extension}`, matched case-insensitively on both
    // sides so an upper- or mixed-case extension (`Report.PDF`) still has the
    // marker inserted before it, not appended after.
    let suffix = format!(".{extension}");
    let stem = display_name
        .len()
        .checked_sub(suffix.len())
        .filter(|_| !extension.is_empty())
        .filter(|&at| display_name.is_char_boundary(at))
        .map(|at| display_name.split_at(at))
        .filter(|(_, tail)| tail.eq_ignore_ascii_case(&suffix))
        .map(|(stem, _)| stem);
    match stem {
        Some(stem) => format!("{stem}.redacted.{extension}"),
        None => format!("{display_name}.redacted"),
    }
}

#[cfg(test)]
mod tests {
    use super::redacted_display_name;

    #[test]
    fn inserts_marker_before_the_extension() {
        assert_eq!(
            redacted_display_name("report.pdf", "pdf"),
            "report.redacted.pdf"
        );
    }

    #[test]
    fn matches_the_extension_case_insensitively() {
        // The name's extension case differs from the passed extension...
        assert_eq!(
            redacted_display_name("Report.PDF", "pdf"),
            "Report.redacted.pdf"
        );
        // ...and the passed extension itself may be upper- or mixed-case; the
        // marker still lands before it, and the original extension text is kept.
        assert_eq!(
            redacted_display_name("Report.PDF", "PDF"),
            "Report.redacted.PDF"
        );
        assert_eq!(
            redacted_display_name("report.pdf", "PDF"),
            "report.redacted.PDF"
        );
    }

    #[test]
    fn preserves_a_multi_dot_stem() {
        assert_eq!(
            redacted_display_name("2026.q1.report.pdf", "pdf"),
            "2026.q1.report.redacted.pdf"
        );
    }

    #[test]
    fn appends_when_the_name_lacks_its_extension() {
        // A display name that does not end in `.{extension}` just gains the
        // marker, so no extension is fabricated.
        assert_eq!(redacted_display_name("report", "pdf"), "report.redacted");
        assert_eq!(
            redacted_display_name("report.txt", "pdf"),
            "report.txt.redacted"
        );
    }
}
