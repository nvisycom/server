//! A typed `JSONB` column.
//!
//! [`Json<T>`] maps to Postgres `JSONB` and is parameterized by the value type
//! `T` it holds, so a column reads as `Json<WorkspaceSettings>` or
//! `Json<NotificationPayload>` rather than a bare `Value`. Writes go through
//! [`encode`](Json::encode); reads pick a policy for what to do with a blob that
//! does not match `T`:
//!
//! - [`strict`](Json::strict) — error on mismatch (callers that treat bad data
//!   as a failure).
//! - [`or_default`](Json::or_default) — repair to `T::default()` on mismatch
//!   (config that must stay usable across shape changes).
//! - [`typed`](Json::typed) — a fail-closed [`JsonBody<T>`] (`Known`/`Unknown`)
//!   that never drops the row (tagged payloads a list must always surface).
//!
//! The value is stored as a [`serde_json::Value`]; `T` is a compile-time marker
//! that fixes what `encode`/reads accept and yield, without the data layer
//! depending on `T`'s internals.

use std::marker::PhantomData;

use diesel::deserialize::{self, FromSql, FromSqlRow};
use diesel::expression::AsExpression;
use diesel::pg::{Pg, PgValue};
use diesel::serialize::{self, Output, ToSql};
use diesel::sql_types::Jsonb;
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::JsonBody;

/// A `JSONB` column holding a value of type `T`.
///
/// `Clone`/`Debug`/`PartialEq`/`Eq` are implemented by hand rather than derived,
/// because the derives would wrongly require `T` to implement them — the value
/// type is only a phantom marker (`PhantomData<fn() -> T>`), never stored.
#[derive(AsExpression, FromSqlRow)]
#[diesel(sql_type = Jsonb)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "schema", schemars(transparent))]
pub struct Json<T> {
    value: serde_json::Value,
    #[cfg_attr(feature = "schema", schemars(skip))]
    _marker: PhantomData<fn() -> T>,
}

impl<T> Clone for Json<T> {
    fn clone(&self) -> Self {
        Self::from_raw(self.value.clone())
    }
}

impl<T> std::fmt::Debug for Json<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Json").field(&self.value).finish()
    }
}

impl<T> PartialEq for Json<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T> Eq for Json<T> {}

impl<T> Json<T> {
    /// Wraps a raw JSON value as a column, without checking it against `T`.
    ///
    /// This is how a stored blob enters the type (mirroring `FromSql`); a value
    /// that does not match `T` is caught later by the chosen read policy
    /// (`strict`/`or_default`/`typed`), never here.
    pub fn from_raw(value: serde_json::Value) -> Self {
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

impl<T: Serialize> Json<T> {
    /// Serializes a value into the column.
    ///
    /// # Panics
    ///
    /// Panics if `T` does not serialize to JSON. This is a value-authoring
    /// invariant, not a runtime condition: these value types are plain derived
    /// `Serialize` types whose `to_value` cannot fail, so a panic here means a
    /// value type was defined with a fallible `Serialize` and must be fixed.
    pub fn encode(value: &T) -> Self {
        let value = serde_json::to_value(value).expect("value must serialize to JSON");
        Self::from_raw(value)
    }
}

impl<T: DeserializeOwned> Json<T> {
    /// Decodes the stored value into `T`, erroring on a mismatch.
    pub fn strict(&self) -> serde_json::Result<T> {
        serde_json::from_value(self.value.clone())
    }

    /// Decodes the stored value into `T`, or a fail-closed [`JsonBody<T>`] that
    /// keeps a non-matching blob as [`JsonBody::Unknown`] instead of dropping it.
    pub fn typed(&self) -> JsonBody<T> {
        JsonBody::decode(self.value.clone())
    }
}

impl<T: DeserializeOwned + Default> Json<T> {
    /// Decodes the stored value into `T`, falling back to `T::default()` when it
    /// does not match — for configuration that must stay usable across shape
    /// changes rather than error or surface a raw blob.
    pub fn or_default(&self) -> T {
        serde_json::from_value(self.value.clone()).unwrap_or_default()
    }
}

impl<T> ToSql<Jsonb, Pg> for Json<T> {
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> serialize::Result {
        <serde_json::Value as ToSql<Jsonb, Pg>>::to_sql(&self.value, &mut out.reborrow())
    }
}

impl<T> FromSql<Jsonb, Pg> for Json<T> {
    fn from_sql(bytes: PgValue<'_>) -> deserialize::Result<Self> {
        let value = <serde_json::Value as FromSql<Jsonb, Pg>>::from_sql(bytes)?;
        Ok(Self::from_raw(value))
    }
}
