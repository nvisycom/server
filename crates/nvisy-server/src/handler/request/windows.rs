//! Shared date-window request type for endpoints scoped to a day range.
//!
//! A `from`/`to` day range (inclusive), defaulted and bounded uniformly so every
//! range-scoped endpoint — analytics series, activity export — validates the same
//! way and resolves to the same half-open `[from, to)` timestamp bounds.

use std::borrow::Cow;

use jiff::civil::Date;
use jiff::tz::TimeZone;
use jiff::{ToSpan, Zoned};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::handler::{Error, ErrorKind, Result};

/// Longest window, as a count of inclusive calendar days. Ranges are materialized
/// per day (a gap-filled series, an exported row set), so the span is capped to
/// bound the response and cost.
pub const MAX_WINDOW_DAYS: i64 = 366;

/// Default window, as a count of inclusive calendar days: the last 30 days
/// through today.
pub const DEFAULT_WINDOW_DAYS: i64 = 30;

/// A `from`/`to` day range (inclusive), both optional. Flatten it into an
/// endpoint's query struct with `#[serde(flatten)]`, or use it directly when the
/// range is the only parameter.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DateWindow {
    /// First day of the range (inclusive), `YYYY-MM-DD`. Defaults so the range
    /// spans the last `DEFAULT_WINDOW_DAYS` days through `to`.
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
    /// `to` defaults to today (UTC); `from` defaults so the default window spans
    /// exactly `DEFAULT_WINDOW_DAYS` inclusive dates. Rejects `from > to` (400)
    /// and a window of more than `MAX_WINDOW_DAYS` inclusive dates (400). Both
    /// bounds are inclusive, so the counts are date counts, not the difference:
    /// `[from, to]` spans `to - from + 1` dates.
    pub fn resolve(&self) -> Result<ResolvedWindow> {
        let to = self.to.unwrap_or_else(today_utc);
        let from = match self.from {
            Some(from) => from,
            // -1: with both bounds inclusive, N dates ending at `to` start at
            // `to - (N - 1)`.
            None => to
                .checked_sub((DEFAULT_WINDOW_DAYS - 1).days())
                .map_err(|_| invalid("Window start is out of range"))?,
        };

        if from > to {
            return Err(invalid("`from` must not be after `to`"));
        }

        // Inclusive date count is the day difference plus one, so a difference of
        // `MAX_WINDOW_DAYS` would be one date too many.
        let span = to.since(from).map_err(|_| invalid("Invalid window"))?;
        if span.get_days() as i64 >= MAX_WINDOW_DAYS {
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
    Zoned::now().with_time_zone(TimeZone::UTC).date()
}

/// The first instant of `date` in UTC. Fails (400) rather than silently widening
/// the query if the date cannot be represented as a zoned timestamp.
fn day_start_utc(date: Date) -> Result<jiff::Timestamp> {
    date.to_zoned(TimeZone::UTC)
        .map(|z| z.timestamp())
        .map_err(|_| invalid("Window bound is out of range"))
}

/// A 400 for an invalid window.
fn invalid(message: impl Into<Cow<'static, str>>) -> Error<'static> {
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
                .to_zoned(TimeZone::UTC)
                .unwrap()
                .timestamp()
        );
        // Upper bound is the first instant of the day *after* `to`, so a filter of
        // `column < to_timestamp()` includes every row on the `to` day.
        assert_eq!(
            w.to_timestamp().unwrap(),
            date(2026, 2, 1)
                .to_zoned(TimeZone::UTC)
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
        // MAX_WINDOW_DAYS inclusive dates is a day difference of one less.
        let from = date(2026, 1, 1);
        let to = from
            .checked_add((MAX_WINDOW_DAYS - 1).days())
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
        // One date past the cap: a day difference equal to MAX_WINDOW_DAYS.
        let from = date(2026, 1, 1);
        let to = from
            .checked_add(MAX_WINDOW_DAYS.days())
            .expect("date in range");
        let err = DateWindow {
            from: Some(from),
            to: Some(to),
        }
        .resolve();
        assert!(err.is_err());
    }

    #[test]
    fn default_window_spans_exactly_default_window_days_dates() {
        // No `from`: the default range is DEFAULT_WINDOW_DAYS inclusive dates,
        // i.e. a day difference of DEFAULT_WINDOW_DAYS - 1.
        let to = date(2026, 6, 15);
        let w = DateWindow {
            from: None,
            to: Some(to),
        }
        .resolve()
        .expect("resolves");
        assert_eq!(
            w.to.since(w.from).unwrap().get_days() as i64,
            DEFAULT_WINDOW_DAYS - 1
        );
    }
}
