//! Prefixed identifiers: type-prefixed opaque ids for API resources.

use std::fmt;
use std::str::FromStr;

use uuid::Uuid;

/// Error returned when a string is not a valid prefixed id.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PrefixedIdError {
    /// The value does not start with the expected `<prefix>_` marker.
    #[error("id must start with the '{0}_' prefix")]
    Prefix(&'static str),
    /// The portion after the prefix is not a valid UUID.
    #[error("id does not contain a valid identifier")]
    Uuid,
}

/// Declares a distinct, type-prefixed opaque id newtype wrapping a [`Uuid`].
///
/// The id renders as `<prefix>_<uuid>` at the API boundary and parses the same
/// shape back. The underlying database column remains `Uuid`; the prefix is a
/// presentation encoding only, so these types convert to and from [`Uuid`] at
/// the handler edge and are never used as a Diesel SQL type. Each invocation
/// produces its own type, so ids of different resources cannot be interchanged.
macro_rules! prefixed_id {
    ($(#[$meta:meta])* $name:ident, $prefix:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[derive(serde::Serialize, serde::Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(Uuid);

        impl $name {
            /// The textual prefix (without the trailing underscore).
            pub const PREFIX: &'static str = $prefix;

            /// Wraps a raw [`Uuid`] as this prefixed id.
            #[inline]
            #[must_use]
            pub const fn from_uuid(id: Uuid) -> Self {
                Self(id)
            }

            /// Returns the underlying [`Uuid`].
            #[inline]
            #[must_use]
            pub const fn as_uuid(&self) -> Uuid {
                self.0
            }

            /// Parses a `<prefix>_<uuid>` string into this id.
            ///
            /// # Errors
            ///
            /// Returns [`PrefixedIdError`] if the prefix is wrong or the
            /// remainder is not a valid UUID.
            pub fn parse(value: &str) -> Result<Self, PrefixedIdError> {
                let rest = value
                    .strip_prefix(concat!($prefix, "_"))
                    .ok_or(PrefixedIdError::Prefix($prefix))?;
                let id = Uuid::parse_str(rest).map_err(|_| PrefixedIdError::Uuid)?;
                Ok(Self(id))
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}_{}", $prefix, self.0)
            }
        }

        impl From<Uuid> for $name {
            #[inline]
            fn from(id: Uuid) -> Self {
                Self(id)
            }
        }

        impl From<$name> for Uuid {
            #[inline]
            fn from(id: $name) -> Self {
                id.0
            }
        }

        impl FromStr for $name {
            type Err = PrefixedIdError;

            #[inline]
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::parse(value)
            }
        }

        impl TryFrom<String> for $name {
            type Error = PrefixedIdError;

            #[inline]
            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::parse(&value)
            }
        }

        impl From<$name> for String {
            #[inline]
            fn from(id: $name) -> Self {
                id.to_string()
            }
        }

        #[cfg(feature = "schema")]
        impl schemars::JsonSchema for $name {
            fn schema_name() -> std::borrow::Cow<'static, str> {
                stringify!($name).into()
            }

            fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
                schemars::json_schema!({
                    "type": "string",
                    "pattern": concat!("^", $prefix, r"_[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$"),
                    "description": concat!("Opaque ", $prefix, " identifier (", $prefix, "_<uuid>)."),
                })
            }
        }
    };
}

prefixed_id! {
    /// Opaque identifier for a workspace connection (`conn_<uuid>`).
    ConnectionId, "conn"
}

prefixed_id! {
    /// Opaque identifier for a workspace webhook (`whk_<uuid>`).
    WebhookId, "whk"
}

prefixed_id! {
    /// Opaque identifier for a pipeline run (`run_<uuid>`).
    RunId, "run"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_string() {
        let uuid = Uuid::from_u128(0x0123_4567_89ab_cdef_0123_4567_89ab_cdef);
        let id = ConnectionId::from_uuid(uuid);
        let rendered = id.to_string();
        assert!(rendered.starts_with("conn_"));
        assert_eq!(ConnectionId::parse(&rendered).unwrap(), id);
        assert_eq!(id.as_uuid(), uuid);
    }

    #[test]
    fn rejects_wrong_prefix() {
        let uuid = Uuid::from_u128(1);
        let webhook = WebhookId::from_uuid(uuid).to_string();
        // A whk_ id must not parse as a ConnectionId.
        assert_eq!(
            ConnectionId::parse(&webhook),
            Err(PrefixedIdError::Prefix("conn"))
        );
    }

    #[test]
    fn rejects_malformed_uuid() {
        assert_eq!(
            ConnectionId::parse("conn_not-a-uuid"),
            Err(PrefixedIdError::Uuid)
        );
    }

    #[test]
    fn rejects_missing_prefix() {
        let bare = Uuid::from_u128(1).to_string();
        assert_eq!(
            ConnectionId::parse(&bare),
            Err(PrefixedIdError::Prefix("conn"))
        );
    }
}
