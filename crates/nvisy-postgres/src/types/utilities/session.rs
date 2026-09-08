//! Session-lifetime policy for account API tokens.
//!
//! A session is the `account_api_tokens` row: it is the single source of truth
//! for whether a session is still valid. The bearer JWT is a signed pointer to
//! the row, checked against it on every request, so revocation and expiry are
//! both enforced by the row, not by the token in isolation.
//!
//! Two bounds govern a session's life, following the standard idle + absolute
//! timeout model (OWASP Session Management Cheat Sheet; NIST SP 800-63B):
//!
//! - an **idle timeout** ([`idle_window`]): the session dies after this much
//!   inactivity. It slides forward on use, so an actively used session stays
//!   alive. "Remember me" chooses a longer idle window.
//! - an **absolute cap** ([`MAX_AGE`]): the session cannot outlive this from its
//!   original issue, however actively it is used. It never slides.
//!
//! The idle bound lives in the row's `expired_at`; the absolute cap is derived
//! from the row's `issued_at`.

use std::time::Duration;

/// Idle window for a "remember me" session: it survives this much inactivity
/// before dying. Chosen for a long-lived, sticky login.
pub const IDLE_REMEMBERED: Duration = Duration::from_secs(7 * 24 * 60 * 60); // 7 days

/// Idle window for an ordinary (not "remembered") session: it dies after this
/// much inactivity. Shorter, for the shared- or public-computer case.
pub const IDLE_DEFAULT: Duration = Duration::from_secs(24 * 60 * 60); // 1 day

/// Absolute maximum session age from original issue. A session is rejected past
/// this regardless of activity, forcing a fresh sign-in. Independent of
/// "remember me".
pub const MAX_AGE: Duration = Duration::from_secs(90 * 24 * 60 * 60); // 90 days

/// Minimum staleness before an active session's idle bound is slid forward. The
/// slide is throttled to this interval so a burst of requests does not write on
/// every one: at most one slide write per interval of continuous use.
pub const SLIDE_THROTTLE: Duration = Duration::from_secs(5 * 60); // 5 minutes

/// Lifetime of a native-app (desktop) session token. Unlike a browser session it
/// does not slide or hit the browser absolute cap — it is a long-lived `app` token
/// the desktop stores and sends as a Bearer credential, expiring only at this
/// fixed age from issue.
pub const APP_TOKEN_LIFETIME: Duration = Duration::from_secs(365 * 24 * 60 * 60); // 1 year

/// Maximum number of live `app` (desktop) session tokens kept per account. Each
/// desktop login mints a new long-lived token automatically, so without a cap they
/// would accumulate across re-logins; on mint, the oldest beyond this many are
/// revoked. Sized to cover a handful of devices per user.
pub const MAX_APP_TOKENS_PER_ACCOUNT: usize = 10;

/// The sliding-session bounds a keep-alive slide operates under: the two idle
/// windows (chosen per the row's `is_remembered`), the absolute cap the slide
/// clamps to, and the throttle interval below which a slide is skipped.
///
/// Groups the four durations that always travel together so
/// [`slide_account_api_token`](crate::query::AccountApiTokenRepository::slide_account_api_token)
/// takes one parameter, not four positional `Duration`s.
#[derive(Debug, Clone, Copy)]
pub struct SlidingWindow {
    /// Idle window for a "remembered" session.
    pub idle_remembered: Duration,
    /// Idle window for an ordinary (not "remembered") session.
    pub idle_default: Duration,
    /// Absolute maximum age from issue; the slide never pushes past it.
    pub max_age: Duration,
    /// Minimum staleness before a slide writes, throttling bursts.
    pub throttle: Duration,
}

impl SlidingWindow {
    /// The deployment's standard sliding-session bounds, from this module's
    /// [`IDLE_REMEMBERED`], [`IDLE_DEFAULT`], [`MAX_AGE`], and [`SLIDE_THROTTLE`].
    #[must_use]
    pub const fn standard() -> Self {
        Self {
            idle_remembered: IDLE_REMEMBERED,
            idle_default: IDLE_DEFAULT,
            max_age: MAX_AGE,
            throttle: SLIDE_THROTTLE,
        }
    }
}

impl Default for SlidingWindow {
    fn default() -> Self {
        Self::standard()
    }
}

/// The idle window for a session, chosen by whether it is "remembered".
#[must_use]
pub fn idle_window(is_remembered: bool) -> Duration {
    if is_remembered {
        IDLE_REMEMBERED
    } else {
        IDLE_DEFAULT
    }
}

/// The initial idle bound (`expired_at`) for a freshly minted session: `now`
/// plus the idle window for its "remembered" state. Slid forward on use
/// thereafter, up to the absolute cap.
#[must_use]
pub fn initial_expires_at(is_remembered: bool) -> jiff::Timestamp {
    let idle = idle_window(is_remembered);
    jiff::Timestamp::now() + jiff::Span::new().seconds(idle.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_windows_are_within_the_absolute_cap() {
        // The slide clamps `expired_at` to `issued_at + MAX_AGE`; that clamp is
        // only meaningful if each idle window fits within the cap. A window longer
        // than the cap would mean a session could never reach its full idle bound.
        assert!(IDLE_REMEMBERED <= MAX_AGE);
        assert!(IDLE_DEFAULT <= MAX_AGE);
    }

    #[test]
    fn remembered_sessions_get_a_longer_idle_window() {
        assert!(IDLE_REMEMBERED > IDLE_DEFAULT);
        assert_eq!(idle_window(true), IDLE_REMEMBERED);
        assert_eq!(idle_window(false), IDLE_DEFAULT);
    }

    #[test]
    fn initial_idle_bound_reflects_remembered_state() {
        let now = jiff::Timestamp::now();
        let remembered = initial_expires_at(true);
        let default = initial_expires_at(false);

        // Both are in the future, remembered further out than default, and neither
        // exceeds the absolute cap from `now`.
        assert!(remembered > now);
        assert!(default > now);
        assert!(remembered > default);

        let cap = now + jiff::Span::new().seconds(MAX_AGE.as_secs() as i64);
        assert!(remembered <= cap);
    }
}
