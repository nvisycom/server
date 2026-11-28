//! Connection configuration for the blob store.

use nvisy_core::Secret;

/// Connection settings for the first-party S3-compatible blob store.
///
/// A single S3 bucket holds every logical store (files, audits, avatars);
/// [`Bucket`] prefixes keep them apart within it, so a deployment provisions one
/// bucket rather than one per kind. `endpoint` selects the backend: unset targets
/// AWS S3, set targets an S3-compatible server (`RustFS`, `MinIO`, R2, …).
///
/// [`Bucket`]: crate::Bucket
#[derive(Debug, Clone)]
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

    /// Access key id. When unset (alongside `secret_access_key`), credentials
    /// come from the environment chain (`AWS_*` variables, web identity, ECS
    /// container credentials, instance metadata); `~/.aws` profiles are not read.
    #[cfg_attr(feature = "cli", arg(long, env = "S3_ACCESS_KEY_ID"))]
    pub access_key_id: Option<Secret<String>>,

    /// Secret access key, paired with `access_key_id` for static credentials.
    #[cfg_attr(feature = "cli", arg(long, env = "S3_SECRET_ACCESS_KEY"))]
    pub secret_access_key: Option<Secret<String>>,
}
