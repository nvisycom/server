//! Human-friendly durations for configuration values.

use std::time::Duration;

use jiff::{Span, SpanRelativeTo};

/// Parses a duration written like `500ms`, `30s`, `1h 30m`, `30d`, or ISO 8601
/// `PT1H`.
///
/// Days and weeks are fixed lengths (24 hours and 7 days), so a value never
/// depends on a calendar. Months and years have no fixed length and are
/// rejected, as are negative durations.
///
/// # Errors
///
/// Returns a message when `input` is not a duration in jiff's friendly or ISO
/// 8601 format, uses months or years, or is negative.
pub fn parse_duration(input: &str) -> Result<Duration, String> {
    let span: Span = input.parse().map_err(|err: jiff::Error| err.to_string())?;
    let signed = span
        .to_duration(SpanRelativeTo::days_are_24_hours())
        .map_err(|err| err.to_string())?;
    Duration::try_from(signed).map_err(|_| format!("duration must not be negative: {input}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_friendly_units() {
        assert_eq!(parse_duration("500ms"), Ok(Duration::from_millis(500)));
        assert_eq!(parse_duration("30s"), Ok(Duration::from_secs(30)));
        assert_eq!(parse_duration("1h 30m"), Ok(Duration::from_mins(90)));
    }

    #[test]
    fn days_and_weeks_are_fixed_length() {
        assert_eq!(parse_duration("30d"), Ok(Duration::from_hours(30 * 24)));
        assert_eq!(parse_duration("1w"), Ok(Duration::from_hours(7 * 24)));
    }

    #[test]
    fn parses_iso_8601() {
        assert_eq!(parse_duration("PT1H"), Ok(Duration::from_hours(1)));
    }

    #[test]
    fn rejects_calendar_units_negatives_and_garbage() {
        assert!(parse_duration("1mo").is_err());
        assert!(parse_duration("-5s").is_err());
        assert!(parse_duration("soon").is_err());
    }
}
