//! A `JSONB` column that stores a self-describing, typed payload.
//!
//! [`TypedJson<P>`] maps to Postgres `JSONB` and is parameterized by the payload
//! type `P` it holds, so a column reads as `TypedJson<NotificationPayload>` rather
//! than a bare `Value`. It owns the symmetric encode/decode round-trip:
//!
//! - [`encode`](TypedJson::encode) serializes a `P` (an internally-tagged enum)
//!   into the column, tag included, so the stored object is self-describing.
//! - [`decode`](TypedJson::decode) reads it back into a [`TypedBody<P>`], falling
//!   back to the raw value when it does not match any variant.
//!
//! The value is stored as a [`serde_json::Value`]; `P` is a compile-time marker
//! that fixes what `encode`/`decode` accept and yield, without the data layer
//! depending on `P`'s internals.

use std::marker::PhantomData;

use diesel::deserialize::{self, FromSql, FromSqlRow};
use diesel::expression::AsExpression;
use diesel::pg::{Pg, PgValue};
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::Jsonb;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::TypedBody;

/// A `JSONB` column holding a self-describing tagged payload of type `P`.
///
/// `Clone`/`Debug`/`PartialEq`/`Eq` are implemented by hand rather than derived,
/// because the derives would wrongly require `P` to implement them — the payload
/// type is only a phantom marker (`PhantomData<fn() -> P>`), never stored.
#[derive(AsExpression, FromSqlRow)]
#[diesel(sql_type = Jsonb)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "schema", schemars(transparent))]
pub struct TypedJson<P> {
    value: serde_json::Value,
    #[cfg_attr(feature = "schema", schemars(skip))]
    _marker: PhantomData<fn() -> P>,
}

impl<P> Clone for TypedJson<P> {
    fn clone(&self) -> Self {
        Self::from_value(self.value.clone())
    }
}

impl<P> std::fmt::Debug for TypedJson<P> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("TypedJson").field(&self.value).finish()
    }
}

impl<P> PartialEq for TypedJson<P> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<P> Eq for TypedJson<P> {}

impl<P> TypedJson<P> {
    /// Wraps a raw JSON value without checking it against `P`.
    fn from_value(value: serde_json::Value) -> Self {
        Self {
            value,
            _marker: PhantomData,
        }
    }

    /// The raw stored value.
    pub fn as_value(&self) -> &serde_json::Value {
        &self.value
    }

    /// Whether the stored object is non-empty.
    pub fn is_empty(&self) -> bool {
        self.value.as_object().is_none_or(|obj| obj.is_empty())
    }
}

impl<P: Serialize> TypedJson<P> {
    /// Serializes a payload into the column, discriminator included.
    ///
    /// # Panics
    ///
    /// Panics if `P` does not serialize to JSON. This is a payload-authoring
    /// invariant, not a runtime condition: the payload enums are plain derived
    /// `Serialize` types whose `to_value` cannot fail, so a panic here means a
    /// payload type was defined with a fallible `Serialize` and must be fixed.
    pub fn encode(payload: &P) -> Self {
        let value = serde_json::to_value(payload).expect("payload must serialize to JSON");
        Self::from_value(value)
    }
}

impl<P: DeserializeOwned> TypedJson<P> {
    /// Decodes the stored value into `P`, or a raw [`TypedBody::Unknown`] fallback
    /// when it does not match any variant. Never fails, never drops.
    pub fn decode(&self) -> TypedBody<P> {
        TypedBody::decode(self.value.clone())
    }
}

impl<P> ToSql<Jsonb, Pg> for TypedJson<P> {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        <serde_json::Value as ToSql<Jsonb, Pg>>::to_sql(&self.value, &mut out.reborrow())
    }
}

impl<P> FromSql<Jsonb, Pg> for TypedJson<P> {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        let value = <serde_json::Value as FromSql<Jsonb, Pg>>::from_sql(bytes)?;
        Ok(Self::from_value(value))
    }
}
