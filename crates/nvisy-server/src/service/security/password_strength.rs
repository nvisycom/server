//! Password strength evaluation service.
//!
//! This module provides password strength analysis using the zxcvbn algorithm.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use zxcvbn::feedback::Feedback;
use zxcvbn::time_estimates::CrackTimeSeconds;
use zxcvbn::zxcvbn;

use crate::handler::{ErrorKind, Result};
use crate::utility::tracing_targets::PASSWORD_STRENGTH as TRACING_TARGET;

/// Password strength evaluator using the zxcvbn algorithm.
#[derive(Debug, Clone)]
pub struct PasswordStrength {
    /// Minimum acceptable score (0-4).
    min_score: u8,
}

/// Result of password strength evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordStrengthResult {
    /// Score from 0 (weakest) to 4 (strongest).
    pub score: u8,
    /// Estimated guesses required to crack the password.
    pub guesses: u64,
    /// Time estimates for cracking the password under different scenarios.
    pub crack_times: CrackTimes,
    /// Optional feedback for improving the password.
    pub feedback: Option<PasswordFeedback>,
}

/// Time estimates for cracking a password.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrackTimes {
    /// Offline attack, fast hashing (10B guesses/sec).
    pub offline_fast_hashing_seconds: f64,
    /// Offline attack, slow hashing (10K guesses/sec).
    pub offline_slow_hashing_seconds: f64,
    /// Online attack, no throttling (10 guesses/sec).
    pub online_no_throttling_seconds: f64,
    /// Online attack, with throttling (100 guesses/hour).
    pub online_throttling_seconds: f64,
}

/// Feedback for improving password strength.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PasswordFeedback {
    /// Warning message about password weaknesses.
    pub warning: Option<String>,
    /// Suggestions for improving the password.
    pub suggestions: Vec<String>,
}

impl PasswordStrength {
    /// Creates a new instance of a [`PasswordStrength`] service.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new password strength evaluator with custom minimum score.
    ///
    /// # Arguments
    ///
    /// * `min_score` - Minimum acceptable score (0-4, recommended: 3)
    #[inline]
    pub const fn with_min_score(min_score: u8) -> Self {
        Self { min_score }
    }

    /// Evaluates the strength of a password.
    ///
    /// # Arguments
    ///
    /// * `password` - The password to evaluate
    /// * `user_inputs` - Optional user-specific words to penalize (e.g., username, email)
    pub fn evaluate(&self, password: &str, user_inputs: &[&str]) -> PasswordStrengthResult {
        tracing::debug!(
            target: TRACING_TARGET,
            user_inputs_count = user_inputs.len(),
            "evaluating password strength"
        );

        let entropy = zxcvbn(password, user_inputs);

        let crack_times = CrackTimes {
            offline_fast_hashing_seconds: Self::crack_time(
                entropy.crack_times().offline_fast_hashing_1e10_per_second(),
            )
            .as_secs_f64(),
            offline_slow_hashing_seconds: Self::crack_time(
                entropy.crack_times().offline_slow_hashing_1e4_per_second(),
            )
            .as_secs_f64(),
            online_no_throttling_seconds: Self::crack_time(
                entropy.crack_times().online_no_throttling_10_per_second(),
            )
            .as_secs_f64(),
            online_throttling_seconds: Self::crack_time(
                entropy.crack_times().online_throttling_100_per_hour(),
            )
            .as_secs_f64(),
        };

        let feedback = entropy.feedback().map(Self::convert_feedback);
        let score: u8 = entropy.score().into();

        tracing::debug!(
            target: TRACING_TARGET,
            score = score,
            guesses = entropy.guesses(),
            has_feedback = feedback.is_some(),
            "password strength evaluation completed"
        );

        PasswordStrengthResult {
            score,
            guesses: entropy.guesses(),
            crack_times,
            feedback,
        }
    }

    /// Validates a password meets minimum strength requirement.
    ///
    /// # Arguments
    ///
    /// * `password` - The password to check
    /// * `user_inputs` - Optional user-specific words to penalize
    ///
    /// # Errors
    ///
    /// Returns an HTTP 400 Bad Request error with suggestions if password is too weak.
    pub fn validate_password(&self, password: &str, user_inputs: &[&str]) -> Result<()> {
        tracing::debug!(
            target: TRACING_TARGET,
            min_score = self.min_score,
            "validating password strength"
        );

        let result = self.evaluate(password, user_inputs);

        if result.score <= self.min_score {
            tracing::warn!(
                target: TRACING_TARGET,
                score = result.score,
                min_score = self.min_score,
                has_warning = result.feedback.as_ref().and_then(|f| f.warning.as_ref()).is_some(),
                suggestions_count = result.feedback.as_ref().map(|f| f.suggestions.len()).unwrap_or(0),
                "password validation failed: insufficient strength"
            );

            let mut error = ErrorKind::BadRequest
                .with_message("Password does not meet minimum strength requirements")
                .with_resource("password");

            if let Some(feedback) = result.feedback {
                // Add warning as context if present
                if let Some(warning) = feedback.warning {
                    error = error.with_context(warning);
                }

                // Add suggestions as suggestion field
                if !feedback.suggestions.is_empty() {
                    error = error.with_suggestion(feedback.suggestions.join("; "));
                }
            }

            return Err(error);
        }

        tracing::debug!(
            target: TRACING_TARGET,
            score = result.score,
            "password validation successful"
        );

        Ok(())
    }

    /// Checks if a password meets the minimum strength requirement (non-error version).
    ///
    /// # Arguments
    ///
    /// * `password` - The password to check
    /// * `user_inputs` - Optional user-specific words to penalize
    pub fn meets_requirements(&self, password: &str, user_inputs: &[&str]) -> bool {
        tracing::debug!(
            target: TRACING_TARGET,
            min_score = self.min_score,
            "checking if password meets requirements"
        );

        let result = self.evaluate(password, user_inputs);
        let meets_requirements = result.score >= self.min_score;

        tracing::debug!(
            target: TRACING_TARGET,
            score = result.score,
            min_score = self.min_score,
            meets_requirements = meets_requirements,
            "password requirements check completed"
        );

        meets_requirements
    }

    /// Converts a `CrackTimeSeconds` value to a `Duration`.
    ///
    /// # Arguments
    ///
    /// * `crack_time` - The crack time to convert
    fn crack_time(crack_time: CrackTimeSeconds) -> Duration {
        match crack_time {
            CrackTimeSeconds::Integer(i) => Duration::from_secs(i),
            CrackTimeSeconds::Float(f) => Duration::from_secs_f64(f),
        }
    }

    /// Converts a `Feedback` value to a `PasswordFeedback`.
    ///
    /// # Arguments
    ///
    /// * `feedback` - The feedback to convert
    fn convert_feedback(feedback: &Feedback) -> PasswordFeedback {
        PasswordFeedback {
            warning: feedback.warning().map(|w| w.to_string()),
            suggestions: feedback
                .suggestions()
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
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

    #[test]
    fn test_weak_password() {
        let checker = PasswordStrength::new();
        let result = checker.evaluate("password", &[]);
        assert!(result.score < 3);
        assert!(result.feedback.is_some());
    }

    #[test]
    fn test_strong_password() {
        let checker = PasswordStrength::new();
        let result = checker.evaluate("kX9$mP2#vL5@wQ8!", &[]);
        assert!(result.score >= 3);
    }

    #[test]
    fn test_with_user_inputs() {
        let checker = PasswordStrength::new();
        let result = checker.evaluate("john1234", &["john", "smith"]);
        assert!(result.score < 3);
    }

    #[test]
    fn test_meets_requirements() {
        let checker = PasswordStrength::new();
        assert!(!checker.meets_requirements("password", &[]));
        assert!(checker.meets_requirements("kX9$mP2#vL5@wQ8!", &[]));
    }

    #[test]
    fn test_custom_min_score() {
        let lenient = PasswordStrength::with_min_score(0);
        let strict = PasswordStrength::with_min_score(4);

        // Lenient accepts weak passwords
        assert!(lenient.meets_requirements("password", &[]));

        // Strict requires very strong passwords (score 4)
        assert!(!strict.meets_requirements("password123", &[]));
        assert!(strict.meets_requirements("kX9$mP2#vL5@wQ8!xYz", &[]));
    }
}
