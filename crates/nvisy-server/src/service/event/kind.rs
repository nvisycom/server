//! The event-projection contract: how one event becomes its three sink payloads.
//!
//! Every workspace event is a plain struct that owns its facts once and
//! implements [`EventKind`]. The trait is the single place an event decides how
//! it projects onto the activity log, the webhook stream, and notifications, so
//! the drainer just calls the trait rather than matching on a giant enum. The
//! sink payload types ([`ActivityPayload`], [`NotificationPayload`],
//! [`WebhookEvent`]) are the stored/wire formats and live in `nvisy-postgres`;
//! an event's job is to build them.

use nvisy_postgres::types::{ActivityPayload, NotificationPayload, WebhookEvent, WorkspaceRole};
use uuid::Uuid;

/// A webhook delivery an event raises: which webhook event fired, and an optional
/// JSON body carrying the event's display fields.
///
/// The body is built from a typed, `camelCase` struct (never an ad-hoc map), so
/// the delivered shape is defined in one place per event and cannot drift.
pub struct WebhookDelivery {
    /// The webhook event that fired.
    pub event: WebhookEvent,
    /// Extra display fields for subscribers, or `None` when the event carries no
    /// body beyond its envelope.
    pub body: Option<serde_json::Value>,
}

/// Who an in-app notification is delivered to.
///
/// Named (not a bare [`Uuid`]) so a call site can't confuse the recipient with
/// any of the other ids an event carries, and so a single event can fan out to a
/// role-based audience as easily as to one account.
pub enum NotifyTarget {
    /// One specific account, subject to its own notification preferences.
    Account(Uuid),
    /// Every member holding one of `roles`, optionally excluding one account
    /// (e.g. the actor, who need not be told of their own action).
    Roles {
        /// The roles whose holders receive the notification.
        roles: Vec<WorkspaceRole>,
        /// An account to skip, if any.
        exclude: Option<Uuid>,
    },
}

/// An in-app notification an event raises: its audience and payload.
pub struct Notification {
    /// Who receives the notification.
    pub target: NotifyTarget,
    /// The stored notification payload.
    pub payload: NotificationPayload,
}

impl Notification {
    /// A single-recipient notification list: one entry addressed to `recipient`
    /// when it is `Some`, or empty when it is `None` (e.g. an actor acting on
    /// their own resource, who is not notified). Collapses the common
    /// `notify.map(...).into_iter().collect()` at an event's notification site.
    pub fn to_account(recipient: Option<Uuid>, payload: NotificationPayload) -> Vec<Self> {
        recipient
            .map(|recipient| Self {
                target: NotifyTarget::Account(recipient),
                payload,
            })
            .into_iter()
            .collect()
    }
}

/// How one workspace event projects onto its sinks.
///
/// Implemented once per event struct. The event owns its data; each method
/// derives a sink payload from that data, so the field set is declared a single
/// time rather than re-typed per sink. `TAG` is the one place the event's stable
/// wire tag lives; the `workspace_events!` macro checks it against the enum's
/// serde rename.
pub trait EventKind {
    /// The stable dotted wire tag for this event (e.g. `file.assigned`). The
    /// outbox envelope's `type` field, asserted equal to the enum variant's serde
    /// rename by a generated test.
    const TAG: &'static str;

    /// The affected resource's id, for the webhook envelope.
    fn resource_id(&self) -> Uuid;

    /// The activity-log payload. Every event is recorded, so this is total.
    fn activity(&self) -> ActivityPayload;

    /// The webhook delivery this event raises, or `None` when the webhook
    /// vocabulary does not carry it (workspace lifecycle, invites, webhook CRUD).
    fn webhook(&self) -> Option<WebhookDelivery> {
        None
    }

    /// The in-app notifications this event raises — an empty `Vec` when it raises
    /// none, one entry for a single-recipient event, or several to fan out to
    /// distinct audiences. Consumes the event, moving its facts into the payloads.
    fn notification(self) -> Vec<Notification>
    where
        Self: Sized,
    {
        Vec::new()
    }
}
