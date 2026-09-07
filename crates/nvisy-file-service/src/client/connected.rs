//! The result of connecting a cloud file-service config.

use super::FileServiceClient;
use crate::provider::FileServiceConfig;

/// A connected client plus, when a refresh happened, the updated config the
/// caller must persist back to the connection.
pub struct ConnectedFileService {
    /// The connected provider client.
    pub client: Box<dyn FileServiceClient>,
    /// Present only when the access token was refreshed: the config carrying the
    /// new tokens, for the caller to re-encrypt and store so the next connect
    /// starts from fresh tokens.
    pub refreshed: Option<FileServiceConfig>,
}
