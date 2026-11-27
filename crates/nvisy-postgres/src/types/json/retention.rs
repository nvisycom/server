//! Data-retention value types: the typed rules for how long each class of stored
//! data is kept.
//!
//! These are pure serializable views of stored JSON — a workspace's
//! `settings.retention` baseline ([`RetentionSettings`], one [`Retention`] per
//! [`RetentionScope`]) and a detection's per-scope override ([`RetentionOverride`],
//! snapshotted on the detection it ran under). The worker that enforces them lives
//! in the server crate.

use jiff::{Span, Timestamp};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

/// How long [`Retention::Ephemeral`] data is kept: long enough to detect, review,
/// redact, and download a document, after which the reaper deletes it on its next
/// hourly tick.
pub const EPHEMERAL_WINDOW_HOURS: i64 = 24;

/// The longest [`Retention::Persistent`] window, in days. Retention is time-limited:
/// nothing is kept indefinitely.
pub const MAX_RETENTION_DAYS: u32 = 365;

/// A detection's optional per-scope override of the workspace retention. A `None`
/// field inherits the workspace value for that scope.
///
/// Only scopes that are produced are overridable — original documents are ingested,
/// not produced, so they have no override and always follow the workspace baseline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", default)]
pub struct RetentionOverride {
    /// Overrides redacted-document retention when set.
    pub redacted_documents: Option<Retention>,
    /// Overrides audit-blob retention when set.
    pub audit_logs: Option<Retention>,
    /// Overrides enrichment-intermediate retention when set.
    pub intermediates: Option<Retention>,
}

impl RetentionOverride {
    /// The override for `scope`, if any. Original documents have no override.
    #[must_use]
    pub fn get(&self, scope: RetentionScope) -> Option<Retention> {
        match scope {
            RetentionScope::OriginalDocuments => None,
            RetentionScope::RedactedDocuments => self.redacted_documents,
            RetentionScope::AuditLogs => self.audit_logs,
            RetentionScope::Intermediates => self.intermediates,
        }
    }
}

/// How long a class of data is retained. Every mode is time-limited.
///
/// Wire shape is internally tagged on `mode`: `{ "mode": "ephemeral" }`,
/// `{ "mode": "persistent", "days": 30 }`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum Retention {
    /// Keep data only for the processing window ([`EPHEMERAL_WINDOW_HOURS`]),
    /// then delete it (the default).
    #[default]
    Ephemeral,
    /// Keep data for a fixed number of days, then delete it.
    Persistent {
        /// Number of days to retain data, from 1 to [`MAX_RETENTION_DAYS`].
        #[serde(deserialize_with = "deserialize_days")]
        #[cfg_attr(feature = "schema", schemars(range(min = 1, max = MAX_RETENTION_DAYS)))]
        days: u32,
    },
}

/// Reads a [`Retention::Persistent`] window, refusing one outside
/// `1..=MAX_RETENTION_DAYS`.
fn deserialize_days<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    let days = u32::deserialize(deserializer)?;
    if (1..=MAX_RETENTION_DAYS).contains(&days) {
        Ok(days)
    } else {
        Err(D::Error::custom(format!(
            "retention days must be between 1 and {MAX_RETENTION_DAYS}, got {days}"
        )))
    }
}

impl Retention {
    /// When data written at `now` expires under this policy. Stored as a file's
    /// `expires_at`; the retention sweep deletes rows whose `expires_at` is in the
    /// past.
    ///
    /// [`Retention::Ephemeral`] expires once the processing window has passed.
    #[must_use]
    pub fn expires_at(self, now: Timestamp) -> Timestamp {
        match self {
            Self::Ephemeral => now + Span::new().hours(EPHEMERAL_WINDOW_HOURS),
            // `Timestamp` arithmetic only accepts uniform units (hours or
            // smaller), not calendar days, so express the window in hours.
            Self::Persistent { days } => now + Span::new().hours(i64::from(days) * 24),
        }
    }
}

/// The class of stored data a retention rule applies to. Each maps 1:1 to a
/// [`DocumentKind`].
///
/// [`DocumentKind`]: super::super::DocumentKind
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetentionScope {
    /// Uploaded or imported source documents.
    OriginalDocuments,
    /// Generated redacted documents.
    RedactedDocuments,
    /// Engine analysis (audit) blobs.
    AuditLogs,
    /// Enrichment content extracted from a document (OCR layout, transcript).
    Intermediates,
}

/// Retention for every scope. Missing fields default to [`Retention::Ephemeral`],
/// so an empty settings blob deletes each class of data once the processing
/// window has passed.
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
    /// Retention for enrichment intermediates (OCR layout, transcript).
    pub intermediates: Retention,
}

impl RetentionSettings {
    /// The retention configured for `scope`.
    #[must_use]
    pub fn get(&self, scope: RetentionScope) -> Retention {
        match scope {
            RetentionScope::OriginalDocuments => self.original_documents,
            RetentionScope::RedactedDocuments => self.redacted_documents,
            RetentionScope::AuditLogs => self.audit_logs,
            RetentionScope::Intermediates => self.intermediates,
        }
    }

    /// The effective retention for `scope`: a detection's retention override wins
    /// over this workspace baseline; data with no override passes `None`.
    #[must_use]
    pub fn resolve(
        &self,
        scope: RetentionScope,
        override_: Option<&RetentionOverride>,
    ) -> Retention {
        override_
            .and_then(|over| over.get(scope))
            .unwrap_or_else(|| self.get(scope))
    }
}

#[cfg(test)]
mod tests {
    use jiff::{Span, Timestamp};

    use super::*;

    #[test]
    fn expires_at_is_now_for_ephemeral_and_future_for_days() {
        let now = Timestamp::UNIX_EPOCH + Span::new().hours(100 * 24);
        assert_eq!(
            Retention::Ephemeral.expires_at(now),
            now + Span::new().hours(EPHEMERAL_WINDOW_HOURS),
        );
        assert_eq!(
            Retention::Persistent { days: 10 }.expires_at(now),
            now + Span::new().hours(10 * 24),
        );
    }

    #[test]
    fn persistent_days_outside_the_window_are_refused() {
        let parse = |value: serde_json::Value| serde_json::from_value::<Retention>(value);
        assert_eq!(
            parse(serde_json::json!({ "mode": "persistent", "days": 365 })).unwrap(),
            Retention::Persistent { days: 365 },
        );
        for days in [0, 366] {
            assert!(parse(serde_json::json!({ "mode": "persistent", "days": days })).is_err());
        }
        // A window is required: nothing is kept indefinitely.
        assert!(parse(serde_json::json!({ "mode": "persistent" })).is_err());
    }

    #[test]
    fn resolve_prefers_override() {
        let workspace = RetentionSettings {
            redacted_documents: Retention::Persistent { days: 30 },
            ..Default::default()
        };
        let over = RetentionOverride {
            redacted_documents: Some(Retention::Ephemeral),
            ..Default::default()
        };
        // Override wins when set.
        assert_eq!(
            workspace.resolve(RetentionScope::RedactedDocuments, Some(&over)),
            Retention::Ephemeral,
        );
        // Workspace baseline applies when there is no override.
        assert_eq!(
            workspace.resolve(RetentionScope::RedactedDocuments, None),
            Retention::Persistent { days: 30 },
        );
        // A scope the override leaves unset inherits the workspace value (here the
        // default, which is Ephemeral).
        assert_eq!(
            workspace.resolve(RetentionScope::AuditLogs, Some(&over)),
            Retention::Ephemeral,
        );
    }

    #[test]
    fn original_documents_ignore_override() {
        let workspace = RetentionSettings {
            original_documents: Retention::Persistent { days: 30 },
            ..Default::default()
        };
        // Original documents are ingested, not produced by a detection, so an
        // override never applies — the workspace baseline always wins.
        assert_eq!(
            workspace.resolve(
                RetentionScope::OriginalDocuments,
                Some(&RetentionOverride::default())
            ),
            Retention::Persistent { days: 30 },
        );
    }
}
