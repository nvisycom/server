//! Workspace activity request types.

use nvisy_postgres::query::ActivityFilter;
use nvisy_postgres::types::ActivityType;
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::handler::Result;
use crate::handler::request::{CursorPagination, DateWindow, ExportFormat, ResolvedWindow};

/// Most rows a single export returns. A hard ceiling so one request cannot
/// materialize an unbounded result; a truncated export says so in its response.
pub const MAX_EXPORT_ROWS: usize = 100_000;

/// The non-window activity filters — type and actor — shared by the feed and the
/// export so both narrow the log the same way. Time is a separate facet: the feed
/// takes it as an optional filter, the export as its bounded window.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityFilterQuery {
    /// Keep only these activity types (e.g. `file.created`). Repeat the parameter
    /// for several; omit for no type constraint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub types: Option<Vec<ActivityType>>,
    /// Keep only activities performed by this account. Omit for any actor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<Uuid>,
}

impl ActivityFilterQuery {
    /// Builds the repository filter, attaching the resolved half-open time bounds
    /// (either may be `None`).
    fn to_filter(
        &self,
        from: Option<jiff::Timestamp>,
        to: Option<jiff::Timestamp>,
    ) -> ActivityFilter {
        ActivityFilter {
            types: self.types.clone().unwrap_or_default(),
            actor: self.actor,
            from,
            to,
        }
    }
}

/// Query parameters for the activity feed: the shared filter, an optional date
/// window (narrows only when given — the feed is otherwise all-time), and cursor
/// pagination.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityListQuery {
    /// Type/actor filter.
    #[serde(flatten)]
    pub filter: ActivityFilterQuery,
    /// Optional `from`/`to` day range; each bound narrows the feed only if given.
    #[serde(flatten)]
    pub window: DateWindow,
    /// Cursor pagination.
    #[serde(flatten)]
    pub pagination: CursorPagination,
}

impl ActivityListQuery {
    /// Resolves the repository filter, validating the optional window bounds.
    pub fn to_filter(&self) -> Result<ActivityFilter> {
        let (from, to) = self.window.resolve_optional_bounds()?;
        Ok(self.filter.to_filter(from, to))
    }
}

/// Query parameters for the activity export: the shared filter, a bounded date
/// window (defaulted and capped, since the export materializes rows), and the
/// output `format`. See [`DateWindow`] for the range defaults and bounds.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityExportQuery {
    /// Type/actor filter.
    #[serde(flatten)]
    pub filter: ActivityFilterQuery,
    /// The `from`/`to` day range.
    #[serde(flatten)]
    pub window: DateWindow,
    /// Output format; defaults to `csv`.
    #[serde(default)]
    pub format: ExportFormat,
}

/// A validated export request: the repository filter (with the resolved window
/// folded in) and the chosen format.
#[derive(Debug, Clone)]
pub struct ResolvedExport {
    /// The repository filter, including the export's resolved time window.
    pub filter: ActivityFilter,
    /// The validated day range (for naming the file).
    pub window: ResolvedWindow,
    /// Output format.
    pub format: ExportFormat,
}

impl ActivityExportQuery {
    /// Resolves the request: validates and defaults the window, folds it plus the
    /// type/actor filter into one repository filter, and carries the format.
    pub fn resolve(&self) -> Result<ResolvedExport> {
        let window = self.window.resolve()?;
        let filter = self
            .filter
            .to_filter(Some(window.from_timestamp()?), Some(window.to_timestamp()?));
        Ok(ResolvedExport {
            filter,
            window,
            format: self.format,
        })
    }
}
