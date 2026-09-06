//! Writing refreshed cloud-file OAuth tokens back to a connection's stored config.

use nvisy_file_service::oauth::OAuthTokens;
use nvisy_postgres::model::UpdateWorkspaceConnection;
use nvisy_postgres::query::WorkspaceConnectionRepository;
use nvisy_postgres::{AsyncConnection, PgConn};
use uuid::Uuid;

use crate::handler::Result;
use crate::service::{ConnectionConfig, CryptoService};

/// Persists refreshed OAuth tokens, merging them onto the connection's *current*
/// stored config rather than a caller-held snapshot.
///
/// Writing a whole snapshot back would clobber any edit made since it was read
/// (a root-folder change, or a re-authorization). This re-reads the row, applies
/// only the new tokens, and re-encrypts within one transaction, so a concurrent
/// config edit survives. A connection deleted or switched to a non-cloud
/// provider in the meantime is left untouched.
pub async fn persist_refreshed_tokens(
    conn: &mut PgConn,
    crypto: &CryptoService,
    workspace_id: Uuid,
    connection_id: Uuid,
    new_tokens: OAuthTokens,
) -> Result<()> {
    let crypto = crypto.clone();
    conn.transaction(async move |conn| {
        // Lock the row for the transaction so a concurrent config replace cannot
        // overwrite this token update from a stale read, and vice versa.
        let Some(current) = conn
            .find_workspace_connection_by_id_for_update(connection_id)
            .await?
        else {
            return Ok(());
        };
        let mut config: ConnectionConfig =
            crypto.decrypt_json(workspace_id, &current.encrypted_data)?;
        let ConnectionConfig::CloudFiles(cloud) = &mut config else {
            return Ok(());
        };
        cloud.set_tokens(new_tokens);
        let encrypted_data = crypto.encrypt_json(workspace_id, &config)?;
        let update = UpdateWorkspaceConnection {
            encrypted_data: Some(encrypted_data),
            ..Default::default()
        };
        conn.update_workspace_connection(connection_id, update)
            .await?;
        Ok::<_, crate::handler::Error>(())
    })
    .await?;
    Ok(())
}
