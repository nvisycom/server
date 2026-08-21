//! Workspace activity request types.

use schemars::JsonSchema;
use serde::Deserialize;

use crate::handler::Result;
use crate::handler::request::{DateWindow, ExportFormat, ResolvedWindow};

/// Most rows a single export returns. A hard ceiling so one request cannot
/// materialize an unbounded result; a truncated export says so in its response.
pub const MAX_EXPORT_ROWS: usize = 100_000;

/// Query parameters for the activity export: a `from`/`to` day range (inclusive)
/// and the output `format`. See [`DateWindow`] for the range defaults and bounds.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityExportQuery {
    /// The `from`/`to` day range.
    #[serde(flatten)]
    pub window: DateWindow,
    /// Output format; defaults to `csv`.
    #[serde(default)]
    pub format: ExportFormat,
}

/// A validated export request: a bounded day range and the chosen format.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedExport {
    /// The validated day range.
    pub window: ResolvedWindow,
    /// Output format.
    pub format: ExportFormat,
}

impl ActivityExportQuery {
    /// Resolves the request: validates the window and carries the format through.
    pub fn resolve(&self) -> Result<ResolvedExport> {
        Ok(ResolvedExport {
            window: self.window.resolve()?,
            format: self.format,
        })
    }
}
