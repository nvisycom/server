//! Password strength evaluation using zxcvbn.

use serde::{Deserialize, Serialize};
use zxcvbn::feedback::Feedback;
use zxcvbn::time_estimates::CrackTimeSeconds;
use zxcvbn::{Score, zxcvbn};

use crate::handler::{ErrorKind, Result};

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
    /// Creates a new password strength evaluator with default minimum score of 3.
    #[inline]
    pub const fn new() -> Self {
        Self { min_score: 3 }
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
        let entropy = zxcvbn(password, user_inputs);

        let crack_times = CrackTimes {
            offline_fast_hashing_seconds: Self::crack_time_to_f64(
                entropy.crack_times().offline_fast_hashing_1e10_per_second(),
            ),
            offline_slow_hashing_seconds: Self::crack_time_to_f64(
                entropy.crack_times().offline_slow_hashing_1e4_per_second(),
            ),
            online_no_throttling_seconds: Self::crack_time_to_f64(
                entropy.crack_times().online_no_throttling_10_per_second(),
            ),
            online_throttling_seconds: Self::crack_time_to_f64(
                entropy.crack_times().online_throttling_100_per_hour(),
            ),
        };

        let feedback = entropy.feedback().map(|f| Self::convert_feedback(f));

        PasswordStrengthResult {
            score: entropy.score().into(),
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
        let result = self.evaluate(password, user_inputs);

        if result.score <= self.min_score.into() {
            let mut error_parts = Vec::new();

            if let Some(feedback) = result.feedback {
                if let Some(warning) = feedback.warning {
                    error_parts.push(warning);
                }
                if !feedback.suggestions.is_empty() {
                    error_parts.push(format!("Suggestions: {}", feedback.suggestions.join(", ")));
                }
            }

            let error_msg = if error_parts.is_empty() {
                "Password is too weak".to_string()
            } else {
                error_parts.join(". ")
            };

            return Err(ErrorKind::BadRequest.with_context(error_msg));
        }

        Ok(())
    }

    /// Checks if a password meets the minimum strength requirement (non-error version).
    ///
    /// # Arguments
    ///
    /// * `password` - The password to check
    /// * `user_inputs` - Optional user-specific words to penalize
    pub fn meets_requirements(&self, password: &str, user_inputs: &[&str]) -> bool {
        let result = self.evaluate(password, user_inputs);
        result.score >= self.min_score
    }

    fn crack_time_to_f64(crack_time: CrackTimeSeconds) -> f64 {
        match crack_time {
            CrackTimeSeconds::Integer(i) => i as f64,
            CrackTimeSeconds::Float(f) => f,
        }
    }

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
        Self::new()
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
