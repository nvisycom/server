//! Test-only support for integration tests that need a real NATS server.
//!
//! Starts an ephemeral NATS container with `JetStream` enabled and hands out a
//! connected [`NatsClient`], so the messaging code can be exercised against a
//! real server without any external setup. Gated on the `test_util` feature, so
//! it never ships in a default build.

use nvisy_core::test_util::boot_permit;
use testcontainers_modules::nats::{Nats, NatsServerCmd};
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};

use crate::{NatsClient, NatsConfig};

/// A running NATS (`JetStream`) container paired with a connected [`NatsClient`].
///
/// Keep the whole `TestNats` alive for the duration of a test: dropping it stops
/// and removes the container. `JetStream` is enabled, so stream and KV operations
/// work.
pub struct TestNats {
    /// Held only to keep the container alive; the client talks to it by URL.
    _container: ContainerAsync<Nats>,
    /// A client connected to the container.
    pub client: NatsClient,
    /// The `nats://` URL the container is reachable at, for composing a
    /// [`NatsConfig`] elsewhere (e.g. a server's `ServiceState`).
    pub url: String,
}

impl TestNats {
    /// Starts a fresh NATS container with `JetStream` enabled and connects a
    /// [`NatsClient`].
    ///
    /// # Panics
    ///
    /// Panics if the container cannot start or the client cannot connect — a test
    /// cannot proceed without a working NATS server.
    pub async fn start() -> Self {
        // Serialize the boot and first connect so many tests do not overwhelm the
        // Docker daemon at once; the permit is released once the server is usable.
        let (container, client, url) = {
            let _permit = boot_permit().await;
            let container = Nats::default()
                .with_cmd(&NatsServerCmd::default().with_jetstream())
                .start()
                .await
                .expect("failed to start NATS container");
            let port = container
                .get_host_port_ipv4(4222)
                .await
                .expect("failed to resolve container port");
            let url = format!("nats://127.0.0.1:{port}");
            let client = NatsClient::connect(NatsConfig::new(url.clone(), String::new()))
                .await
                .expect("failed to connect to the test NATS server");
            (container, client, url)
        };

        Self {
            _container: container,
            client,
            url,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn start_yields_a_connected_client() {
        let nats = TestNats::start().await;
        assert!(nats.client.is_connected());
        nats.client.ping().await.expect("ping the test NATS server");
    }
}
