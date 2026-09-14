//! Invite service inputs.

use jiff::Timestamp;
use nvisy_postgres::model::NewWorkspaceInvite;
use nvisy_postgres::types::WorkspaceRole;
use uuid::Uuid;

/// Input for creating a workspace invitation addressed to an email.
pub struct CreateInviteInput {
    /// The invitee's email address, as supplied by the caller.
    pub invitee_email: String,
    /// Role the invitee will have if they accept.
    pub invited_role: WorkspaceRole,
    /// When the invitation expires, or `None` for no expiry.
    pub expires_at: Option<Timestamp>,
}

impl CreateInviteInput {
    /// The invitee email, normalized (trimmed and lowercased) so lookups and the
    /// stored value are consistent regardless of how the caller cased it.
    #[must_use]
    pub fn normalized_email(&self) -> String {
        self.invitee_email.trim().to_lowercase()
    }

    /// Builds the database model, storing the normalized invitee email.
    pub fn to_model(&self, workspace_id: Uuid, created_by: Uuid) -> NewWorkspaceInvite {
        NewWorkspaceInvite {
            workspace_id,
            invitee_email: Some(self.normalized_email()),
            invited_role: Some(self.invited_role),
            expires_at: self.expires_at.map(Into::into),
            created_by,
            updated_by: created_by,
            ..Default::default()
        }
    }
}

/// Input for generating a shareable, single-use invite code.
pub struct GenerateInviteCodeInput {
    /// Role to assign when someone joins via this code.
    pub invited_role: WorkspaceRole,
    /// When the code expires, or `None` for no expiry.
    pub expires_at: Option<Timestamp>,
}

impl GenerateInviteCodeInput {
    /// Builds the database model for an open (email-unbound) invite code.
    pub fn into_model(self, workspace_id: Uuid, created_by: Uuid) -> NewWorkspaceInvite {
        NewWorkspaceInvite {
            workspace_id,
            invitee_email: None,
            invited_role: Some(self.invited_role),
            expires_at: self.expires_at.map(Into::into),
            created_by,
            updated_by: created_by,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use nvisy_postgres::types::WorkspaceRole;

    use super::*;

    #[test]
    fn to_model_normalizes_email_and_carries_the_actor() {
        let workspace_id = Uuid::now_v7();
        let actor_id = Uuid::now_v7();
        let input = CreateInviteInput {
            invitee_email: "  Invitee@Example.com ".to_owned(),
            invited_role: WorkspaceRole::Editor,
            expires_at: Timestamp::now()
                .checked_add(jiff::Span::new().hours(24))
                .ok(),
        };

        let model = input.to_model(workspace_id, actor_id);

        assert_eq!(model.workspace_id, workspace_id);
        assert_eq!(model.invitee_email.as_deref(), Some("invitee@example.com"));
        assert_eq!(model.invited_role, Some(WorkspaceRole::Editor));
        assert_eq!(model.created_by, actor_id);
        assert_eq!(model.updated_by, actor_id);
        // The DB default supplies the token; the input never sets one.
        assert!(model.invite_token.is_none());
        assert!(model.expires_at.is_some());

        // `to_model` borrows, so the input is still usable afterwards.
        assert_eq!(input.invitee_email, "  Invitee@Example.com ");
    }
}
