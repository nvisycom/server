//! Credentials registry for workflow execution.

use std::collections::HashMap;

use uuid::Uuid;

use super::ProviderCredentials;
use crate::error::{WorkflowError, WorkflowResult};

/// In-memory credentials registry.
///
/// Stores credentials by UUID for lookup during workflow execution.
#[derive(Debug, Clone, Default)]
pub struct CredentialsRegistry {
    credentials: HashMap<Uuid, ProviderCredentials>,
}

impl CredentialsRegistry {
    /// Creates a new registry from a JSON value.
    ///
    /// Expects a JSON object with UUID keys and credential objects as values.
    pub fn new(value: serde_json::Value) -> WorkflowResult<Self> {
        let map: HashMap<Uuid, ProviderCredentials> =
            serde_json::from_value(value).map_err(WorkflowError::CredentialsRegistry)?;
        Ok(Self { credentials: map })
    }

    /// Retrieves credentials by ID.
    pub fn get(&self, credentials_id: Uuid) -> WorkflowResult<&ProviderCredentials> {
        self.credentials
            .get(&credentials_id)
            .ok_or(WorkflowError::CredentialsNotFound(credentials_id))
    }

    /// Inserts credentials with a new UUID v4.
    ///
    /// Generates a unique UUID that doesn't conflict with existing entries.
    pub fn insert(&mut self, credentials: ProviderCredentials) -> Uuid {
        loop {
            let id = Uuid::new_v4();
            if !self.credentials.contains_key(&id) {
                self.credentials.insert(id, credentials);
                return id;
            }
        }
    }

    /// Removes credentials by ID.
    pub fn remove(&mut self, credentials_id: Uuid) -> Option<ProviderCredentials> {
        self.credentials.remove(&credentials_id)
    }

    /// Lists all credential IDs.
    pub fn list(&self) -> Vec<Uuid> {
        self.credentials.keys().copied().collect()
    }
}
