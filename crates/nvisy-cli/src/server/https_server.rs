//! HTTPS server implementation using enhanced lifecycle management.

use std::net::SocketAddr;
use std::path::Path;

use axum::Router;
use axum_server::tls_rustls::RustlsConfig;

use crate::TRACING_TARGET_SERVER_STARTUP;
use crate::config::ServerConfig;
use crate::server::lifecycle::serve_with_shutdown;
use crate::server::{ServerError, ServerResult, shutdown_signal};

/// Starts an HTTPS server with enhanced lifecycle management.
#[allow(dead_code)]
pub async fn serve_https(
    app: Router,
    server_config: ServerConfig,
    cert_path: impl AsRef<Path>,
    key_path: impl AsRef<Path>,
) -> ServerResult<()> {
    let server_addr = server_config.socket_addr();
    let shutdown_timeout = server_config.shutdown_timeout();
    let cert_path = cert_path.as_ref();
    let key_path = key_path.as_ref();

    // Pre-validate TLS files before starting lifecycle
    validate_tls_files(cert_path, key_path)?;

    serve_with_shutdown(&server_config, move || async move {
        let tls_config = RustlsConfig::from_pem_file(cert_path, key_path)
            .await
            .map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("Failed to load TLS certificates: {e}"),
                )
            })?;

        tracing::info!(
            target: TRACING_TARGET_SERVER_STARTUP,
            cert_path = %cert_path.display(),
            key_path = %key_path.display(),
            "TLS certificates loaded successfully"
        );

        tracing::info!(
            target: TRACING_TARGET_SERVER_STARTUP,
            addr = %server_addr,
            "HTTPS server bound and ready"
        );

        let handle = axum_server::Handle::new();
        let shutdown_handle = handle.clone();

        tokio::spawn(async move {
            shutdown_signal(shutdown_timeout).await;
            shutdown_handle.graceful_shutdown(Some(shutdown_timeout));
        });

        axum_server::bind_rustls(server_addr, tls_config)
            .handle(handle)
            .serve(app.into_make_service_with_connect_info::<SocketAddr>())
            .await
    })
    .await
}

fn validate_tls_files(cert_path: &Path, key_path: &Path) -> ServerResult<()> {
    let validate_file = |path: &Path, file_type: &str| -> ServerResult<()> {
        if !path.exists() {
            return Err(ServerError::TlsCertificate(format!(
                "{} file does not exist: {}",
                file_type,
                path.display()
            )));
        }

        if !path.is_file() {
            return Err(ServerError::TlsCertificate(format!(
                "{} path is not a file: {}",
                file_type,
                path.display()
            )));
        }

        let metadata = std::fs::metadata(path).map_err(|err| {
            ServerError::TlsCertificate(format!(
                "Cannot read {} file {}: {}",
                file_type,
                path.display(),
                err
            ))
        })?;

        if metadata.len() == 0 {
            return Err(ServerError::TlsCertificate(format!(
                "{} file is empty: {}",
                file_type,
                path.display()
            )));
        }

        Ok(())
    };

    validate_file(cert_path, "Certificate")?;
    validate_file(key_path, "Private key")?;

    tracing::debug!(
        target: TRACING_TARGET_SERVER_STARTUP,
        cert_path = %cert_path.display(),
        key_path = %key_path.display(),
        "TLS files validated successfully"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_tls_files_rejects_nonexistent_files() {
        let cert_path = Path::new("nonexistent_cert.pem");
        let key_path = Path::new("nonexistent_key.pem");

        let result = validate_tls_files(cert_path, key_path);
        assert!(result.is_err());

        if let Err(ServerError::TlsCertificate(msg)) = result {
            assert!(msg.contains("Certificate file does not exist"));
        } else {
            panic!("Expected TlsCertificate error");
        }
    }
}
