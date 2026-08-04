//! Cron-based due checks for scheduled connection syncs.
//!
//! Connections store a cron expression; the scheduler asks whether a
//! connection is due relative to its last successful sync. croner operates on
//! `chrono` datetimes, so this module bridges jiff timestamps to chrono at the
//! Unix-second boundary and keeps that conversion isolated here.

use std::str::FromStr;

use chrono::{DateTime, Utc};
use croner::Cron;
use jiff::Timestamp;

/// Whether a cron-scheduled connection is due to sync at `now`.
///
/// A connection is due when the first cron occurrence strictly after
/// `last_sync` is at or before `now`. A connection that has never synced
/// (`last_sync` is `None`) is treated as if its last sync was the Unix epoch,
/// so it becomes due at its first scheduled occurrence.
///
/// Returns `false` if the cron expression fails to parse or its next
/// occurrence cannot be computed, so a malformed schedule never fires.
pub fn is_cron_due(cron: &str, last_sync: Option<Timestamp>, now: Timestamp) -> bool {
    let Ok(schedule) = Cron::from_str(cron) else {
        return false;
    };

    let since = last_sync.unwrap_or(Timestamp::UNIX_EPOCH);
    let Some(after) = to_chrono(since) else {
        return false;
    };

    match schedule.find_next_occurrence(&after, false) {
        Ok(next) => next.timestamp() <= now.as_second(),
        Err(_) => false,
    }
}

/// Validates a cron expression, returning whether it parses.
pub fn is_valid_cron(cron: &str) -> bool {
    Cron::from_str(cron).is_ok()
}

/// Bridges a jiff [`Timestamp`] to a chrono UTC datetime via Unix seconds.
fn to_chrono(ts: Timestamp) -> Option<DateTime<Utc>> {
    DateTime::from_timestamp(ts.as_second(), 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(secs: i64) -> Timestamp {
        Timestamp::from_second(secs).unwrap()
    }

    #[test]
    fn every_minute_due_after_a_minute() {
        // "* * * * *" fires every minute. Last sync at t=0; at t=120 the next
        // occurrence after 0 (t=60) is already past -> due.
        assert!(is_cron_due("* * * * *", Some(ts(0)), ts(120)));
    }

    #[test]
    fn not_due_before_next_occurrence() {
        // Daily at midnight; last sync just after an occurrence, now only a few
        // seconds later -> next midnight is far away -> not due.
        let last = ts(0); // 1970-01-01 00:00:00 UTC (a midnight)
        assert!(!is_cron_due("0 0 * * *", Some(last), ts(30)));
    }

    #[test]
    fn never_synced_is_due_once_first_occurrence_passed() {
        // Never synced (epoch baseline); by t=120 a per-minute schedule has
        // fired -> due.
        assert!(is_cron_due("* * * * *", None, ts(120)));
    }

    #[test]
    fn malformed_cron_never_due() {
        assert!(!is_cron_due("not a cron", Some(ts(0)), ts(999999)));
        assert!(!is_valid_cron("nope"));
        assert!(is_valid_cron("0 0 * * *"));
    }
}
