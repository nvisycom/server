//! Data-retention value types: the typed rules for how long each class of stored
//! data is kept.
//!
//! These are pure serializable views of stored JSON — a workspace's
//! `settings.retention` baseline ([`RetentionSettings`], one [`Retention`] per
//! [`RetentionScope`]) and a pipeline's per-scope override ([`RetentionOverride`],
//! carried by `PipelineMetadata::retention`). The worker that enforces them lives
//! in the server crate.

use jiff::{Span, Timestamp};
use serde::{Deserialize, Serialize};

use super::RetentionOverride;

/// How long a class of data is retained.
///
/// Wire shape is internally tagged on `mode`: `{ "mode": "forever" }`,
/// `{ "mode": "zeroDays" }`, `{ "mode": "days", "days": 30 }`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum Retention {
    /// Keep data indefinitely (the default).
    #[default]
    Forever,
    /// Delete data as soon as it has been processed.
    ZeroDays,
    /// Keep data for a fixed number of days, then delete it.
    Days {
        /// Number of days to retain data.
        days: u32,
    },
}

impl Retention {
    /// When data written at `now` expires under this policy, or `None` when it
    /// never expires ([`Retention::Forever`]). Stored as a file's `expires_at`;
    /// the retention sweep deletes rows whose `expires_at` is in the past.
    ///
    /// [`Retention::ZeroDays`] expires immediately (`now`), so the data is
    /// eligible for deletion as soon as it has been written.
    #[must_use]
    pub fn expires_at(self, now: Timestamp) -> Option<Timestamp> {
        match self {
            Self::Forever => None,
            Self::ZeroDays => Some(now),
            // `Timestamp` arithmetic only accepts uniform units (hours or
            // smaller), not calendar days, so express the window in hours.
            Self::Days { days } => Some(now + Span::new().hours(i64::from(days) * 24)),
        }
    }
}

/// The class of stored data a retention rule applies to. Each maps 1:1 to a
/// [`FileKind`](super::super::FileKind).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetentionScope {
    /// Uploaded or imported source documents.
    OriginalDocuments,
    /// Generated redacted documents.
    RedactedDocuments,
    /// Engine analysis (audit) blobs.
    AuditLogs,
}

/// Retention for every scope. Missing fields default to [`Retention::Forever`],
/// so an empty settings blob keeps everything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RetentionSettings {
    /// Retention for uploaded/imported source documents.
    pub original_documents: Retention,
    /// Retention for generated redacted documents.
    pub redacted_documents: Retention,
    /// Retention for audit blobs.
    pub audit_logs: Retention,
}

impl RetentionSettings {
    /// The retention configured for `scope`.
    #[must_use]
    pub fn get(&self, scope: RetentionScope) -> Retention {
        match scope {
            RetentionScope::OriginalDocuments => self.original_documents,
            RetentionScope::RedactedDocuments => self.redacted_documents,
            RetentionScope::AuditLogs => self.audit_logs,
        }
    }

    /// Whether every scope is [`Retention::Forever`] (nothing to enforce).
    #[must_use]
    pub fn is_noop(&self) -> bool {
        *self == Self::default()
    }

    /// The effective retention for `scope`: a pipeline override wins over this
    /// workspace baseline. Data with no owning pipeline passes `None`.
    #[must_use]
    pub fn resolve(
        &self,
        scope: RetentionScope,
        pipeline: Option<&RetentionOverride>,
    ) -> Retention {
        pipeline
            .and_then(|over| over.get(scope))
            .unwrap_or_else(|| self.get(scope))
    }
}

#[cfg(test)]
mod tests {
    use jiff::{Span, Timestamp};

    use super::*;

    #[test]
    fn expires_at_is_none_for_forever_and_future_for_days() {
        let now = Timestamp::UNIX_EPOCH + Span::new().hours(100 * 24);
        // Forever never expires; ZeroDays expires immediately; Days expires in
        // the future (now + window), never in the past.
        assert_eq!(Retention::Forever.expires_at(now), None);
        assert_eq!(Retention::ZeroDays.expires_at(now), Some(now));
        assert_eq!(
            Retention::Days { days: 10 }.expires_at(now),
            Some(now + Span::new().hours(10 * 24)),
        );
    }

    #[test]
    fn resolve_prefers_pipeline_override() {
        let workspace = RetentionSettings {
            redacted_documents: Retention::Days { days: 30 },
            ..Default::default()
        };
        let over = RetentionOverride {
            redacted_documents: Some(Retention::ZeroDays),
            ..Default::default()
        };
        // Override wins when set.
        assert_eq!(
            workspace.resolve(RetentionScope::RedactedDocuments, Some(&over)),
            Retention::ZeroDays,
        );
        // Workspace baseline applies when there is no override.
        assert_eq!(
            workspace.resolve(RetentionScope::RedactedDocuments, None),
            Retention::Days { days: 30 },
        );
        // A scope the override leaves unset inherits the workspace value.
        assert_eq!(
            workspace.resolve(RetentionScope::AuditLogs, Some(&over)),
            Retention::Forever,
        );
    }

    #[test]
    fn original_documents_ignore_pipeline_override() {
        let workspace = RetentionSettings {
            original_documents: Retention::Days { days: 30 },
            ..Default::default()
        };
        // Original documents are ingested, not produced by a pipeline, so an
        // override never applies — the workspace baseline always wins.
        assert_eq!(
            workspace.resolve(
                RetentionScope::OriginalDocuments,
                Some(&RetentionOverride::default())
            ),
            Retention::Days { days: 30 },
        );
    }
}
