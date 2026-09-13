//! Account API-token domain outputs.

use nvisy_postgres::model::AccountApiToken;

/// A newly created API token together with its one-time signed JWT.
///
/// The JWT is available only at creation; the handler surfaces it once and it is
/// never returned again.
pub struct CreatedApiToken {
    /// The persisted token row.
    pub token: AccountApiToken,
    /// The signed JWT for the token, shown only once.
    pub jwt: String,
}
