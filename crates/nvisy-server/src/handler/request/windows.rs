//! Shared date-window request type for endpoints scoped to a day range.
//!
//! A `from`/`to` day range (inclusive), defaulted and bounded uniformly so every
//! range-scoped endpoint — analytics series, activity export — validates the same
//! way and resolves to the same half-open `[from, to)` timestamp bounds.

use jiff::civil::Date;
use jiff::{ToSpan, Zoned};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::handler::{Error, ErrorKind, Result};

/// Longest window, in days. Ranges are materialized per day (a gap-filled series,
/// an exported row set), so the span is capped to bound the response and cost.
pub const MAX_WINDOW_DAYS: i64 = 366;

/// Default window when the caller gives no dates: the last 30 days through today.
pub const DEFAULT_WINDOW_DAYS: i64 = 30;

/// A `from`/`to` day range (inclusive), both optional. Flatten it into an
/// endpoint's query struct with `#[serde(flatten)]`, or use it directly when the
/// range is the only parameter.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DateWindow {
    /// First day of the range (inclusive), `YYYY-MM-DD`. Defaults to
    /// `DEFAULT_WINDOW_DAYS` before `to`.
    pub from: Option<Date>,
    /// Last day of the range (inclusive), `YYYY-MM-DD`. Defaults to today (UTC).
    pub to: Option<Date>,
}

/// A validated, bounded day range: `from <= to` and at most `MAX_WINDOW_DAYS`
/// days wide.
#[derive(Debug, Clone, Copy)]
pub struct ResolvedWindow {
    /// First day (inclusive).
    pub from: Date,
    /// Last day (inclusive).
    pub to: Date,
}

impl DateWindow {
    /// Resolves the window, filling defaults and validating bounds.
    ///
    /// `to` defaults to today (UTC); `from` defaults to `DEFAULT_WINDOW_DAYS`
    /// before `to`. Rejects `from > to` (400) and a span wider than
    /// `MAX_WINDOW_DAYS` (400).
    pub fn resolve(&self) -> Result<ResolvedWindow> {
        let to = self.to.unwrap_or_else(today_utc);
        let from = match self.from {
            Some(from) => from,
            None => to
                .checked_sub(DEFAULT_WINDOW_DAYS.days())
                .map_err(|_| invalid("Window start is out of range"))?,
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

        Ok(ResolvedWindow { from, to })
    }
}

impl ResolvedWindow {
    /// The window's start as a UTC timestamp at the day's first instant
    /// (inclusive lower bound).
    pub fn from_timestamp(&self) -> Result<jiff::Timestamp> {
        day_start_utc(self.from)
    }

    /// The window's end as an *exclusive* upper bound: the first instant of the
    /// day after `to`. A filter of `column < to_timestamp()` therefore includes
    /// the whole `to` day without spilling into the next.
    pub fn to_timestamp(&self) -> Result<jiff::Timestamp> {
        let next = self
            .to
            .checked_add(1.days())
            .map_err(|_| invalid("Window end is out of range"))?;
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

/// A 400 for an invalid window.
fn invalid(message: impl Into<std::borrow::Cow<'static, str>>) -> Error<'static> {
    ErrorKind::BadRequest.with_message(message)
}

#[cfg(test)]
mod tests {
    use jiff::civil::date;

    use super::*;

    fn window(from: Date, to: Date) -> ResolvedWindow {
        DateWindow {
            from: Some(from),
            to: Some(to),
        }
        .resolve()
        .expect("window resolves")
    }

    #[test]
    fn to_timestamp_is_an_exclusive_next_day_bound() {
        let w = window(date(2026, 1, 1), date(2026, 1, 31));
        // Lower bound is the first instant of `from`.
        assert_eq!(
            w.from_timestamp().unwrap(),
            date(2026, 1, 1)
                .to_zoned(jiff::tz::TimeZone::UTC)
                .unwrap()
                .timestamp()
        );
        // Upper bound is the first instant of the day *after* `to`, so a filter of
        // `column < to_timestamp()` includes every row on the `to` day.
        assert_eq!(
            w.to_timestamp().unwrap(),
            date(2026, 2, 1)
                .to_zoned(jiff::tz::TimeZone::UTC)
                .unwrap()
                .timestamp()
        );
    }

    #[test]
    fn single_day_window_spans_exactly_one_day() {
        let w = window(date(2026, 1, 15), date(2026, 1, 15));
        let seconds =
            w.to_timestamp().unwrap().as_second() - w.from_timestamp().unwrap().as_second();
        assert_eq!(seconds, 24 * 60 * 60);
    }

    #[test]
    fn rejects_from_after_to() {
        let err = DateWindow {
            from: Some(date(2026, 2, 1)),
            to: Some(date(2026, 1, 1)),
        }
        .resolve();
        assert!(err.is_err());
    }

    #[test]
    fn accepts_window_exactly_at_the_cap() {
        let from = date(2026, 1, 1);
        let to = from
            .checked_add(MAX_WINDOW_DAYS.days())
            .expect("date in range");
        assert!(
            DateWindow {
                from: Some(from),
                to: Some(to),
            }
            .resolve()
            .is_ok()
        );
    }

    #[test]
    fn rejects_window_wider_than_the_cap() {
        let from = date(2026, 1, 1);
        let to = from
            .checked_add((MAX_WINDOW_DAYS + 1).days())
            .expect("date in range");
        let err = DateWindow {
            from: Some(from),
            to: Some(to),
        }
        .resolve();
        assert!(err.is_err());
    }
}
