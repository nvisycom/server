//! Database enumeration types for type-safe queries.
//!
//! This module provides strongly-typed enumerations that correspond to PostgreSQL ENUM types
//! defined in the database schema. Each enumeration provides serialization support for APIs
//! and database integration through Diesel.

// Account-related enumerations
pub mod api_token_type;
pub mod identity_provider;
pub mod notification_event;
pub mod outbox_status;

// Chat-related enumerations
pub mod chat_role;

// Connection-related enumerations
pub mod connection_type;
pub mod provider_type;

// Workspace-related enumerations
pub mod activity_type;
pub mod invite_status;
pub mod sync_deletion_policy;
pub mod sync_mode;
pub mod sync_status;
pub mod sync_trigger_type;
pub mod webhook_event;
pub mod webhook_status;
pub mod workspace_role;

// File-related enumerations
pub mod file_kind;

// Detection / pipeline-related enumerations
pub mod detection_status;
pub mod pipeline_status;
pub mod pipeline_trigger_type;

pub use activity_type::ActivityType;
pub use api_token_type::ApiTokenType;
pub use chat_role::ChatRole;
pub use connection_type::ConnectionType;
pub use detection_status::DetectionStatus;
pub use file_kind::FileKind;
pub use identity_provider::IdentityProvider;
pub use invite_status::InviteStatus;
pub use notification_event::NotificationEvent;
pub use outbox_status::OutboxStatus;
pub use pipeline_status::PipelineStatus;
pub use pipeline_trigger_type::PipelineTriggerType;
pub use provider_type::ProviderType;
pub use sync_deletion_policy::SyncDeletionPolicy;
pub use sync_mode::SyncMode;
pub use sync_status::SyncStatus;
pub use sync_trigger_type::SyncTriggerType;
pub use webhook_event::WebhookEvent;
pub use webhook_status::WebhookStatus;
pub use workspace_role::WorkspaceRole;

/// Defines a PostgreSQL-backed enum whose wire form is written once per variant.
///
/// A `DbEnum` needs the same string in three places per variant — `db_rename`
/// (the Postgres label), `serde(rename)` (the JSON tag), and `strum(serialize)`
/// (the `Display`/`FromStr`/`Into<&str>` form) — and nothing enforces that they
/// agree. This macro takes each variant's wire string once (`Variant = "wire"`)
/// and expands it into all three, so they cannot drift. The common derives, the
/// `schema` feature gate on `JsonSchema`, and doc comments are handled here;
/// per-enum methods live in a separate `impl` block as usual.
///
/// Name a default variant in the header (`enum X: Default = "sql" { … }`) to also
/// implement `Default`; omit `: Default` for an enum with no default. The default
/// variant must be one of the variants listed.
macro_rules! db_enum {
    // The shared enum definition: derives, SQL type path, and each variant with
    // its wire string fanned out to db_rename / serde / strum.
    (@define
        $(#[doc = $enum_doc:literal])*
        $vis:vis enum $name:ident = $sql_ty:literal {
            $(
                $(#[doc = $var_doc:literal])*
                $variant:ident = $wire:literal
            ),+ $(,)?
        }
    ) => {
        $(#[doc = $enum_doc])*
        #[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
        #[cfg_attr(feature = "schema", derive(::schemars::JsonSchema))]
        #[derive(
            ::serde::Serialize,
            ::serde::Deserialize,
            ::diesel_derive_enum::DbEnum,
            ::strum::Display,
            ::strum::EnumIter,
            ::strum::EnumString,
            ::strum::IntoStaticStr,
        )]
        #[ExistingTypePath = $sql_ty]
        $vis enum $name {
            $(
                $(#[doc = $var_doc])*
                #[db_rename = $wire]
                #[serde(rename = $wire)]
                #[strum(serialize = $wire)]
                $variant,
            )+
        }
    };

    // No default variant.
    (
        $(#[doc = $enum_doc:literal])*
        $vis:vis enum $name:ident = $sql_ty:literal { $($body:tt)* }
    ) => {
        $crate::types::enums::db_enum!(@define
            $(#[doc = $enum_doc])* $vis enum $name = $sql_ty { $($body)* });
    };

    // A default variant named in the header, implemented directly.
    (
        $(#[doc = $enum_doc:literal])*
        $vis:vis enum $name:ident: Default = $default:ident, $sql_ty:literal { $($body:tt)* }
    ) => {
        $crate::types::enums::db_enum!(@define
            $(#[doc = $enum_doc])* $vis enum $name = $sql_ty { $($body)* });

        impl Default for $name {
            fn default() -> Self {
                Self::$default
            }
        }
    };
}
pub(crate) use db_enum;
