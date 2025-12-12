//! Error types for the Portkey AI Gateway client.

use portkey_sdk::Error as PortkeyError;

use crate::client::LlmBuilderError;
use crate::completion::{TypedChatRequestBuilderError, TypedChatResponseBuilderError};

/// Result type alias for Portkey operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur when using the Portkey AI Gateway client.
///
/// # Examples
///
/// ```
/// use nvisy_portkey::{Error, Result, LlmClient};
///
/// # async fn example() -> Result<()> {
/// let client = LlmClient::from_api_key("your-api-key")?;
/// // Use the client for API operations
/// # Ok(())
/// # }
/// ```
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// API-related errors from the Portkey service.
    #[error(transparent)]
    Api(#[from] PortkeyError),

    /// Configuration errors.
    #[error("Configuration error: {0}")]
    Config(#[from] LlmBuilderError),

    /// Typed request error.
    #[error("Typed request error: {0}")]
    TypedRequest(#[from] TypedChatRequestBuilderError),

    /// Typed response error.
    #[error("Typed response error: {0}")]
    TypedResponse(#[from] TypedChatResponseBuilderError),

    /// Invalid response structure errors.
    #[error("Invalid response: {message}")]
    InvalidResponse {
        /// Description of what's invalid
        message: String,
    },

    /// JSON serialization/deserialization errors.
    #[error("JSON serialization error: {0}")]
    JsonSerialization(#[from] serde_json::Error),
}

impl Error {
    /// Creates an invalid response error.
    ///
    /// # Examples
    ///
    /// ```
    /// # use nvisy_portkey::Error;
    /// let error = Error::invalid_response("Missing required field 'model'");
    /// assert!(matches!(error, Error::InvalidResponse { .. }));
    /// ```
    pub fn invalid_response(message: impl Into<String>) -> Self {
        Self::InvalidResponse {
            message: message.into(),
        }
    }
}
