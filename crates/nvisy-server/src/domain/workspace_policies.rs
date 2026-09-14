//! Workspace policy domain logic: create (authored or one-shot), read, list,
//! update, and delete.
//!
//! Holds the policy rules — one-shot content-addressing, the authored/one-shot
//! split, and one-shot immutability — in one place, factored out of the handler
//! so they can be reasoned about (and tested) independently of the HTTP flow.

use elide_pipeline::governance::policy::Policy;
use nvisy_postgres::model::{NewWorkspacePolicy, WorkspacePolicy, WorkspacePolicyVersion};
use nvisy_postgres::query::{
    PolicyCursor, WorkspacePolicyRepository, WorkspacePolicyVersionRepository,
};
use nvisy_postgres::types::{CursorPage, CursorPagination, PolicyKind, WithAccountRef};
use nvisy_postgres::{AsyncConnection, PgClient, PgConn, model};
use uuid::Uuid;

use crate::domain::input::{CreatePolicyInput, PolicyBodyInput, UpdatePolicyInput};
use crate::domain::output::ResolvedPolicy;
use crate::response::{Error, ErrorKind, Result};
use crate::service::event;
use crate::service::event::EventEmitter;

/// Tracing target for policy domain operations.
const TRACING_TARGET: &str = "nvisy_server::domain::policy";

/// Creates, reads, updates, and deletes workspace policies.
///
/// Holds the Postgres client and acquires its own connection per call, so each
/// method is a self-contained transaction. Resolved per request from
/// [`ServiceState`].
///
/// [`ServiceState`]: crate::service::ServiceState
#[derive(Clone)]
pub struct WorkspacePolicyService {
    postgres: PgClient,
}

impl WorkspacePolicyService {
    /// Creates a [`WorkspacePolicyService`] over the given connection pool.
    #[must_use]
    pub fn new(postgres: PgClient) -> Self {
        Self { postgres }
    }

    /// Creates a policy from a create request.
    ///
    /// A labels body is content-addressed: it mints — or reuses an identical live
    /// one — a one-shot policy with a hash-derived name (`created` is false on
    /// reuse). A template or inline body is a permanent authored policy the caller
    /// names. The policy, its first version, and the creation event commit
    /// together.
    ///
    /// # Errors
    ///
    /// - `InternalServerError` if the policy definition cannot be serialized.
    /// - A database error if the query fails.
    pub async fn create(
        &self,
        origin: event::EventOrigin<'_>,
        input: CreatePolicyInput,
    ) -> Result<ResolvedPolicy> {
        let mut conn = self.postgres.get_connection().await?;
        match input.body.oneshot_content_hash() {
            Some(content_hash) => {
                self.create_oneshot(&mut conn, origin, input.body, content_hash)
                    .await
            }
            None => self.create_authored(&mut conn, origin, input).await,
        }
    }

    /// Creates an authored (template or inline) policy: a new permanent row with
    /// its first version and a creation event, in one transaction.
    async fn create_authored(
        &self,
        conn: &mut PgConn,
        origin: event::EventOrigin<'_>,
        input: CreatePolicyInput,
    ) -> Result<ResolvedPolicy> {
        let definition = input.body.into_definition("");
        let display_name = input
            .display_name
            .unwrap_or_else(|| definition.name.to_string());
        let description = input
            .description
            .or_else(|| definition.description.clone().map(Into::into));
        let body = definition_to_json(&definition)?;

        let new_policy = NewWorkspacePolicy {
            workspace_id: origin.workspace_id,
            account_id: origin.account_id,
            display_name,
            description,
            kind: PolicyKind::Authored,
            content_hash: None,
            metadata: None,
        };

        let created = conn
            .transaction(async |conn| {
                let created = conn.create_workspace_policy(new_policy, body, None).await?;
                conn.emit_event(
                    origin,
                    event::WorkspaceEvent::PolicyCreated(event::PolicyCreated {
                        policy_id: created.policy.id,
                    }),
                )
                .await?;
                Ok::<_, Error>(created)
            })
            .await?;

        tracing::info!(target: TRACING_TARGET, policy_id = %created.policy.id, "Policy created");
        Ok(ResolvedPolicy {
            policy: created.policy,
            version: created.version,
            created: true,
        })
    }

    /// Creates or reuses a one-shot policy from a labels body, content-addressed by
    /// `content_hash`: an identical live one-shot is reused rather than duplicated,
    /// and a fresh one is created with a hash-derived name plus a creation event,
    /// all in one transaction.
    async fn create_oneshot(
        &self,
        conn: &mut PgConn,
        origin: event::EventOrigin<'_>,
        body: PolicyBodyInput,
        content_hash: Vec<u8>,
    ) -> Result<ResolvedPolicy> {
        let display_name = oneshot_display_name(&content_hash);

        let definition = body.into_definition(&display_name);
        let definition_json = definition_to_json(&definition)?;

        let new_policy = NewWorkspacePolicy {
            workspace_id: origin.workspace_id,
            account_id: origin.account_id,
            display_name,
            description: None,
            kind: PolicyKind::Oneshot,
            content_hash: Some(content_hash.clone()),
            metadata: None,
        };

        let resolved = conn
            .transaction(async |conn| {
                let resolved = conn
                    .find_or_create_oneshot_policy(new_policy, content_hash, definition_json, None)
                    .await?;
                if resolved.created {
                    conn.emit_event(
                        origin,
                        event::WorkspaceEvent::PolicyCreated(event::PolicyCreated {
                            policy_id: resolved.policy.policy.id,
                        }),
                    )
                    .await?;
                }
                Ok::<_, Error>(resolved)
            })
            .await?;

        if resolved.created {
            tracing::info!(target: TRACING_TARGET, policy_id = %resolved.policy.policy.id, "One-shot policy created");
        }

        Ok(ResolvedPolicy {
            policy: resolved.policy.policy,
            version: resolved.policy.version,
            created: resolved.created,
        })
    }

    /// Lists a workspace's policies, newest first. `kind` narrows to a single
    /// policy kind; `None` returns every kind.
    ///
    /// # Errors
    ///
    /// A database error if the query fails.
    pub async fn list(
        &self,
        workspace_id: Uuid,
        pagination: CursorPagination<PolicyCursor>,
        kind: Option<PolicyKind>,
    ) -> Result<CursorPage<WithAccountRef<WorkspacePolicy>>> {
        let mut conn = self.postgres.get_connection().await?;
        Ok(conn
            .cursor_list_workspace_policies(workspace_id, pagination, kind)
            .await?)
    }

    /// Finds a policy by id with its creator and current version, or a `NotFound`.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the policy does not exist in the workspace or has no current
    ///   version.
    /// - A database error if the query fails.
    pub async fn find(
        &self,
        workspace_id: Uuid,
        policy_id: Uuid,
    ) -> Result<(WithAccountRef<WorkspacePolicy>, WorkspacePolicyVersion)> {
        let mut conn = self.postgres.get_connection().await?;
        let found = find_policy(&mut conn, workspace_id, policy_id).await?;
        let version = current_version(&mut conn, workspace_id, &found.item).await?;
        Ok((found, version))
    }

    /// Updates an authored policy, returning it with its current version and
    /// creator.
    ///
    /// A definition change mints a new version; a label-only change mutates the
    /// logical row in place. The write and its event commit together. A one-shot
    /// policy is immutable — content-addressed and reused by identity — so any edit
    /// to one is rejected; to change it, create a new policy.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the policy does not exist in the workspace or has no current
    ///   version.
    /// - `BadRequest` if the policy is a one-shot (immutable).
    /// - `InternalServerError` if the stored definition is malformed or the new
    ///   definition cannot be serialized.
    /// - A database error if the query fails.
    pub async fn update(
        &self,
        origin: event::EventOrigin<'_>,
        policy_id: Uuid,
        input: UpdatePolicyInput,
    ) -> Result<(WithAccountRef<WorkspacePolicy>, WorkspacePolicyVersion)> {
        let mut conn = self.postgres.get_connection().await?;
        let existing = find_policy(&mut conn, origin.workspace_id, policy_id)
            .await?
            .item;

        if existing.kind == PolicyKind::Oneshot {
            return Err(ErrorKind::BadRequest
                .with_message("One-shot policies are immutable; create a new policy instead"));
        }

        let current = current_version(&mut conn, origin.workspace_id, &existing).await?;

        // A replaced body keeps the policy's server-owned template origin: the
        // caller authored new rules, but where the policy came from is provenance
        // the client cannot set or clear. Carry the current version's origin forward.
        let new_definition = match input.definition {
            Some(draft) => {
                let template = serde_json::from_value::<Policy>(current.definition)
                    .map_err(|err| malformed_definition(&err))?
                    .template;
                let definition = draft.into_definition(template);
                Some(definition_to_json(&definition)?)
            }
            None => None,
        };

        let account_id = origin.account_id;
        let workspace_id = origin.workspace_id;
        conn.transaction(async |conn| {
            if let Some(definition) = new_definition {
                conn.create_policy_version(workspace_id, policy_id, account_id, definition, None)
                    .await?;
            }
            // Only touch the logical row when there is a label field to change:
            // a definition-only edit mints a version but leaves the row's own
            // columns alone, so an empty changeset is never issued.
            if input.display_name.is_some() || input.description.is_some() {
                conn.update_workspace_policy(
                    policy_id,
                    model::UpdateWorkspacePolicy {
                        display_name: input.display_name,
                        description: input.description,
                        ..Default::default()
                    },
                )
                .await?;
            }
            conn.emit_event(
                origin,
                event::WorkspaceEvent::PolicyUpdated(event::PolicyUpdated { policy_id }),
            )
            .await?;
            Ok::<(), Error>(())
        })
        .await?;

        tracing::info!(target: TRACING_TARGET, "Policy updated");
        self.find(workspace_id, policy_id).await
    }

    /// Soft-deletes a policy from its workspace, recording the event atomically.
    ///
    /// # Errors
    ///
    /// - `NotFound` if the policy does not exist in the workspace.
    /// - A database error if the query fails.
    pub async fn delete(&self, origin: event::EventOrigin<'_>, policy_id: Uuid) -> Result<()> {
        let mut conn = self.postgres.get_connection().await?;
        // Confirm the policy exists in the workspace before deleting.
        find_policy(&mut conn, origin.workspace_id, policy_id).await?;

        conn.transaction(async |conn| {
            conn.delete_workspace_policy(policy_id).await?;
            conn.emit_event(
                origin,
                event::WorkspaceEvent::PolicyDeleted(event::PolicyDeleted { policy_id }),
            )
            .await?;
            Ok::<(), Error>(())
        })
        .await?;

        tracing::info!(target: TRACING_TARGET, "Policy deleted");
        Ok(())
    }
}

/// Serializes a policy definition to the plaintext JSONB stored on its version.
fn definition_to_json(definition: &Policy) -> Result<serde_json::Value> {
    serde_json::to_value(definition).map_err(|err| {
        ErrorKind::InternalServerError
            .with_message("Failed to serialize the policy definition")
            .with_context(err.to_string())
    })
}

/// Maps a failed deserialization of a stored definition to a server error.
fn malformed_definition(err: &serde_json::Error) -> Error<'static> {
    ErrorKind::InternalServerError
        .with_message("Stored policy definition is malformed")
        .with_context(err.to_string())
}

/// Finds a policy within a workspace by id, with its creator, or a `NotFound`.
async fn find_policy(
    conn: &mut PgConn,
    workspace_id: Uuid,
    policy_id: Uuid,
) -> Result<WithAccountRef<WorkspacePolicy>> {
    conn.find_policy_in_workspace_by_id(workspace_id, policy_id)
        .await?
        .ok_or_else(|| Error::not_found("policy"))
}

/// Loads a policy's current version (the one whose definition the engine
/// consumes). A live policy always has a current version.
async fn current_version(
    conn: &mut PgConn,
    workspace_id: Uuid,
    policy: &WorkspacePolicy,
) -> Result<WorkspacePolicyVersion> {
    let version_id = policy
        .current_version_id
        .ok_or_else(|| Error::not_found("policy_version"))?;
    conn.find_policy_version(workspace_id, version_id)
        .await?
        .ok_or_else(|| Error::not_found("policy_version"))
}

/// The display name for a one-shot policy, e.g. `Quick redaction 1f0a3c8b9d2e`.
///
/// Derived from the content hash (distinct hashes give distinct names), so it
/// never violates the per-workspace display-name uniqueness invariant while dedup
/// keeps one row per distinct content.
fn oneshot_display_name(content_hash: &[u8]) -> String {
    format!("Quick redaction {}", hex_prefix(content_hash, 6))
}

/// Lowercase hex of the first `bytes` bytes of `content_hash`.
fn hex_prefix(content_hash: &[u8], bytes: usize) -> String {
    use std::fmt::Write;

    content_hash
        .iter()
        .take(bytes)
        .fold(String::with_capacity(bytes * 2), |mut acc, b| {
            let _ = write!(acc, "{b:02x}");
            acc
        })
}

#[cfg(test)]
mod tests {
    use nvisy_postgres::test_util::TestDatabase;

    use super::*;
    use crate::domain::input::PolicyDraftInput;
    use crate::extract::SecurityContext;

    fn security() -> SecurityContext {
        SecurityContext::default()
    }

    fn authored_request(name: &str) -> CreatePolicyInput {
        CreatePolicyInput {
            display_name: None,
            description: None,
            body: PolicyBodyInput::Inline {
                definition: Box::new(PolicyDraftInput {
                    name: name.to_owned(),
                    description: None,
                    scopes: Vec::new(),
                    rules: Vec::new(),
                    fallback: None,
                }),
            },
        }
    }

    fn oneshot_request(labels: &[&str]) -> CreatePolicyInput {
        CreatePolicyInput {
            display_name: None,
            description: None,
            body: PolicyBodyInput::Labels {
                labels: labels.iter().map(|l| (*l).to_owned()).collect(),
            },
        }
    }

    #[tokio::test]
    async fn creates_an_authored_policy() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let service = WorkspacePolicyService::new(db.client.clone());
        let origin = event::EventOrigin {
            workspace_id: seeded.workspace_id,
            account_id: seeded.account_id,
            security: &security(),
        };

        let resolved = service
            .create(origin, authored_request("audit-policy"))
            .await?;
        assert!(resolved.created);
        assert_eq!(resolved.policy.kind, PolicyKind::Authored);
        assert!(resolved.policy.content_hash.is_none());
        assert_eq!(resolved.version.version_number, 1);
        Ok(())
    }

    #[tokio::test]
    async fn one_shot_dedups_identical_content() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let service = WorkspacePolicyService::new(db.client.clone());
        let security = security();
        let origin = || event::EventOrigin {
            workspace_id: seeded.workspace_id,
            account_id: seeded.account_id,
            security: &security,
        };

        let first = service
            .create(origin(), oneshot_request(&["person_name"]))
            .await?;
        assert!(first.created);
        assert_eq!(first.policy.kind, PolicyKind::Oneshot);

        // The same labels reuse the row rather than minting a second one.
        let second = service
            .create(origin(), oneshot_request(&["person_name"]))
            .await?;
        assert!(!second.created);
        assert_eq!(second.policy.id, first.policy.id);
        assert_eq!(second.version.id, first.version.id);
        Ok(())
    }

    #[tokio::test]
    async fn editing_a_one_shot_is_rejected() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let service = WorkspacePolicyService::new(db.client.clone());
        let security = security();
        let origin = || event::EventOrigin {
            workspace_id: seeded.workspace_id,
            account_id: seeded.account_id,
            security: &security,
        };

        let oneshot = service
            .create(origin(), oneshot_request(&["person_name"]))
            .await?;
        let policy_id = oneshot.policy.id;

        // A label-only edit is rejected: a one-shot is immutable.
        let label_edit = UpdatePolicyInput {
            display_name: Some("Renamed".to_owned()),
            description: None,
            definition: None,
        };
        let err = service
            .update(origin(), policy_id, label_edit)
            .await
            .expect_err("a one-shot rejects a label edit");
        assert_eq!(err.kind(), ErrorKind::BadRequest);

        // A definition edit is rejected too.
        let definition_edit = UpdatePolicyInput {
            display_name: None,
            description: None,
            definition: Some(PolicyDraftInput {
                name: "Edited".to_owned(),
                description: None,
                scopes: Vec::new(),
                rules: Vec::new(),
                fallback: None,
            }),
        };
        let err = service
            .update(origin(), policy_id, definition_edit)
            .await
            .expect_err("a one-shot rejects a definition edit");
        assert_eq!(err.kind(), ErrorKind::BadRequest);

        // The one-shot is unchanged.
        let (found, _) = service.find(seeded.workspace_id, policy_id).await?;
        assert_eq!(found.item.kind, PolicyKind::Oneshot);
        assert!(found.item.content_hash.is_some());
        Ok(())
    }

    #[tokio::test]
    async fn editing_an_authored_policy_mints_a_version() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let service = WorkspacePolicyService::new(db.client.clone());
        let security = security();
        let origin = || event::EventOrigin {
            workspace_id: seeded.workspace_id,
            account_id: seeded.account_id,
            security: &security,
        };

        let created = service
            .create(origin(), authored_request("editable"))
            .await?;

        let request = UpdatePolicyInput {
            display_name: None,
            description: None,
            definition: Some(PolicyDraftInput {
                name: "Edited".to_owned(),
                description: None,
                scopes: Vec::new(),
                rules: Vec::new(),
                fallback: None,
            }),
        };
        let (updated, version) = service.update(origin(), created.policy.id, request).await?;
        assert_eq!(updated.item.kind, PolicyKind::Authored);
        assert_eq!(version.version_number, 2);
        Ok(())
    }
}
