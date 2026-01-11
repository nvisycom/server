//! HTTPS server implementation using enhanced lifecycle management.

use std::io;
use std::path::Path;

use axum::Router;
use axum_server::tls_rustls::RustlsConfig;
use nvisy_server::extract::AppConnectInfo;

use super::TRACING_TARGET_STARTUP;
use crate::config::ServerConfig;
use crate::server::lifecycle::serve_with_shutdown;
use crate::server::shutdown_signal;

/// Starts an HTTPS server with enhanced lifecycle management.
pub async fn serve_https(app: Router, server_config: ServerConfig) -> io::Result<()> {
    let server_addr = server_config.socket_addr();
    let shutdown_timeout = server_config.shutdown_timeout();
    let cert_path = &server_config.tls_cert_path;
    let key_path = &server_config.tls_key_path;

    // Pre-validate TLS files before starting lifecycle
    validate_tls_files(cert_path, key_path)?;

    serve_with_shutdown(&server_config, move || async move {
        let tls_config = RustlsConfig::from_pem_file(cert_path, key_path)
            .await
            .map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Failed to load TLS certificates: {e}"),
                )
            })?;

        tracing::debug!(
            target: TRACING_TARGET_STARTUP,
            cert_path = %cert_path.display(),
            key_path = %key_path.display(),
            "TLS certificates loaded"
        );

        tracing::info!(
            target: TRACING_TARGET_STARTUP,
            addr = %server_addr,
            tls = true,
            "Server listening"
        );

        let handle = axum_server::Handle::new();
        let shutdown_handle = handle.clone();

        tokio::spawn(async move {
            shutdown_signal(shutdown_timeout).await;
            shutdown_handle.graceful_shutdown(Some(shutdown_timeout));
        });

        axum_server::bind_rustls(server_addr, tls_config)
            .handle(handle)
            .serve(app.into_make_service_with_connect_info::<AppConnectInfo>())
            .await
    })
    .await
}

fn validate_tls_files(cert_path: &Path, key_path: &Path) -> io::Result<()> {
    let validate_file = |path: &Path, file_type: &str| -> io::Result<()> {
        if !path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{} file does not exist: {}", file_type, path.display()),
            ));
        }

        if !path.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{} path is not a file: {}", file_type, path.display()),
            ));
        }

        let metadata = std::fs::metadata(path)?;

        if metadata.len() == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{} file is empty: {}", file_type, path.display()),
            ));
        }

        Ok(())
    };

    validate_file(cert_path, "Certificate")?;
    validate_file(key_path, "Private key")?;

    tracing::debug!(
        target: TRACING_TARGET_STARTUP,
        cert_path = %cert_path.display(),
        key_path = %key_path.display(),
        "TLS files validated"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn validate_tls_files_rejects_nonexistent_files() {
        let cert_path = Path::new("nonexistent_cert.pem");
        let key_path = Path::new("nonexistent_key.pem");

        let result = validate_tls_files(cert_path, key_path);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
        assert!(err.to_string().contains("Certificate file does not exist"));
    }
}
