//! User session management using NATS KV.

use std::collections::HashMap;
use std::time::Duration;

use async_nats::jetstream;
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument, warn};
use uuid::Uuid;

use super::KvStore;
use crate::{Result, TRACING_TARGET_KV};

/// Session store for user authentication and state
pub struct SessionStore {
    store: KvStore,
    #[allow(dead_code)]
    default_ttl: Duration,
}

impl SessionStore {
    /// Create a new session store
    #[instrument(skip(jetstream), target = TRACING_TARGET_KV)]
    pub async fn new(jetstream: &jetstream::Context, ttl: Option<Duration>) -> Result<Self> {
        let default_ttl = ttl.unwrap_or(Duration::from_secs(86400)); // 24 hours default

        let store = KvStore::new(
            jetstream,
            "sessions",
            Some("User authentication sessions"),
            Some(default_ttl),
        )
        .await?;

        debug!(
            target: TRACING_TARGET_KV,
            ttl_secs = default_ttl.as_secs(),
            "Created session store"
        );

        Ok(Self { store, default_ttl })
    }

    /// Create a new user session
    #[instrument(skip(self, session), target = TRACING_TARGET_KV)]
    pub async fn create(&self, session_id: &str, session: &UserSession) -> Result<()> {
        self.store.put(session_id, session).await?;
        debug!(
            target: TRACING_TARGET_KV,
            session_id = %session_id,
            user_id = %session.user_id,
            expires_at = %session.expires_at,
            "Created user session"
        );
        Ok(())
    }

    /// Get and validate a session
    #[instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn get(&self, session_id: &str) -> Result<Option<UserSession>> {
        match self.store.get::<UserSession>(session_id).await? {
            Some(mut session) => {
                // Check if session is expired
                if session.is_expired() {
                    warn!(
                        target: TRACING_TARGET_KV,
                        session_id = %session_id,
                        user_id = %session.user_id,
                        "Session expired, removing"
                    );
                    self.delete(session_id).await?;
                    return Ok(None);
                }

                // Update last activity
                session.last_activity = Timestamp::now();
                self.store.put(session_id, &session).await?;

                debug!(
                    target: TRACING_TARGET_KV,
                    session_id = %session_id,
                    user_id = %session.user_id,
                    idle_time_secs = session.idle_time().as_secs(),
                    "Retrieved and updated user session"
                );
                Ok(Some(session))
            }
            None => {
                debug!(
                    target: TRACING_TARGET_KV,
                    session_id = %session_id,
                    "Session not found"
                );
                Ok(None)
            }
        }
    }

    /// Delete a session
    #[instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn delete(&self, session_id: &str) -> Result<()> {
        self.store.delete(session_id).await?;
        debug!(
            target: TRACING_TARGET_KV,
            session_id = %session_id,
            "Deleted session"
        );
        Ok(())
    }

    /// Delete all sessions for a user
    #[instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn delete_user_sessions(&self, user_id: &Uuid) -> Result<u32> {
        let all_keys = self.store.keys().await?;
        let mut deleted_count = 0;

        for key in all_keys {
            if let Ok(Some(session)) = self.store.get::<UserSession>(&key).await
                && session.user_id == *user_id
            {
                self.store.delete(&key).await?;
                deleted_count += 1;
            }
        }

        debug!(
            target: TRACING_TARGET_KV,
            user_id = %user_id,
            count = deleted_count,
            "Deleted user sessions"
        );
        Ok(deleted_count)
    }

    /// Get all active sessions for a user
    #[instrument(skip(self), target = TRACING_TARGET_KV)]
    pub async fn get_user_sessions(&self, user_id: &Uuid) -> Result<Vec<UserSessionInfo>> {
        let all_keys = self.store.keys().await?;
        let mut sessions = Vec::new();

        for key in all_keys {
            if let Ok(Some(session)) = self.store.get::<UserSession>(&key).await
                && session.user_id == *user_id
                && !session.is_expired()
            {
                sessions.push(UserSessionInfo {
                    session_id: key,
                    device_info: session.device_info,
                    ip_address: session.ip_address,
                    created_at: session.created_at,
                    last_activity: session.last_activity,
                });
            }
        }

        debug!(
            target: TRACING_TARGET_KV,
            user_id = %user_id,
            count = sessions.len(),
            "Retrieved active user sessions"
        );
        Ok(sessions)
    }
}

/// User session data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSession {
    pub user_id: Uuid,
    pub session_id: String,
    pub created_at: Timestamp,
    pub last_activity: Timestamp,
    pub expires_at: Timestamp,
    pub device_info: DeviceInfo,
    pub ip_address: String,
    pub user_agent: String,
    pub permissions: Vec<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

impl UserSession {
    /// Create a new session
    pub fn new(
        user_id: Uuid,
        session_id: String,
        device_info: DeviceInfo,
        ip_address: String,
        user_agent: String,
        ttl: Duration,
    ) -> Self {
        let now = Timestamp::now();
        let expires_at = now
            .checked_add(jiff::SignedDuration::from_secs(ttl.as_secs() as i64))
            .unwrap_or(now);

        Self {
            user_id,
            session_id,
            created_at: now,
            last_activity: now,
            expires_at,
            device_info,
            ip_address,
            user_agent,
            permissions: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Check if session is expired
    pub fn is_expired(&self) -> bool {
        Timestamp::now() > self.expires_at
    }

    /// Get session age
    pub fn age(&self) -> Duration {
        let now = Timestamp::now();
        let signed_dur = now.duration_since(self.created_at);
        Duration::from_secs(signed_dur.as_secs().max(0) as u64)
    }

    /// Get time since last activity
    pub fn idle_time(&self) -> Duration {
        let now = Timestamp::now();
        let signed_dur = now.duration_since(self.last_activity);
        Duration::from_secs(signed_dur.as_secs().max(0) as u64)
    }

    /// Add a permission to the session
    pub fn add_permission(&mut self, permission: String) {
        if !self.permissions.contains(&permission) {
            self.permissions.push(permission);
        }
    }

    /// Check if session has a specific permission
    pub fn has_permission(&self, permission: &str) -> bool {
        self.permissions.contains(&permission.to_string())
    }

    /// Set metadata value
    pub fn set_metadata(&mut self, key: String, value: serde_json::Value) {
        self.metadata.insert(key, value);
    }

    /// Get metadata value
    pub fn get_metadata(&self, key: &str) -> Option<&serde_json::Value> {
        self.metadata.get(key)
    }
}

/// Device information for session tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_type: DeviceType,
    pub os: String,
    pub browser: Option<String>,
    pub device_name: Option<String>,
    pub is_mobile: bool,
    pub screen_resolution: Option<String>,
    pub timezone: Option<String>,
}

impl DeviceInfo {
    /// Create device info from user agent string
    pub fn from_user_agent(user_agent: &str) -> Self {
        // Simplified user agent parsing - in production, use a proper parser
        let is_mobile = user_agent.contains("Mobile") || user_agent.contains("Android");
        let device_type = if is_mobile {
            DeviceType::Mobile
        } else {
            DeviceType::Desktop
        };

        let os = if user_agent.contains("Windows") {
            "Windows".to_string()
        } else if user_agent.contains("Mac") {
            "macOS".to_string()
        } else if user_agent.contains("Linux") {
            "Linux".to_string()
        } else if user_agent.contains("Android") {
            "Android".to_string()
        } else if user_agent.contains("iOS") {
            "iOS".to_string()
        } else {
            "Unknown".to_string()
        };

        let browser = if user_agent.contains("Chrome") {
            Some("Chrome".to_string())
        } else if user_agent.contains("Firefox") {
            Some("Firefox".to_string())
        } else if user_agent.contains("Safari") {
            Some("Safari".to_string())
        } else if user_agent.contains("Edge") {
            Some("Edge".to_string())
        } else {
            None
        };

        Self {
            device_type,
            os,
            browser,
            device_name: None,
            is_mobile,
            screen_resolution: None,
            timezone: None,
        }
    }
}

/// Device type enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DeviceType {
    Desktop,
    Mobile,
    Tablet,
    Unknown,
}

/// Session information for user display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSessionInfo {
    pub session_id: String,
    pub device_info: DeviceInfo,
    pub ip_address: String,
    pub created_at: Timestamp,
    pub last_activity: Timestamp,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let user_id = Uuid::new_v4();
        let session_id = "session_123".to_string();
        let device_info = DeviceInfo::from_user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        );
        let ttl = Duration::from_secs(3600);

        let session = UserSession::new(
            user_id,
            session_id.clone(),
            device_info,
            "192.168.1.1".to_string(),
            "Mozilla/5.0".to_string(),
            ttl,
        );

        assert_eq!(session.user_id, user_id);
        assert_eq!(session.session_id, session_id);
        assert!(!session.is_expired());
        assert!(session.age() < Duration::from_secs(1));
    }

    #[test]
    fn test_device_info_parsing() {
        let user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36";
        let device_info = DeviceInfo::from_user_agent(user_agent);

        assert_eq!(device_info.os, "Windows");
        assert_eq!(device_info.browser, Some("Chrome".to_string()));
        assert!(!device_info.is_mobile);
        assert!(matches!(device_info.device_type, DeviceType::Desktop));
    }

    #[test]
    fn test_session_permissions() {
        let mut session = UserSession::new(
            Uuid::new_v4(),
            "test".to_string(),
            DeviceInfo::from_user_agent("test"),
            "127.0.0.1".to_string(),
            "test".to_string(),
            Duration::from_secs(3600),
        );

        session.add_permission("read".to_string());
        session.add_permission("write".to_string());

        assert!(session.has_permission("read"));
        assert!(session.has_permission("write"));
        assert!(!session.has_permission("admin"));
    }

    #[test]
    fn test_session_metadata() {
        let mut session = UserSession::new(
            Uuid::new_v4(),
            "test".to_string(),
            DeviceInfo::from_user_agent("test"),
            "127.0.0.1".to_string(),
            "test".to_string(),
            Duration::from_secs(3600),
        );

        session.set_metadata("theme".to_string(), serde_json::json!("dark"));
        session.set_metadata("language".to_string(), serde_json::json!("en"));

        assert_eq!(
            session.get_metadata("theme"),
            Some(&serde_json::json!("dark"))
        );
        assert_eq!(
            session.get_metadata("language"),
            Some(&serde_json::json!("en"))
        );
        assert_eq!(session.get_metadata("nonexistent"), None);
    }
}
