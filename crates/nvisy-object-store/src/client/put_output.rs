//! Result type for [`ObjectStoreClient::put`] and [`ObjectStoreClient::put_opts`].
//!
//! [`ObjectStoreClient::put`]: super::ObjectStoreClient::put
//! [`ObjectStoreClient::put_opts`]: super::ObjectStoreClient::put_opts

use object_store::PutResult;

/// Result of a successful put operation.
#[derive(Debug)]
pub struct PutOutput {
    /// Unique identifier for the newly created object, if the backend provides one.
    pub e_tag: Option<String>,
    /// A version indicator for the newly created object, if the backend provides one.
    pub version: Option<String>,
}

impl From<PutResult> for PutOutput {
    fn from(r: PutResult) -> Self {
        Self {
            e_tag: r.e_tag,
            version: r.version,
        }
    }
}
