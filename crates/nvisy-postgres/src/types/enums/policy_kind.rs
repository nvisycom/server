//! Policy kind enumeration: how a policy came to exist.

use super::db_enum;

db_enum! {
    /// The kind of a redaction policy.
    ///
    /// Corresponds to the `POLICY_KIND` `PostgreSQL` enum. An authored policy is a
    /// normal, editable policy created from a template or an inline definition. A
    /// derived policy is minted inline from a bare label list (the ad-hoc redact
    /// flow): it is content-addressed and deduplicated, and immutable once minted.
    /// A derived policy can be promoted to an authored one, which clears its
    /// content hash and makes it an editable, reusable policy selectable for
    /// detections.
    pub enum PolicyKind: Default = Authored, "crate::schema::sql_types::PolicyKind" {
        /// A normal, editable policy (template or inline definition).
        Authored = "authored",
        /// A policy derived from a label list, content-addressed and immutable.
        Derived = "derived",
    }
}
