//! Policy kind enumeration: how a policy came to exist.

use super::db_enum;

db_enum! {
    /// The kind of a redaction policy.
    ///
    /// Corresponds to the `POLICY_KIND` PostgreSQL enum. An authored policy is a
    /// normal, permanent policy created from a template or an inline definition. A
    /// one-shot policy is minted inline from a bare label list (the ad-hoc redact
    /// flow): it is content-addressed and deduplicated, hidden from the default
    /// list, and not attachable to a pipeline; promoting one makes it authored.
    pub enum PolicyKind: Default = Authored, "crate::schema::sql_types::PolicyKind" {
        /// A normal, permanent policy (template or inline definition).
        Authored = "authored",
        /// A one-shot policy minted from labels, temporary until promoted.
        Oneshot = "oneshot",
    }
}
