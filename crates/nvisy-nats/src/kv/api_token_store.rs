//! API token store operations using NATS KV.

use std::time::Duration;

use async_nats::jetstream;
use derive_more::{Deref, DerefMut};
use jiff::Timestamp;
use uuid::Uuid;

use crate::kv::KvStore;
use crate::kv::api_token::{ApiToken, ApiTokenType};
use crate::{Result, TRACING_TARGET_KV};

/// API token store for authentication token management.
///
/// Provides operations for creating, retrieving, updating, and managing
/// API authentication tokens with automatic expiry handling.
#[derive(Deref, DerefMut)]
pub struct ApiTokenStore {
    #[deref]
    #[deref_mut]
    store: KvStore<ApiToken>,
    default_ttl: Duration,
}

impl ApiTokenStore {
    /// Create a new API token store.
    ///
    /// # Arguments
    /// * `jetstream` - JetStream context for NATS operations
    /// * `ttl` - Default time-to-live for tokens (defaults to 24 hours)
    #[tracing::instrument(skip(jetstream), target = TRACING_TARGET_KV)]
    pub async fn new(jetstream: &jetstream::Context, ttl: Option<Duration>) -> Result<Self> {
        let default_ttl = ttl.unwrap_or(Duration::from_secs(86400)); // 24 hours default

        let store = KvStore::new(
            jetstream,
            "api_tokens",
            Some("API authentication tokens"),
            Some(default_ttl),
        )
        .await?;

        tracing::info!(
            target: TRACING_TARGET_KV,
            ttl_hours = default_ttl.as_secs() / 3600,
            bucket = %store.bucket_name(),
            "Created API token store"
        );

        Ok(Self { store, default_ttl })
    }

    /// Create and store a new API token.
    ///
    /// # Arguments
    /// * `account_id` - Account UUID this token belongs to
    /// * `token_type` - Type of token (web, mobile, api)
    /// * `ip_address` - IP address where token originated
    /// * `user_agent` - User agent string from client
    /// * `ttl` - Token lifetime (uses default if None)
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn create_token(
        &self,
        account_id: Uuid,
        token_type: ApiTokenType,
        ip_address: String,
        user_agent: String,
        ttl: Option<Duration>,
    ) -> Result<ApiToken> {
        let token_ttl = ttl.unwrap_or(self.default_ttl);
        let now = Timestamp::now();
        let expires_at = now
            .checked_add(jiff::SignedDuration::from_secs(token_ttl.as_secs() as i64))
            .unwrap_or(
                now.checked_add(jiff::SignedDuration::from_secs(86400))
                    .unwrap_or(now),
            );

        let token = ApiToken {
            access_seq: Uuid::new_v4(),
            account_id,
            ip_address,
            user_agent,
            token_type,
            is_suspicious: false,
            issued_at: now,
            expired_at: expires_at,
            last_used_at: Some(now),
            deleted_at: None,
        };

        let token_key = token.access_seq.to_string();
        self.store.put(&token_key, &token).await?;

        tracing::info!(
            target: TRACING_TARGET_KV,
            access_seq = %token.access_seq,
            account_id = %token.account_id,
            token_type = ?token.token_type,
            expires_at = %token.expired_at,
            ip_address = %token.ip_address,
            "Created new API token"
        );

        Ok(token)
    }

    /// Retrieve and validate an API token by access sequence.
    ///
    /// Returns None if token doesn't exist, is expired, or is deleted.
    /// Does NOT automatically update last_used_at to avoid write amplification.
    /// Use `touch_token()` separately if you need to update the timestamp.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn get_token(&self, access_seq: &Uuid) -> Result<Option<ApiToken>> {
        let token_key = access_seq.to_string();

        match self.store.get(&token_key).await? {
            Some(kv_token) => {
                let token = kv_token.value;

                // Check if token is deleted
                if token.is_deleted() {
                    tracing::debug!(
                        target: TRACING_TARGET_KV,
                        access_seq = %access_seq,
                        deleted_at = ?token.deleted_at,
                        "Token is soft-deleted"
                    );
                    return Ok(None);
                }

                // Check if token is expired
                if token.is_expired() {
                    tracing::warn!(
                        target: TRACING_TARGET_KV,
                        access_seq = %access_seq,
                        expired_at = %token.expired_at,
                        "Token has expired"
                    );

                    // Soft delete expired token
                    self.delete_token(access_seq).await?;
                    return Ok(None);
                }

                tracing::debug!(
                    target: TRACING_TARGET_KV,
                    access_seq = %access_seq,
                    account_id = %token.account_id,
                    last_used_at = ?token.last_used_at,
                    "Retrieved API token"
                );

                Ok(Some(token))
            }
            None => {
                tracing::debug!(
                    target: TRACING_TARGET_KV,
                    access_seq = %access_seq,
                    "Token not found"
                );
                Ok(None)
            }
        }
    }

    /// Update the last_used_at timestamp for a token.
    ///
    /// Call this periodically (e.g., every 5 minutes) instead of on every access
    /// to avoid write amplification while still tracking activity.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn touch_token(&self, access_seq: &Uuid) -> Result<bool> {
        let token_key = access_seq.to_string();

        if let Some(kv_token) = self.store.get(&token_key).await? {
            let mut token = kv_token.value;

            if token.is_valid() {
                token.touch();
                self.store.put(&token_key, &token).await?;

                tracing::debug!(
                    target: TRACING_TARGET_KV,
                    access_seq = %access_seq,
                    last_used_at = ?token.last_used_at,
                    "Updated token last_used_at"
                );

                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Mark a token as deleted (soft delete).
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn delete_token(&self, access_seq: &Uuid) -> Result<()> {
        let token_key = access_seq.to_string();

        if let Some(kv_token) = self.store.get(&token_key).await? {
            let mut token = kv_token.value;
            token.mark_deleted();

            self.store.put(&token_key, &token).await?;

            tracing::info!(
                target: TRACING_TARGET_KV,
                access_seq = %access_seq,
                account_id = %token.account_id,
                deleted_at = ?token.deleted_at,
                "Soft-deleted API token"
            );
        }

        Ok(())
    }

    /// Delete all tokens for a specific account.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn delete_account_tokens(&self, account_id: &Uuid) -> Result<u32> {
        let all_keys = self.store.keys().await?;
        let mut deleted_count = 0;

        for key in all_keys {
            if let Ok(Some(kv_token)) = self.store.get(&key).await
                && kv_token.value.account_id == *account_id
                && !kv_token.value.is_deleted()
                && let Ok(access_seq) = Uuid::parse_str(&key)
            {
                self.delete_token(&access_seq).await?;
                deleted_count += 1;
            }
        }

        tracing::info!(
            target: TRACING_TARGET_KV,
            account_id = %account_id,
            deleted_count = deleted_count,
            "Deleted all account tokens"
        );

        Ok(deleted_count)
    }

    /// Get all active tokens for a specific account.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn get_account_tokens(&self, account_id: &Uuid) -> Result<Vec<ApiToken>> {
        let all_keys = self.store.keys().await?;
        let mut tokens = Vec::new();

        for key in all_keys {
            if let Ok(Some(kv_token)) = self.store.get(&key).await {
                let token = kv_token.value;
                if token.account_id == *account_id && token.is_valid() {
                    tokens.push(token);
                }
            }
        }

        // Sort by most recently used
        tokens.sort_by(|a, b| {
            b.last_used_at
                .unwrap_or(b.issued_at)
                .cmp(&a.last_used_at.unwrap_or(a.issued_at))
        });

        tracing::debug!(
            target: TRACING_TARGET_KV,
            account_id = %account_id,
            active_tokens = tokens.len(),
            "Retrieved account tokens"
        );

        Ok(tokens)
    }

    /// Mark a token as suspicious.
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn mark_suspicious(&self, access_seq: &Uuid) -> Result<bool> {
        let token_key = access_seq.to_string();

        if let Some(kv_token) = self.store.get(&token_key).await? {
            let mut token = kv_token.value;

            if !token.is_suspicious {
                token.mark_suspicious();
                self.store.put(&token_key, &token).await?;

                tracing::warn!(
                    target: TRACING_TARGET_KV,
                    access_seq = %access_seq,
                    account_id = %token.account_id,
                    "Marked token as suspicious"
                );

                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Clean up expired and deleted tokens (maintenance operation).
    #[tracing::instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn cleanup_expired(&self) -> Result<u32> {
        let all_keys = self.store.keys().await?;
        let mut cleaned_count = 0;
        let now = Timestamp::now();

        // Define cleanup threshold (delete tokens that have been soft-deleted for more than 7 days)
        let cleanup_threshold = now
            .checked_sub(jiff::SignedDuration::from_secs(7 * 24 * 3600))
            .unwrap_or(now);

        for key in all_keys {
            if let Ok(Some(kv_token)) = self.store.get(&key).await {
                let token = kv_token.value;

                // Hard delete tokens that have been soft-deleted for more than the threshold
                if let Some(deleted_at) = token.deleted_at
                    && deleted_at < cleanup_threshold
                {
                    self.store.delete(&key).await?;
                    cleaned_count += 1;
                    continue;
                }

                // Hard delete expired tokens that haven't been accessed in the threshold period
                if token.is_expired() {
                    let last_activity = token.last_used_at.unwrap_or(token.issued_at);
                    if last_activity < cleanup_threshold {
                        self.store.delete(&key).await?;
                        cleaned_count += 1;
                    }
                }
            }
        }

        tracing::info!(
            target: TRACING_TARGET_KV,
            cleaned_count = cleaned_count,
            cleanup_threshold = %cleanup_threshold,
            "Cleaned up expired tokens"
        );

        Ok(cleaned_count)
    }

    /// Get the default TTL for tokens.
    pub fn default_ttl(&self) -> Duration {
        self.default_ttl
    }
}
