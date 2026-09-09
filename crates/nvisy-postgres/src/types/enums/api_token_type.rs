//! API token type enumeration for authentication tracking.

use super::db_enum;

db_enum! {
    /// The type of API token, for authentication and tracking.
    ///
    /// Corresponds to the `API_TOKEN_TYPE` PostgreSQL enum and categorizes tokens
    /// by the client type they authenticate.
    pub enum ApiTokenType: Default = Web, "crate::schema::sql_types::ApiTokenType" {
        /// Web browser token (desktop or mobile browser).
        Web = "web",
        /// API client token (programmatic access).
        Api = "api",
        /// Native app session token (obtained by interactive desktop login via
        /// the external browser + deep-link flow, sent as a Bearer token).
        App = "app",
    }
}
