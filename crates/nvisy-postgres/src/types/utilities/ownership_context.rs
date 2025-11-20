//! Ownership and attribution helper utilities and traits for models.
//!
//! This module provides ownership and attribution capabilities for models that track
//! who created or modified entities, enabling consistent access control and audit trails.

use uuid::Uuid;

/// Trait for models that track ownership and attribution.
pub trait HasOwnership {
    /// Returns the account that created this entity.
    fn created_by(&self) -> Uuid;

    /// Returns the account that last updated this entity, if applicable.
    fn updated_by(&self) -> Option<Uuid> {
        None // Default implementation for models without updated_by
    }

    /// Returns whether the specified account created this entity.
    fn is_created_by(&self, account_id: Uuid) -> bool {
        self.created_by() == account_id
    }

    /// Returns whether the specified account last updated this entity.
    fn is_updated_by(&self, account_id: Uuid) -> bool {
        self.updated_by() == Some(account_id)
    }

    /// Returns whether the specified account has any ownership relationship.
    fn is_owned_by(&self, account_id: Uuid) -> bool {
        self.is_created_by(account_id) || self.is_updated_by(account_id)
    }

    /// Returns whether the specified account can modify this entity.
    /// Default implementation checks ownership, but can be overridden for more complex rules.
    fn can_be_modified_by(&self, account_id: Uuid) -> bool {
        self.is_owned_by(account_id)
    }

    /// Returns a summary of ownership for audit purposes.
    fn ownership_summary(&self) -> String {
        match self.updated_by() {
            Some(updated_by) if updated_by != self.created_by() => {
                format!("Created: {} | Updated: {}", self.created_by(), updated_by)
            }
            _ => format!("Created: {}", self.created_by()),
        }
    }
}
