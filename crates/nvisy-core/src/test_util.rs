//! Test support shared by every crate's container-backed test harness.

use std::sync::OnceLock;

use tokio::sync::{Semaphore, SemaphorePermit};

/// Takes a permit to boot a test container (Postgres, NATS, or S3).
///
/// Booting many containers at once stresses the Docker daemon enough that
/// readiness probes and first connections time out. One semaphore covers every
/// harness in the process, so a test binary that starts all three still boots
/// them one at a time. Hold the permit until the container is usable (started
/// and first connected); the tests themselves run in parallel afterwards.
///
/// The permit count defaults to 1 and can be raised with the
/// `NVISY_TEST_BOOT_CONCURRENCY` environment variable.
///
/// # Panics
///
/// Never in practice: the semaphore is never closed.
pub async fn boot_permit() -> SemaphorePermit<'static> {
    static BOOT: OnceLock<Semaphore> = OnceLock::new();
    BOOT.get_or_init(|| {
        let permits = std::env::var("NVISY_TEST_BOOT_CONCURRENCY")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|&n| n >= 1)
            .unwrap_or(1);
        Semaphore::new(permits)
    })
    .acquire()
    .await
    .expect("the boot semaphore is never closed")
}
