//! Workspace activity request types.

use jiff::civil::Date;
use jiff::{ToSpan, Zoned};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::handler::request::ExportFormat;
use crate::handler::{Error, ErrorKind, Result};

/// Longest export window, in days. The export materializes every row in the
/// window in memory, so the span is capped to bound the response size and cost.
const MAX_WINDOW_DAYS: i64 = 366;

/// Default window when the caller gives no dates: the last 30 days through today.
const DEFAULT_WINDOW_DAYS: i64 = 30;

/// Most rows a single export returns. A hard ceiling so one request cannot
/// materialize an unbounded result; a truncated export says so in its response.
pub const MAX_EXPORT_ROWS: usize = 100_000;

/// Query parameters for the activity export: a `[from, to]` day range (inclusive)
/// and the output `format`. All optional — see [`ActivityExportQuery::resolve`].
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityExportQuery {
    /// First day to include (inclusive), `YYYY-MM-DD`. Defaults to 30 days
    /// before `to`.
    pub from: Option<Date>,
    /// Last day to include (inclusive), `YYYY-MM-DD`. Defaults to today (UTC).
    pub to: Option<Date>,
    /// Output format; defaults to `csv`.
    #[serde(default)]
    pub format: ExportFormat,
}

/// A validated export request: a bounded day range and the chosen format.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedExport {
    /// First day (inclusive).
    pub from: Date,
    /// Last day (inclusive).
    pub to: Date,
    /// Output format.
    pub format: ExportFormat,
}

impl ActivityExportQuery {
    /// Resolves the request, filling defaults and validating bounds.
    ///
    /// `to` defaults to today (UTC); `from` defaults to `DEFAULT_WINDOW_DAYS`
    /// before `to`. Rejects `from > to` (400) and a span wider than
    /// `MAX_WINDOW_DAYS` (400).
    pub fn resolve(self) -> Result<ResolvedExport> {
        let to = self.to.unwrap_or_else(today_utc);
        let from = match self.from {
            Some(from) => from,
            None => to
                .checked_sub(DEFAULT_WINDOW_DAYS.days())
                .map_err(|_| invalid("Export start is out of range"))?,
        };

        if from > to {
            return Err(invalid("`from` must not be after `to`"));
        }

        let span = to.since(from).map_err(|_| invalid("Invalid window"))?;
        if span.get_days() as i64 > MAX_WINDOW_DAYS {
            return Err(invalid(format!(
                "Window is too wide (max {MAX_WINDOW_DAYS} days)"
            )));
        }

        Ok(ResolvedExport {
            from,
            to,
            format: self.format,
        })
    }
}

impl ResolvedExport {
    /// The window's start as a UTC timestamp at the day's first instant
    /// (inclusive lower bound).
    pub fn from_timestamp(&self) -> Result<jiff::Timestamp> {
        day_start_utc(self.from)
    }

    /// The window's end as an *exclusive* upper bound: the first instant of the
    /// day after `to`, so the whole `to` day is included.
    pub fn to_timestamp(&self) -> Result<jiff::Timestamp> {
        let next = self
            .to
            .checked_add(1.days())
            .map_err(|_| invalid("Export end is out of range"))?;
        day_start_utc(next)
    }
}

/// Today's date in UTC.
fn today_utc() -> Date {
    Zoned::now().with_time_zone(jiff::tz::TimeZone::UTC).date()
}

/// The first instant of `date` in UTC. Fails (400) rather than silently widening
/// the query if the date cannot be represented as a zoned timestamp.
fn day_start_utc(date: Date) -> Result<jiff::Timestamp> {
    date.to_zoned(jiff::tz::TimeZone::UTC)
        .map(|z| z.timestamp())
        .map_err(|_| invalid("Window bound is out of range"))
}

/// A 400 for an invalid export request.
fn invalid(message: impl Into<std::borrow::Cow<'static, str>>) -> Error<'static> {
    ErrorKind::BadRequest.with_message(message)
}
