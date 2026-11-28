//! Building a [`BlobStore`] from configuration.

use std::time::Duration;

use nvisy_object_store::client::ObjectStoreClient;
use object_store::ClientOptions;
use object_store::aws::{AmazonS3Builder, AmazonS3ConfigKey};
use url::Url;

use super::{BlobStore, S3Config};
use crate::error::{Error, Result};

/// Tracing target for connection setup.
const TRACING_TARGET: &str = "nvisy_s3::connect";

/// How long a response body read may wait for the next bytes before failing.
const READ_TIMEOUT: Duration = Duration::from_secs(30);

impl BlobStore {
    /// Builds a [`BlobStore`] for the S3-compatible service described by
    /// `config`.
    ///
    /// When both `access_key_id` and `secret_access_key` are set, those static
    /// credentials are used. Otherwise credentials come from the environment
    /// chain: the `AWS_*` variables, then web identity (EKS), ECS container
    /// credentials, and EC2 instance metadata; shared profile files (`~/.aws`)
    /// are not read. `endpoint` targets an S3-compatible server, with plaintext
    /// HTTP allowed only when it is an `http://` URL; unset targets AWS S3.
    ///
    /// This does not contact the service. Call [`ping`](Self::ping) to verify
    /// endpoint reachability, credentials, and bucket access.
    ///
    /// # Errors
    ///
    /// `Config` if the credentials are incomplete (only one of the access key id
    /// and secret access key is set) or the client cannot be built.
    #[tracing::instrument(name = "s3.connect", skip_all, fields(bucket = %config.bucket))]
    pub fn connect(config: &S3Config) -> Result<Self> {
        tracing::debug!(
            target: TRACING_TARGET,
            endpoint = ?config.endpoint,
            region = %config.region,
            path_style = config.force_path_style,
            "Building the blob store client",
        );

        // Static credentials stand alone; only without them does the builder read
        // the `AWS_*` environment, so an unrelated session token or signing
        // override there never mixes into a configured key pair.
        let mut builder = match (&config.access_key_id, &config.secret_access_key) {
            (Some(access_key_id), Some(secret_access_key)) => AmazonS3Builder::new()
                .with_access_key_id(access_key_id.expose())
                .with_secret_access_key(secret_access_key.expose()),
            (None, None) => AmazonS3Builder::from_env(),
            _ => {
                return Err(Error::Config(
                    "S3 credentials incomplete: set both an access key id and a secret access key, \
                     or neither to use the environment credential chain"
                        .to_owned(),
                ));
            }
        };

        builder = builder
            .with_bucket_name(&config.bucket)
            .with_region(&config.region)
            // S3-compatible servers address as `endpoint/bucket/key`; AWS S3 uses
            // `bucket.endpoint/key`.
            .with_virtual_hosted_style_request(!config.force_path_style)
            // Each call deletes one key, so plain `DeleteObject` suffices and
            // works on servers without the bulk `DeleteObjects` API.
            .with_disable_bulk_delete(true)
            // A streamed download is read at the consumer's pace, so no
            // whole-request timeout; connecting is bounded, and a body that
            // stops sending data fails after the read timeout.
            .with_client_options(
                ClientOptions::new()
                    .with_timeout_disabled()
                    .with_read_timeout(READ_TIMEOUT),
            );

        if let Some(endpoint) = &config.endpoint {
            let endpoint = if config.force_path_style {
                endpoint.clone()
            } else {
                virtual_hosted_endpoint(endpoint, &config.bucket)?
            };
            // The S3-specific endpoint key outranks the generic one, so setting it
            // keeps an `AWS_ENDPOINT_URL_S3` read by `from_env` from overriding
            // the configured endpoint.
            builder = builder
                .with_allow_http(endpoint.starts_with("http://"))
                .with_config(AmazonS3ConfigKey::S3Endpoint, endpoint);
        }

        let store = builder
            .build()
            .map_err(|err| Error::Config(err.to_string()))?;
        Ok(Self::new(ObjectStoreClient::new(store)))
    }
}

/// Puts `bucket` in front of `endpoint`'s host for virtual-hosted-style
/// addressing, which `object_store` expects the endpoint to carry already.
fn virtual_hosted_endpoint(endpoint: &str, bucket: &str) -> Result<String> {
    let mut url = Url::parse(endpoint)
        .map_err(|err| Error::Config(format!("invalid S3 endpoint {endpoint:?}: {err}")))?;
    let host = url
        .host_str()
        .ok_or_else(|| Error::Config(format!("S3 endpoint {endpoint:?} has no host")))?;
    let host = format!("{bucket}.{host}");
    url.set_host(Some(&host))
        .map_err(|err| Error::Config(format!("invalid S3 endpoint host {host:?}: {err}")))?;
    Ok(url.as_str().trim_end_matches('/').to_owned())
}

#[cfg(test)]
mod tests {
    use super::virtual_hosted_endpoint;

    #[test]
    fn virtual_hosted_endpoint_prefixes_the_bucket() {
        assert_eq!(
            virtual_hosted_endpoint("https://s3.eu-west-1.amazonaws.com", "blobs").unwrap(),
            "https://blobs.s3.eu-west-1.amazonaws.com"
        );
        assert_eq!(
            virtual_hosted_endpoint("https://acct.r2.cloudflarestorage.com:8443/", "blobs")
                .unwrap(),
            "https://blobs.acct.r2.cloudflarestorage.com:8443"
        );
    }
}
