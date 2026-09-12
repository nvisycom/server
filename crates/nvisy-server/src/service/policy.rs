//! Workspace policy domain logic: create (authored or one-shot), read, list,
//! update, promote, and delete.
//!
//! Stateless, operating on a connection and the request's parsed body, factored
//! out of the handler so the policy rules — one-shot content-addressing, the
//! authored/one-shot split, promotion, and the auto-promote-on-edit invariant —
//! live in one place and can be reasoned about (and tested) independently of the
//! HTTP flow.

use elide_pipeline::governance::policy::Policy;
use nvisy_postgres::model::{NewWorkspacePolicy, WorkspacePolicy, WorkspacePolicyVersion};
use nvisy_postgres::query::{
    PolicyCursor, WorkspacePolicyRepository, WorkspacePolicyVersionRepository,
};
use nvisy_postgres::types::{CursorPage, CursorPagination, Handle, PolicyKind, WithAccountRef};
use nvisy_postgres::{AsyncConnection, PgConn, model};
use uuid::Uuid;

use crate::handler::request::{CreateWorkspacePolicy, PolicyBody, UpdateWorkspacePolicy};
use crate::response::{Error, ErrorKind, Result};
use crate::service::event;
use crate::service::event::EventEmitter;

/// Tracing target for policy domain operations.
const TRACING_TARGET: &str = "nvisy_server::service::policy";

/// A policy paired with its current version, and whether this call created it.
///
/// `created` distinguishes a freshly minted policy from a reused one-shot, so the
/// handler can answer `201` on creation and `200` on reuse.
pub struct ResolvedPolicy {
    /// The policy row.
    pub policy: WorkspacePolicy,
    /// The current version whose definition the engine consumes.
    pub version: WorkspacePolicyVersion,
    /// Whether this call created the policy (vs. reusing an existing one-shot).
    pub created: bool,
}

/// Creates, reads, updates, promotes, and deletes workspace policies.
///
/// Stateless: every method takes the connection to act on. Resolved per request
/// from [`ServiceState`](crate::service::ServiceState).
#[derive(Clone, Copy, Default)]
pub struct PolicyService;

impl PolicyService {
    /// Creates a policy from a create request.
    ///
    /// A labels body is content-addressed: it mints — or reuses an identical live
    /// one — a one-shot policy with a hash-derived slug and name (`created` is
    /// false on reuse). A template or inline body is a permanent authored policy
    /// the caller names. The policy, its first version, and the creation event
    /// commit together.
    pub async fn create(
        &self,
        conn: &mut PgConn,
        origin: event::EventOrigin<'_>,
        request: CreateWorkspacePolicy,
    ) -> Result<ResolvedPolicy> {
        match request.body.oneshot_content_hash() {
            Some(content_hash) => {
                self.create_oneshot(conn, origin, request.body, content_hash)
                    .await
            }
            None => self.create_authored(conn, origin, request).await,
        }
    }

    /// Creates an authored (template or inline) policy: a new permanent row with a
    /// caller-supplied slug, its first version, and a creation event, in one
    /// transaction.
    async fn create_authored(
        &self,
        conn: &mut PgConn,
        origin: event::EventOrigin<'_>,
        request: CreateWorkspacePolicy,
    ) -> Result<ResolvedPolicy> {
        let slug = request.slug.ok_or_else(|| {
            ErrorKind::BadRequest.with_message("A slug is required for this policy body")
        })?;

        let definition = request.body.into_definition("");
        let display_name = request
            .display_name
            .unwrap_or_else(|| definition.name.to_string());
        let description = request
            .description
            .or_else(|| definition.description.clone().map(Into::into));
        let body = definition_to_json(&definition)?;

        let new_policy = NewWorkspacePolicy {
            workspace_id: origin.workspace_id,
            account_id: origin.account_id,
            slug,
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
                        policy_slug: created.policy.slug.clone(),
                    }),
                )
                .await?;
                Ok::<_, Error>(created)
            })
            .await?;

        tracing::info!(target: TRACING_TARGET, policy_slug = %created.policy.slug, "Policy created");
        Ok(ResolvedPolicy {
            policy: created.policy,
            version: created.version,
            created: true,
        })
    }

    /// Creates or reuses a one-shot policy from a labels body, content-addressed by
    /// `content_hash`: an identical live one-shot is reused rather than duplicated,
    /// and a fresh one is created with a hash-derived slug and name plus a creation
    /// event, all in one transaction.
    async fn create_oneshot(
        &self,
        conn: &mut PgConn,
        origin: event::EventOrigin<'_>,
        body: PolicyBody,
        content_hash: Vec<u8>,
    ) -> Result<ResolvedPolicy> {
        let slug = Handle::parse(oneshot_slug(&content_hash)).map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Failed to generate a one-shot policy slug")
                .with_context(err.to_string())
        })?;
        let display_name = oneshot_display_name(&content_hash);

        let definition = body.into_definition(&display_name);
        let definition_json = definition_to_json(&definition)?;

        let new_policy = NewWorkspacePolicy {
            workspace_id: origin.workspace_id,
            account_id: origin.account_id,
            slug,
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
                            policy_slug: resolved.policy.policy.slug.clone(),
                        }),
                    )
                    .await?;
                }
                Ok::<_, Error>(resolved)
            })
            .await?;

        if resolved.created {
            tracing::info!(target: TRACING_TARGET, policy_slug = %resolved.policy.policy.slug, "One-shot policy created");
        }

        Ok(ResolvedPolicy {
            policy: resolved.policy.policy,
            version: resolved.policy.version,
            created: resolved.created,
        })
    }

    /// Lists a workspace's policies, newest first. One-shot policies are excluded.
    pub async fn list(
        &self,
        conn: &mut PgConn,
        workspace_id: Uuid,
        pagination: CursorPagination<PolicyCursor>,
    ) -> Result<CursorPage<WithAccountRef<WorkspacePolicy>>> {
        Ok(conn
            .cursor_list_workspace_policies(workspace_id, pagination)
            .await?)
    }

    /// Finds a policy by slug with its creator and current version, or a NotFound.
    pub async fn find(
        &self,
        conn: &mut PgConn,
        workspace_id: Uuid,
        policy_slug: &str,
    ) -> Result<(WithAccountRef<WorkspacePolicy>, WorkspacePolicyVersion)> {
        let found = find_policy(conn, workspace_id, policy_slug).await?;
        let version = current_version(conn, workspace_id, &found.item).await?;
        Ok((found, version))
    }

    /// Updates a policy, returning it with its current version and creator.
    ///
    /// A definition change mints a new version; a label-only change mutates the
    /// logical row in place. Editing a one-shot's definition promotes it (clears
    /// the dedup hash, making it authored), since its content no longer matches its
    /// content-address; a label-only edit leaves a one-shot as it is. The write and
    /// its event commit together.
    pub async fn update(
        &self,
        conn: &mut PgConn,
        origin: event::EventOrigin<'_>,
        policy_slug: &str,
        request: UpdateWorkspacePolicy,
    ) -> Result<(WithAccountRef<WorkspacePolicy>, WorkspacePolicyVersion)> {
        let existing = find_policy(conn, origin.workspace_id, policy_slug)
            .await?
            .item;
        let current = current_version(conn, origin.workspace_id, &existing).await?;

        // A replaced body keeps the policy's server-owned template origin: the
        // caller authored new rules, but where the policy came from is provenance
        // the client cannot set or clear. Carry the current version's origin forward.
        let new_definition = match request.definition {
            Some(draft) => {
                let template = serde_json::from_value::<Policy>(current.definition)
                    .map_err(malformed_definition)?
                    .template;
                let definition = draft.into_definition(template);
                Some(definition_to_json(&definition)?)
            }
            None => None,
        };

        let policy_id = existing.id;
        let event_slug = existing.slug.clone();

        // A definition edit on a one-shot promotes it to authored: its content no
        // longer matches its content-address, so it leaves the dedup set. A
        // label-only edit leaves a one-shot as it is.
        let promote = existing.kind == PolicyKind::Oneshot && new_definition.is_some();

        let account_id = origin.account_id;
        let workspace_id = origin.workspace_id;
        conn.transaction(async |conn| {
            if let Some(definition) = new_definition {
                conn.create_policy_version(workspace_id, policy_id, account_id, definition, None)
                    .await?;
            }
            // Promote conditionally so the Oneshot -> Authored transition is atomic
            // and emits PolicyPromoted only when this call is the one that flips it;
            // the field write below carries the label edits either way.
            let promoted = promote && conn.promote_policy_to_authored(policy_id).await?;
            conn.update_workspace_policy(
                policy_id,
                model::UpdateWorkspacePolicy {
                    display_name: request.display_name,
                    description: request.description,
                    ..Default::default()
                },
            )
            .await?;
            conn.emit_event(
                origin,
                event::WorkspaceEvent::PolicyUpdated(event::PolicyUpdated {
                    policy_id,
                    policy_slug: event_slug.clone(),
                }),
            )
            .await?;
            if promoted {
                conn.emit_event(
                    origin,
                    event::WorkspaceEvent::PolicyPromoted(event::PolicyPromoted {
                        policy_id,
                        policy_slug: event_slug,
                    }),
                )
                .await?;
            }
            Ok::<(), Error>(())
        })
        .await?;

        tracing::info!(target: TRACING_TARGET, "Policy updated");
        self.find(conn, workspace_id, policy_slug).await
    }

    /// Promotes a one-shot policy to authored, clearing its dedup hash, returning
    /// it with its current version and creator.
    ///
    /// A no-op on an already-authored policy: it is returned unchanged, writing
    /// nothing and recording no event.
    pub async fn promote(
        &self,
        conn: &mut PgConn,
        origin: event::EventOrigin<'_>,
        policy_slug: &str,
    ) -> Result<(WithAccountRef<WorkspacePolicy>, WorkspacePolicyVersion)> {
        let found = find_policy(conn, origin.workspace_id, policy_slug).await?;
        let policy_id = found.item.id;
        let policy_slug_owned = found.item.slug.clone();

        // Promote conditionally inside the transaction so the Oneshot -> Authored
        // transition is atomic: an already-authored policy or a concurrent
        // promotion changes no row and emits no event.
        conn.transaction(async |conn| {
            if conn.promote_policy_to_authored(policy_id).await? {
                conn.emit_event(
                    origin,
                    event::WorkspaceEvent::PolicyPromoted(event::PolicyPromoted {
                        policy_id,
                        policy_slug: policy_slug_owned,
                    }),
                )
                .await?;
            }
            Ok::<(), Error>(())
        })
        .await?;

        tracing::info!(target: TRACING_TARGET, "Policy promoted");
        self.find(conn, origin.workspace_id, policy_slug).await
    }

    /// Soft-deletes a policy from its workspace, recording the event atomically.
    pub async fn delete(
        &self,
        conn: &mut PgConn,
        origin: event::EventOrigin<'_>,
        policy_slug: &str,
    ) -> Result<()> {
        let existing = find_policy(conn, origin.workspace_id, policy_slug)
            .await?
            .item;
        let policy_id = existing.id;
        let policy_slug = existing.slug.clone();

        conn.transaction(async |conn| {
            conn.delete_workspace_policy(policy_id).await?;
            conn.emit_event(
                origin,
                event::WorkspaceEvent::PolicyDeleted(event::PolicyDeleted {
                    policy_id,
                    policy_slug,
                }),
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
fn malformed_definition(err: serde_json::Error) -> Error<'static> {
    ErrorKind::InternalServerError
        .with_message("Stored policy definition is malformed")
        .with_context(err.to_string())
}

/// Finds a policy within a workspace by slug, with its creator, or a NotFound.
async fn find_policy(
    conn: &mut PgConn,
    workspace_id: Uuid,
    policy_slug: &str,
) -> Result<WithAccountRef<WorkspacePolicy>> {
    conn.find_policy_in_workspace_by_slug(workspace_id, policy_slug)
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

/// The slug for a one-shot policy, e.g. `oneshot-1f0a3c8b9d2e`.
///
/// Derived from the content hash, so the same one-shot always maps to the same
/// slug and dedup reuses its row rather than colliding. The 12-hex prefix of the
/// hash satisfies the slug format (lowercase alphanumeric with single internal
/// dashes) and length (3-32).
fn oneshot_slug(content_hash: &[u8]) -> String {
    format!("oneshot-{}", hex_prefix(content_hash, 6))
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
    content_hash
        .iter()
        .take(bytes)
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use nvisy_postgres::test_util::TestDatabase;
    use nvisy_postgres::types::Handle;

    use super::*;
    use crate::extract::SecurityContext;
    use crate::handler::request::{CreateWorkspacePolicy, PolicyBody, PolicyDraft};

    fn security() -> SecurityContext {
        SecurityContext::default()
    }

    fn authored_request(slug: &str) -> CreateWorkspacePolicy {
        CreateWorkspacePolicy {
            display_name: None,
            slug: Some(Handle::parse(slug.to_owned()).expect("valid slug")),
            description: None,
            body: PolicyBody::Inline {
                definition: Box::new(PolicyDraft {
                    name: "Test policy".to_owned(),
                    description: None,
                    scopes: Vec::new(),
                    rules: Vec::new(),
                    fallback: None,
                }),
            },
        }
    }

    fn oneshot_request(labels: &[&str]) -> CreateWorkspacePolicy {
        CreateWorkspacePolicy {
            display_name: None,
            slug: None,
            description: None,
            body: PolicyBody::Labels {
                labels: labels.iter().map(|l| (*l).to_owned()).collect(),
                action: Default::default(),
            },
        }
    }

    #[tokio::test]
    async fn creates_an_authored_policy() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;
        let origin = event::EventOrigin {
            workspace_id: seeded.workspace_id,
            account_id: seeded.account_id,
            security: &security(),
        };

        let resolved = PolicyService
            .create(&mut conn, origin, authored_request("audit-policy"))
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
        let mut conn = db.client.get_connection().await?;
        let security = security();
        let origin = || event::EventOrigin {
            workspace_id: seeded.workspace_id,
            account_id: seeded.account_id,
            security: &security,
        };

        let first = PolicyService
            .create(&mut conn, origin(), oneshot_request(&["person_name"]))
            .await?;
        assert!(first.created);
        assert_eq!(first.policy.kind, PolicyKind::Oneshot);

        // The same labels reuse the row rather than minting a second one.
        let second = PolicyService
            .create(&mut conn, origin(), oneshot_request(&["person_name"]))
            .await?;
        assert!(!second.created);
        assert_eq!(second.policy.id, first.policy.id);
        assert_eq!(second.version.id, first.version.id);
        Ok(())
    }

    #[tokio::test]
    async fn promote_is_a_no_op_on_an_authored_policy() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;
        let security = security();
        let origin = || event::EventOrigin {
            workspace_id: seeded.workspace_id,
            account_id: seeded.account_id,
            security: &security,
        };

        let created = PolicyService
            .create(&mut conn, origin(), authored_request("keep-me"))
            .await?;

        let (promoted, _) = PolicyService
            .promote(&mut conn, origin(), "keep-me")
            .await?;
        assert_eq!(promoted.item.kind, PolicyKind::Authored);
        assert_eq!(promoted.item.id, created.policy.id);
        Ok(())
    }

    #[tokio::test]
    async fn promoting_a_one_shot_clears_its_hash() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;
        let security = security();
        let origin = || event::EventOrigin {
            workspace_id: seeded.workspace_id,
            account_id: seeded.account_id,
            security: &security,
        };

        let oneshot = PolicyService
            .create(&mut conn, origin(), oneshot_request(&["email_address"]))
            .await?;

        let (promoted, _) = PolicyService
            .promote(&mut conn, origin(), oneshot.policy.slug.as_str())
            .await?;
        assert_eq!(promoted.item.kind, PolicyKind::Authored);
        assert!(promoted.item.content_hash.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn label_only_edit_does_not_promote_a_one_shot() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;
        let security = security();
        let origin = || event::EventOrigin {
            workspace_id: seeded.workspace_id,
            account_id: seeded.account_id,
            security: &security,
        };

        let oneshot = PolicyService
            .create(&mut conn, origin(), oneshot_request(&["person_name"]))
            .await?;
        let slug = oneshot.policy.slug.as_str().to_owned();

        let request = UpdateWorkspacePolicy {
            display_name: Some("Renamed".to_owned()),
            description: None,
            definition: None,
        };
        let (updated, _) = PolicyService
            .update(&mut conn, origin(), &slug, request)
            .await?;
        assert_eq!(updated.item.kind, PolicyKind::Oneshot);
        assert!(updated.item.content_hash.is_some());
        assert_eq!(updated.item.display_name, "Renamed");
        Ok(())
    }

    #[tokio::test]
    async fn definition_edit_promotes_a_one_shot() -> anyhow::Result<()> {
        let db = TestDatabase::start().await;
        let seeded = db.seed_account_and_workspace().await;
        let mut conn = db.client.get_connection().await?;
        let security = security();
        let origin = || event::EventOrigin {
            workspace_id: seeded.workspace_id,
            account_id: seeded.account_id,
            security: &security,
        };

        let oneshot = PolicyService
            .create(&mut conn, origin(), oneshot_request(&["person_name"]))
            .await?;
        let slug = oneshot.policy.slug.as_str().to_owned();

        let request = UpdateWorkspacePolicy {
            display_name: None,
            description: None,
            definition: Some(PolicyDraft {
                name: "Edited".to_owned(),
                description: None,
                scopes: Vec::new(),
                rules: Vec::new(),
                fallback: None,
            }),
        };
        let (updated, version) = PolicyService
            .update(&mut conn, origin(), &slug, request)
            .await?;
        assert_eq!(updated.item.kind, PolicyKind::Authored);
        assert!(updated.item.content_hash.is_none());
        assert_eq!(version.version_number, 2);
        Ok(())
    }
}
