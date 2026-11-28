//! Test-only support for integration tests that need a real S3-compatible store.
//!
//! Starts an ephemeral `RustFS` container (the same S3-compatible backend the
//! dev stack uses), creates its bucket, and builds a [`BlobStore`], so the
//! storage code can be exercised against a real backend without any external
//! setup. Gated on the `test_util` feature, so it never ships in a default build.

use nvisy_core::Secret;
use nvisy_core::test_util::boot_permit;
use object_store::aws::{AwsAuthorizer, AwsCredential};
use object_store::client::{HttpRequest, HttpRequestBody};
use reqwest::Method;
use testcontainers_modules::testcontainers::core::wait::HttpWaitStrategy;
use testcontainers_modules::testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, GenericImage, ImageExt};

use crate::{BlobStore, S3Config};

/// The bucket every test store uses; created by [`TestBlobStore::start`].
pub const TEST_BUCKET: &str = "nvisy-test";

/// The `RustFS` image the dev stack pins; a `MinIO`-style S3-compatible server.
const RUSTFS_IMAGE: &str = "rustfs/rustfs";
const RUSTFS_TAG: &str = "1.0.0-rc.5";

/// The static credentials the container is configured with.
const S3_ACCESS_KEY: &str = "rustfsadmin";
const S3_SECRET_KEY: &str = "rustfsadmin";

/// A running `RustFS` container paired with a connected [`BlobStore`] whose
/// bucket ([`TEST_BUCKET`]) has been created.
///
/// Keep the whole `TestBlobStore` alive for the duration of a test: dropping it
/// stops and removes the container.
pub struct TestBlobStore {
    /// Held only to keep the container alive; the store talks to it by endpoint.
    _container: ContainerAsync<GenericImage>,
    /// A store connected to the container, with [`TEST_BUCKET`] created.
    pub store: BlobStore,
    /// The [`S3Config`] the store was built from, for composing a server's
    /// `ServiceState` against the same container.
    pub config: S3Config,
}

impl TestBlobStore {
    /// Starts a fresh `RustFS` container, creates the test bucket, and builds a
    /// [`BlobStore`] for it.
    ///
    /// # Panics
    ///
    /// Panics if the container cannot start, the bucket cannot be created, or the
    /// store cannot be built: a test cannot proceed without a working store.
    pub async fn start() -> Self {
        let (container, config) = {
            let _permit = boot_permit().await;
            // Ready only once `/health/ready` serves 200 — RustFS serves requests
            // (and accepts bucket creation) only after storage has initialized.
            let ready = HttpWaitStrategy::new("/health/ready")
                .with_port(9000.tcp())
                .with_expected_status_code(200u16);
            let container = GenericImage::new(RUSTFS_IMAGE, RUSTFS_TAG)
                .with_wait_for(WaitFor::http(ready))
                .with_env_var("RUSTFS_ACCESS_KEY", S3_ACCESS_KEY)
                .with_env_var("RUSTFS_SECRET_KEY", S3_SECRET_KEY)
                .start()
                .await
                .expect("failed to start RustFS container");
            let port = container
                .get_host_port_ipv4(9000)
                .await
                .expect("failed to resolve container port");
            let config = S3Config {
                bucket: TEST_BUCKET.to_owned(),
                region: "us-east-1".to_owned(),
                endpoint: Some(format!("http://127.0.0.1:{port}")),
                force_path_style: true,
                access_key_id: Some(Secret::new(S3_ACCESS_KEY.to_owned())),
                secret_access_key: Some(Secret::new(S3_SECRET_KEY.to_owned())),
            };
            // The first request: the store is usable once its bucket exists.
            create_bucket(&config).await;
            (container, config)
        };
        let store = BlobStore::connect(&config).expect("failed to build the test blob store");

        Self {
            _container: container,
            store,
            config,
        }
    }
}

/// Creates the configured bucket with a SigV4-signed `PUT /{bucket}`.
///
/// Deployments provision their bucket out of band, so the store has no
/// bucket-creation API; the harness signs the request with the store client's
/// own signer instead.
async fn create_bucket(config: &S3Config) {
    let endpoint = config
        .endpoint
        .as_deref()
        .expect("the test store has an endpoint");
    let url = format!("{endpoint}/{}", config.bucket);
    let credential = AwsCredential {
        key_id: S3_ACCESS_KEY.to_owned(),
        secret_key: S3_SECRET_KEY.to_owned(),
        token: None,
    };

    let mut request = HttpRequest::new(HttpRequestBody::empty());
    *request.method_mut() = Method::PUT;
    *request.uri_mut() = url.parse().expect("a valid bucket URL");
    AwsAuthorizer::new(&credential, "s3", &config.region)
        .try_authorize(&mut request, None)
        .expect("sign the create-bucket request");

    let response = reqwest::Client::new()
        .put(&url)
        .headers(request.headers().clone())
        .send()
        .await
        .expect("send the create-bucket request");
    assert!(
        response.status().is_success(),
        "creating the test bucket failed with {}",
        response.status(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AccountAvatarKey;

    #[tokio::test]
    async fn start_yields_a_usable_store() {
        let s3 = TestBlobStore::start().await;
        // A put/get round-trip proves the bucket exists and is writable.
        let key = AccountAvatarKey::new(uuid::Uuid::now_v7(), "v1");
        s3.store
            .put(&key, &b"hello"[..])
            .await
            .expect("put an object");
        let got = s3.store.get(&key).await.expect("get an object");
        assert!(got.is_some(), "the object just written should exist");
    }
}
