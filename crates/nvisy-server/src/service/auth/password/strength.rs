//! Password strength policy, backed by the zxcvbn estimator.

use zxcvbn::feedback::Feedback;
use zxcvbn::zxcvbn;

use crate::response::{ErrorKind, Result};

/// Tracing target for password strength operations.
const TRACING_TARGET: &str = "nvisy_server::service::password";

/// Rejects passwords weaker than a minimum zxcvbn score (0 weakest, 4 strongest).
#[derive(Debug, Clone)]
pub struct PasswordStrength {
    /// Passwords scoring below this are rejected.
    min_score: u8,
}

impl PasswordStrength {
    /// A policy with the default minimum score.
    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A policy rejecting passwords scoring below `min_score` (0–4).
    #[inline]
    #[must_use]
    pub const fn with_min_score(min_score: u8) -> Self {
        Self { min_score }
    }

    /// Accepts the password if it meets the minimum score, else rejects it with
    /// the estimator's improvement suggestions.
    ///
    /// `user_inputs` are values (email, name) the password is penalized for
    /// resembling.
    ///
    /// # Errors
    ///
    /// `BadRequest` if the password scores below the minimum.
    pub fn validate_password(&self, password: &str, user_inputs: &[&str]) -> Result<()> {
        let entropy = zxcvbn(password, user_inputs);
        let score: u8 = entropy.score().into();

        if score >= self.min_score {
            tracing::debug!(target: TRACING_TARGET, score, "password meets the strength policy");
            return Ok(());
        }

        // Surface the estimator's suggestions so the client sees how to fix it;
        // its warning stays in the (logged-only) context.
        let feedback = entropy.feedback();
        let suggestions: Vec<String> = feedback
            .map(|f| f.suggestions().iter().map(ToString::to_string).collect())
            .unwrap_or_default();
        let warning = feedback.and_then(Feedback::warning).map(|w| w.to_string());

        tracing::warn!(
            target: TRACING_TARGET,
            score,
            min_score = self.min_score,
            suggestions = suggestions.len(),
            "password rejected: below the strength policy",
        );

        let mut message = String::from("Password does not meet minimum strength requirements");
        if !suggestions.is_empty() {
            message.push_str(": ");
            message.push_str(&suggestions.join("; "));
        }
        let mut error = ErrorKind::BadRequest.with_message(message);
        if let Some(warning) = warning {
            error = error.with_context(warning);
        }
        Err(error)
    }
}

impl Default for PasswordStrength {
    #[inline]
    fn default() -> Self {
        Self::with_min_score(3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A higher `min_score` rejects a weak password that `min_score = 0` accepts.
    /// A short common password reliably scores at the bottom of the scale, so this
    /// does not depend on zxcvbn's exact score.
    #[test]
    fn stricter_min_score_rejects_more() {
        let weak = "password1";
        assert!(
            PasswordStrength::with_min_score(0)
                .validate_password(weak, &[])
                .is_ok()
        );
        assert!(
            PasswordStrength::with_min_score(4)
                .validate_password(weak, &[])
                .is_err()
        );
    }

    /// A password exactly at `min_score` is accepted: the boundary is `>=`, not
    /// `>`, so the default (min 3) admits score-3 passwords as documented — this
    /// is the off-by-one the old `score <= min_score` got wrong.
    #[test]
    fn score_at_the_minimum_is_accepted() {
        let password = "sunflower-garden-42";
        let score: u8 = zxcvbn(password, &[]).score().into();
        assert!(
            PasswordStrength::with_min_score(score)
                .validate_password(password, &[])
                .is_ok()
        );
    }
}
