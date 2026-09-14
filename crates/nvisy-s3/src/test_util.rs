//! Test-only support for integration tests that need a real S3-compatible store.
//!
//! Starts an ephemeral `RustFS` container (the same S3-compatible backend the
//! dev stack uses), connects a [`BlobStore`], and creates its bucket — so the
//! storage code can be exercised against a real backend without any external
//! setup. Gated on the `test_util` feature, so it never ships in a default build.

use std::sync::OnceLock;

use testcontainers_modules::testcontainers::core::wait::HttpWaitStrategy;
use testcontainers_modules::testcontainers::core::{IntoContainerPort, WaitFor};
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, GenericImage, ImageExt};
use tokio::sync::Semaphore;

use crate::{BlobStore, S3Config};

/// The bucket every test store uses; created by [`TestBlobStore::start`].
pub const TEST_BUCKET: &str = "nvisy-test";

/// The `RustFS` image the dev stack pins; a `MinIO`-style S3-compatible server.
const RUSTFS_IMAGE: &str = "rustfs/rustfs";
const RUSTFS_TAG: &str = "1.0.0-rc.5";

/// The static credentials the container is configured with.
const S3_ACCESS_KEY: &str = "rustfsadmin";
const S3_SECRET_KEY: &str = "rustfsadmin";

/// Caps how many storage containers may boot at once, across the whole test
/// binary — booting all at once can overwhelm the Docker daemon and flake the
/// readiness probes. Serializes only the boot; the containers then run in
/// parallel. Raise with `NVISY_TEST_BOOT_CONCURRENCY`.
fn boot_semaphore() -> &'static Semaphore {
    static BOOT: OnceLock<Semaphore> = OnceLock::new();
    BOOT.get_or_init(|| {
        let permits = std::env::var("NVISY_TEST_BOOT_CONCURRENCY")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|&n| n >= 1)
            .unwrap_or(1);
        Semaphore::new(permits)
    })
}

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
    /// Starts a fresh `RustFS` container, connects a [`BlobStore`], and creates
    /// the test bucket.
    ///
    /// # Panics
    ///
    /// Panics if the container cannot start, the store cannot connect, or the
    /// bucket cannot be created — a test cannot proceed without a working store.
    pub async fn start() -> Self {
        let (container, port) = {
            let _permit = boot_semaphore()
                .acquire()
                .await
                .expect("boot semaphore is never closed");
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
            (container, port)
        };

        let config = S3Config {
            bucket: TEST_BUCKET.to_owned(),
            region: "us-east-1".to_owned(),
            endpoint: Some(format!("http://127.0.0.1:{port}")),
            force_path_style: true,
            access_key_id: Some(S3_ACCESS_KEY.to_owned()),
            secret_access_key: Some(S3_SECRET_KEY.to_owned()),
        };

        let store = BlobStore::connect(&config)
            .await
            .expect("failed to connect to the test blob store");
        store
            .create_bucket()
            .await
            .expect("failed to create the test bucket");

        Self {
            _container: container,
            store,
            config,
        }
    }
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
