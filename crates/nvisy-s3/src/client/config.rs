//! Connection configuration for the blob store.

/// Connection settings for the first-party S3-compatible blob store.
///
/// A single S3 bucket holds every logical store (files, audits, avatars);
/// [`Bucket`](crate::Bucket) prefixes keep them apart within it, so a deployment
/// provisions one bucket rather than one per kind. `endpoint` selects the
/// backend: unset targets AWS S3, set targets an S3-compatible server (RustFS,
/// MinIO, R2, …).
#[derive(Clone)]
#[cfg_attr(feature = "cli", derive(clap::Args))]
#[must_use = "config does nothing unless you use it"]
pub struct S3Config {
    /// Name of the S3 bucket holding all first-party objects.
    #[cfg_attr(feature = "cli", arg(long, env = "S3_BUCKET"))]
    pub bucket: String,

    /// AWS region. Also required by S3-compatible servers, which usually accept
    /// any value.
    #[cfg_attr(
        feature = "cli",
        arg(long, env = "S3_REGION", default_value = "us-east-1")
    )]
    pub region: String,

    /// Endpoint URL of an S3-compatible server (e.g. `http://localhost:9000`).
    /// Unset targets AWS S3 itself.
    #[cfg_attr(feature = "cli", arg(long, env = "S3_ENDPOINT"))]
    pub endpoint: Option<String>,

    /// Use path-style addressing (`endpoint/bucket/key`) rather than virtual-host
    /// style (`bucket.endpoint/key`). Required by most S3-compatible servers; AWS
    /// S3 uses virtual-host style.
    #[cfg_attr(
        feature = "cli",
        arg(
            long,
            env = "S3_FORCE_PATH_STYLE",
            action = clap::ArgAction::Set,
            default_value_t = true
        )
    )]
    pub force_path_style: bool,

    /// Access key id. When unset (alongside `secret_access_key`), the SDK's
    /// default credential chain is used (environment, profile, IAM role, …).
    #[cfg_attr(feature = "cli", arg(long, env = "S3_ACCESS_KEY_ID"))]
    pub access_key_id: Option<String>,

    /// Secret access key, paired with `access_key_id` for static credentials.
    #[cfg_attr(feature = "cli", arg(long, env = "S3_SECRET_ACCESS_KEY"))]
    pub secret_access_key: Option<String>,
}

impl std::fmt::Debug for S3Config {
    /// Redacts the secret access key so effective-config dumps never leak it.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("S3Config")
            .field("bucket", &self.bucket)
            .field("region", &self.region)
            .field("endpoint", &self.endpoint)
            .field("force_path_style", &self.force_path_style)
            .field(
                "access_key_id",
                &self.access_key_id.as_deref().map(|_| "***"),
            )
            .field(
                "secret_access_key",
                &self.secret_access_key.as_deref().map(|_| "***"),
            )
            .finish()
    }
}
