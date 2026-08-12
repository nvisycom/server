//! Fail-closed decoding of a stored JSON payload into a typed value.
//!
//! A row that stores an event as a self-describing JSON object (its discriminator
//! folded in, e.g. `{ "notifyType": "member:invited", ... }`) can be decoded into
//! a typed, tag-tagged payload enum `P`. [`TypedBody`] makes that decode total: a
//! blob that does not match any variant of `P` is surfaced as
//! [`TypedBody::Unknown`] carrying the raw JSON, never dropped — so a list
//! endpoint can never silently disagree with a count over the same rows, and a
//! client can still fall back to a generic rendering.
//!
//! `P` lives in the consuming crate (the tagged payload enums are an API concern);
//! this type is generic over it, so the data layer stays ignorant of the concrete
//! payloads.

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;

/// A stored JSON payload decoded into its typed form, or a raw fallback.
///
/// Untagged, so a [`Known`](TypedBody::Known) value serializes exactly as `P`
/// does, and an [`Unknown`](TypedBody::Unknown) one as the raw stored object.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(untagged)]
pub enum TypedBody<P> {
    /// A recognized payload.
    Known(P),
    /// A blob that did not match any variant of `P`; surfaced raw, never dropped.
    Unknown(serde_json::Value),
}

impl<P: DeserializeOwned> TypedBody<P> {
    /// Decodes a self-describing JSON payload into `P`, or a raw fallback.
    ///
    /// The blob must already carry its discriminator (the tag `P` is tagged by),
    /// so no external type hint is needed. On a decode failure the raw blob is
    /// returned as [`Unknown`](TypedBody::Unknown) rather than dropped.
    pub fn decode(payload: serde_json::Value) -> Self {
        match serde_json::from_value::<P>(payload.clone()) {
            Ok(value) => TypedBody::Known(value),
            Err(_) => TypedBody::Unknown(payload),
        }
    }
}
