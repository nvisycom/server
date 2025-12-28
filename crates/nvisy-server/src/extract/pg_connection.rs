//! PostgreSQL connection extractor for request handlers.
//!
//! This module provides the [`PgPool`] extractor that acquires a database
//! connection from the pool for use in request handlers.

use axum::extract::{FromRef, FromRequestParts};
use axum::http::request::Parts;
use derive_more::{Deref, DerefMut};
use nvisy_postgres::{PgClient, PgConn};

use crate::handler::{Error, ErrorKind};

/// Extractor that provides a database connection from the pool.
///
/// This extractor acquires a [`PgConn`] from the connection pool, which
/// implements all repository traits for database operations.
///
/// # Example
///
/// ```rust
/// use nvisy_server::extract::PgPool;
///
/// async fn get_account(PgPool(conn): PgPool) {
///     // Use conn with repository traits
/// }
/// ```
#[derive(Debug, Deref, DerefMut)]
pub struct PgPool(pub PgConn);

impl<S> FromRequestParts<S> for PgPool
where
    PgClient: FromRef<S>,
    S: Sync,
{
    type Rejection = Error<'static>;

    async fn from_request_parts(_parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let pg_client = PgClient::from_ref(state);
        let conn = pg_client.get_connection().await.map_err(|e| {
            tracing::error!(error = %e, "Failed to acquire database connection");
            ErrorKind::InternalServerError
                .with_message("Database connection unavailable")
                .with_context(e.to_string())
        })?;

        Ok(PgPool(conn))
    }
}

impl aide::OperationInput for PgPool {}
