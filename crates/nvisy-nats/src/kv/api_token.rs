//! API authentication token data structure.

use std::time::Duration;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Type of API token.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiTokenType {
    /// Web browser token
    #[default]
    Web,
    /// API/service token
    Api,
    /// CLI tool token
    Cli,
}

/// Token store statistics.
#[derive(Debug, Clone, Default)]
pub struct TokenStoreStats {
    /// Total number of tokens in store
    pub total_tokens: u32,
    /// Number of valid, non-expired tokens
    pub active_tokens: u32,
    /// Number of expired tokens
    pub expired_tokens: u32,
    /// Number of soft-deleted tokens
    pub deleted_tokens: u32,
    /// Number of tokens marked as suspicious
    pub suspicious_tokens: u32,
    /// Number of web tokens
    pub web_tokens: u32,
    /// Number of API tokens
    pub api_tokens: u32,
    /// Number of CLI tokens
    pub cli_tokens: u32,
}

impl TokenStoreStats {
    /// Get a summary string of the statistics.
    pub fn summary(&self) -> String {
        format!(
            "Tokens: {} total, {} active, {} expired, {} deleted, {} suspicious",
            self.total_tokens,
            self.active_tokens,
            self.expired_tokens,
            self.deleted_tokens,
            self.suspicious_tokens
        )
    }
}

/// API authentication token data structure.
///
/// Simplified token model for session management.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ApiToken {
    /// Unique token identifier used for authentication
    pub access_seq: Uuid,
    /// Reference to the account this token belongs to
    pub account_id: Uuid,
    /// IP address where token originated
    pub ip_address: String,
    /// User agent string from the client
    pub user_agent: String,
    /// Type of token (web, mobile, api)
    pub token_type: ApiTokenType,
    /// Flag indicating potentially suspicious token activity
    pub is_suspicious: bool,
    /// Timestamp of token creation
    pub issued_at: Timestamp,
    /// Timestamp when the token expires and becomes invalid
    pub expired_at: Timestamp,
    /// Timestamp of most recent token activity
    pub last_used_at: Option<Timestamp>,
    /// Timestamp when the token was soft-deleted
    pub deleted_at: Option<Timestamp>,
}

impl ApiToken {
    /// Returns whether the token is currently valid (not expired or deleted).
    pub fn is_valid(&self) -> bool {
        !self.is_expired() && !self.is_deleted()
    }

    /// Returns whether the token has expired.
    pub fn is_expired(&self) -> bool {
        Timestamp::now() > self.expired_at
    }

    /// Returns whether the token is soft-deleted.
    pub fn is_deleted(&self) -> bool {
        self.deleted_at.is_some()
    }

    /// Returns whether the token is active (valid and recently used).
    pub fn is_active(&self) -> bool {
        self.is_valid() && self.is_recently_used()
    }

    /// Returns whether the token was used recently (within last 30 minutes).
    pub fn is_recently_used(&self) -> bool {
        let now = Timestamp::now();
        if let Some(last_used) = self.last_used_at {
            let duration = now.duration_since(last_used);
            duration.as_secs() < 1800 // 30 minutes
        } else {
            false
        }
    }

    /// Returns whether the token can be refreshed.
    pub fn can_be_refreshed(&self) -> bool {
        self.is_valid() && !self.is_suspicious
    }

    /// Returns the remaining time until token expires.
    pub fn time_until_expiry(&self) -> Option<Duration> {
        if self.is_expired() {
            None
        } else {
            let remaining = self.expired_at.duration_since(Timestamp::now());
            Some(Duration::from_secs(remaining.as_secs() as u64))
        }
    }

    /// Returns the duration since the token was last used.
    pub fn time_since_last_used(&self) -> Duration {
        let reference_time = self.last_used_at.unwrap_or(self.issued_at);
        let duration = Timestamp::now().duration_since(reference_time);
        Duration::from_secs(duration.as_secs() as u64)
    }

    /// Returns whether the token is about to expire (within specified minutes).
    pub fn is_expiring_soon(&self, minutes: u64) -> bool {
        if let Some(remaining) = self.time_until_expiry() {
            remaining.as_secs() <= minutes * 60
        } else {
            true // Already expired
        }
    }

    /// Returns a shortened version of the access token for logging/display.
    pub fn access_seq_short(&self) -> String {
        let token_str = self.access_seq.to_string();
        format!("{}...", &token_str[..8])
    }

    /// Update last used timestamp (mutable operation).
    pub fn touch(&mut self) {
        self.last_used_at = Some(Timestamp::now());
    }

    /// Mark token as deleted (soft delete).
    pub fn mark_deleted(&mut self) {
        self.deleted_at = Some(Timestamp::now());
    }

    /// Mark token as suspicious.
    pub fn mark_suspicious(&mut self) {
        self.is_suspicious = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_token() -> ApiToken {
        let now = Timestamp::now();
        ApiToken {
            access_seq: Uuid::new_v4(),
            account_id: Uuid::new_v4(),
            ip_address: "192.168.1.1".to_string(),
            user_agent: "Mozilla/5.0".to_string(),
            token_type: ApiTokenType::Web,
            is_suspicious: false,
            issued_at: now,
            expired_at: now
                .checked_add(jiff::SignedDuration::from_secs(3600))
                .unwrap(),
            last_used_at: Some(now),
            deleted_at: None,
        }
    }

    #[test]
    fn test_token_validation() {
        let token = create_test_token();
        assert!(token.is_valid());
        assert!(!token.is_expired());
        assert!(!token.is_deleted());
        assert!(token.can_be_refreshed());
    }

    #[test]
    fn test_token_expiry() {
        let now = Timestamp::now();
        let mut token = create_test_token();

        // Set token to expire in the past
        token.expired_at = now
            .checked_sub(jiff::SignedDuration::from_secs(3600))
            .unwrap();

        assert!(!token.is_valid());
        assert!(token.is_expired());
        assert!(token.time_until_expiry().is_none());
    }

    #[test]
    fn test_token_soft_delete() {
        let mut token = create_test_token();
        token.mark_deleted();

        assert!(!token.is_valid());
        assert!(token.is_deleted());
        assert!(token.deleted_at.is_some());
    }

    #[test]
    fn test_token_mark_suspicious() {
        let mut token = create_test_token();
        assert!(!token.is_suspicious);

        token.mark_suspicious();
        assert!(token.is_suspicious);
        assert!(!token.can_be_refreshed());
    }

    #[test]
    fn test_token_touch() {
        let mut token = create_test_token();
        let original_last_used = token.last_used_at;

        // Small delay to ensure timestamp difference
        std::thread::sleep(std::time::Duration::from_millis(10));

        token.touch();
        assert!(token.last_used_at > original_last_used);
    }

    #[test]
    fn test_token_short_display() {
        let token = create_test_token();
        let short_access = token.access_seq_short();

        assert_eq!(short_access.len(), 11); // 8 chars + "..."
        assert!(short_access.ends_with("..."));
    }

    #[test]
    fn test_is_expiring_soon() {
        let now = Timestamp::now();
        let mut token = create_test_token();

        // Set expiry to 10 minutes from now
        token.expired_at = now
            .checked_add(jiff::SignedDuration::from_secs(600))
            .unwrap();

        assert!(token.is_expiring_soon(15)); // Within 15 minutes
        assert!(!token.is_expiring_soon(5)); // Not within 5 minutes
    }

    #[test]
    fn test_token_stats_summary() {
        let stats = TokenStoreStats {
            total_tokens: 100,
            active_tokens: 75,
            expired_tokens: 15,
            deleted_tokens: 10,
            suspicious_tokens: 5,
            web_tokens: 60,
            api_tokens: 30,
            cli_tokens: 10,
        };

        let summary = stats.summary();
        assert!(summary.contains("100 total"));
        assert!(summary.contains("75 active"));
        assert!(summary.contains("15 expired"));
        assert!(summary.contains("10 deleted"));
        assert!(summary.contains("5 suspicious"));
    }

    #[test]
    fn test_api_token_type_serialization() {
        let web = ApiTokenType::Web;
        let serialized = serde_json::to_string(&web).unwrap();
        assert_eq!(serialized, "\"web\"");

        let api = ApiTokenType::Api;
        let serialized = serde_json::to_string(&api).unwrap();
        assert_eq!(serialized, "\"api\"");

        let cli = ApiTokenType::Cli;
        let serialized = serde_json::to_string(&cli).unwrap();
        assert_eq!(serialized, "\"cli\"");
    }
}
