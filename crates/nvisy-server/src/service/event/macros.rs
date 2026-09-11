//! The `workspace_events!` macro: one table binds every event struct into the
//! outbox envelope and its trait dispatch.

/// Generates the [`WorkspaceEvent`](super::WorkspaceEvent) outbox envelope from a
/// table of `Variant => "wire.tag"` entries.
///
/// Each variant wraps the like-named event struct (which must implement
/// [`EventKind`](super::EventKind)). The macro expands to:
///
/// 1. the `WorkspaceEvent` enum, `#[serde(tag = "type", content = "data")]`, one
///    newtype variant per entry carrying its serde `rename`;
/// 2. an inherent impl forwarding `tag`, `resource_id`, `activity`, `webhook`,
///    and `notification` to the wrapped struct's [`EventKind`](super::EventKind);
/// 3. a test asserting each variant's serde rename equals the wrapped struct's
///    [`EventKind::TAG`](super::EventKind::TAG), so the tag lives once (in the
///    table) and the two can never drift.
///
/// The tag string appears once per event, in the table.
macro_rules! workspace_events {
    ($( $variant:ident => $tag:literal ),+ $(,)?) => {
        /// A workspace event: the raw facts of one action, as written to the
        /// transactional outbox and later projected onto its sinks by the drainer.
        ///
        /// The wire format is pinned: variants are tagged by an explicit, stable
        /// `type` string (not the Rust identifier), with the event's fields under
        /// `data`. An outbox row written by one build is decoded by a later one,
        /// so a variant's tag must never change.
        #[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize)]
        #[serde(tag = "type", content = "data")]
        pub enum WorkspaceEvent {
            $(
                #[serde(rename = $tag)]
                $variant($variant),
            )+
        }

        impl WorkspaceEvent {
            /// The event's stable wire tag (its `type` in the outbox envelope).
            pub fn tag(&self) -> &'static str {
                match self {
                    $( Self::$variant(_) => <$variant as $crate::service::event::EventKind>::TAG, )+
                }
            }

            /// The affected resource's id, for the webhook envelope.
            pub fn resource_id(&self) -> ::uuid::Uuid {
                match self {
                    $( Self::$variant(e) => $crate::service::event::EventKind::resource_id(e), )+
                }
            }

            /// The activity-log payload. Total: every event is recorded.
            pub fn activity(&self) -> ::nvisy_postgres::types::ActivityPayload {
                match self {
                    $( Self::$variant(e) => $crate::service::event::EventKind::activity(e), )+
                }
            }

            /// The webhook delivery this event raises, or `None` for events the
            /// webhook vocabulary does not carry.
            pub fn webhook(&self) -> Option<$crate::service::event::WebhookDelivery> {
                match self {
                    $( Self::$variant(e) => $crate::service::event::EventKind::webhook(e), )+
                }
            }

            /// The in-app notifications this event raises (empty when none).
            /// Consumes the event, moving its facts into the payloads.
            pub fn notification(self) -> Vec<$crate::service::event::Notification> {
                match self {
                    $( Self::$variant(e) => $crate::service::event::EventKind::notification(e), )+
                }
            }
        }

        #[cfg(test)]
        mod generated_tag_tests {
            use super::*;

            /// The serde rename on each variant must equal the wrapped struct's
            /// `EventKind::TAG`, so the outbox `type` and the event's own tag agree.
            #[test]
            fn variant_rename_matches_event_tag() {
                $(
                    assert_eq!(
                        variant_tag(stringify!($variant)),
                        <$variant as $crate::service::event::EventKind>::TAG,
                        concat!("tag mismatch for ", stringify!($variant)),
                    );
                )+
            }

            /// Reads the serde `rename` for a variant by finding its table entry.
            /// Kept in lockstep with the enum by the same macro expansion.
            fn variant_tag(variant: &str) -> &'static str {
                match variant {
                    $( stringify!($variant) => $tag, )+
                    other => panic!("unknown variant {other}"),
                }
            }
        }
    };
}

pub(crate) use workspace_events;

/// Generates a family of CRUD-style event structs that share a field set and
/// differ only by action (created / updated / deleted / …).
///
/// Each action `Foo` in the family expands to a `pub struct Foo { <fields> }` and
/// its [`EventKind`](super::EventKind) impl, where:
/// - `TAG` is the given per-action wire tag;
/// - `resource_id()` returns the named `id` field;
/// - `activity()` is `ActivityPayload::Foo(<activity params>)` — the payload
///   variant name matches the struct name, and the params are built by the shared
///   `activity` expression (bound to `$this`, the struct value);
/// - `webhook()` is `Some(WebhookEvent::Foo)` with no body when the family is
///   declared `webhook`, and the default (none) otherwise.
///
/// Families whose actions carry a payload body, a notification, or per-action
/// fields are written out by hand instead — this macro is only for the plain,
/// uniform CRUD families.
macro_rules! crud_events {
    (
        fields $fields:tt
        id = $id:ident;
        activity($this:ident) = $activity:expr;
        webhook = $webhook:tt;
        $(
            $(#[doc = $action_doc:literal])*
            $action:ident => $tag:literal
        ),+ $(,)?
    ) => {
        $(
            $crate::service::event::macros::crud_events! {
                @one
                $(#[doc = $action_doc])*
                $action => $tag,
                fields $fields,
                id = $id,
                activity($this) = $activity,
                webhook = $webhook,
            }
        )+
    };

    // One action struct + its EventKind impl. The field group is pasted verbatim,
    // so the per-action loop above never repeats over the fields itself.
    (
        @one
        $(#[doc = $action_doc:literal])*
        $action:ident => $tag:literal,
        fields { $( $field:ident : $field_ty:ty ),+ $(,)? },
        id = $id:ident,
        activity($this:ident) = $activity:expr,
        webhook = $webhook:tt,
    ) => {
        $(#[doc = $action_doc])*
        #[derive(Debug, Clone, ::serde::Serialize, ::serde::Deserialize)]
        pub struct $action {
            $( pub $field : $field_ty ),+
        }

        impl $crate::service::event::EventKind for $action {
            const TAG: &'static str = $tag;

            fn resource_id(&self) -> ::uuid::Uuid {
                self.$id
            }

            fn activity(&self) -> ::nvisy_postgres::types::ActivityPayload {
                let $this = self;
                ::nvisy_postgres::types::ActivityPayload::$action($activity)
            }

            $crate::service::event::macros::crud_events!(@webhook $action, $webhook);
        }
    };

    // `webhook = yes` -> emit a bodyless webhook using the like-named variant.
    (@webhook $action:ident, yes) => {
        fn webhook(&self) -> Option<$crate::service::event::WebhookDelivery> {
            Some($crate::service::event::WebhookDelivery {
                event: ::nvisy_postgres::types::WebhookEvent::$action,
                body: None,
            })
        }
    };
    // `webhook = no` -> keep the trait default (no webhook).
    (@webhook $action:ident, no) => {};
}

pub(crate) use crud_events;
