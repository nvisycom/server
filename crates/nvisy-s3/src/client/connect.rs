//! Building a connected [`BlobStore`] from configuration.

use aws_config::BehaviorVersion;
use aws_credential_types::Credentials;
use aws_sdk_s3::config::Region;

use super::{BlobStore, S3Config};
use crate::error::{Error, Result};

/// Tracing target for connection setup.
const TRACING_TARGET: &str = "nvisy_s3::connect";

/// Static-credentials provider name used when the config supplies a key pair.
const STATIC_PROVIDER: &str = "nvisy-s3-static";

impl BlobStore {
    /// Connects a [`BlobStore`] to the S3-compatible service described by `config`.
    ///
    /// When both `access_key_id` and `secret_access_key` are set, static
    /// credentials are used; otherwise the SDK's default credential chain applies
    /// (environment, profile, container/instance role). `endpoint` targets an
    /// S3-compatible server; unset targets AWS S3.
    #[tracing::instrument(name = "s3.connect", skip_all, fields(bucket = %config.bucket))]
    pub async fn connect(config: &S3Config) -> Result<Self> {
        tracing::debug!(
            target: TRACING_TARGET,
            endpoint = ?config.endpoint,
            region = %config.region,
            path_style = config.force_path_style,
            "Connecting to the blob store",
        );

        let mut loader = aws_config::defaults(BehaviorVersion::latest())
            .region(Region::new(config.region.clone()));

        match (&config.access_key_id, &config.secret_access_key) {
            (Some(access_key_id), Some(secret_access_key)) => {
                loader = loader.credentials_provider(Credentials::new(
                    access_key_id,
                    secret_access_key,
                    None,
                    None,
                    STATIC_PROVIDER,
                ));
            }
            (None, None) => {}
            _ => {
                return Err(Error::Config(
                    "S3 credentials incomplete: set both an access key id and a secret access key, \
                     or neither (to use the default credential chain)"
                        .to_owned(),
                ));
            }
        }

        let shared = loader.load().await;
        let mut builder = aws_sdk_s3::config::Builder::from(&shared);
        if let Some(endpoint) = &config.endpoint {
            builder = builder.endpoint_url(endpoint);
        }
        // S3-compatible servers address as `endpoint/bucket/key`; AWS S3 uses
        // `bucket.endpoint/key`.
        builder = builder.force_path_style(config.force_path_style);

        let client = aws_sdk_s3::Client::from_conf(builder.build());
        Ok(Self::new(client, config.bucket.clone()))
    }
}
