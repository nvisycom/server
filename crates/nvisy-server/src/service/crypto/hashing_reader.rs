//! An [`AsyncRead`] tee that measures the plaintext flowing through it.
//!
//! Once file bytes are encrypted before storage, the object store only ever
//! sees ciphertext and can no longer report the plaintext size or hash for the
//! file record. [`HashingReader`] sits ahead of the encryptor in the upload
//! pipe and captures both as the bytes stream past, without buffering the file.

use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use pin_project_lite::pin_project;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, ReadBuf};

/// Running plaintext size and hash, shared with a [`HashingReader`].
#[derive(Default)]
struct Meter {
    hasher: Sha256,
    bytes: u64,
}

/// A shared handle to the plaintext measurements taken by a [`HashingReader`].
///
/// Cloneable and readable after the reader has been consumed (e.g. handed to
/// the encryptor and streamed to storage), which is when the plaintext size and
/// hash are needed for the file record.
#[derive(Clone, Default)]
pub struct Measurements {
    meter: Arc<Mutex<Meter>>,
}

impl Measurements {
    /// Total plaintext bytes measured.
    pub fn bytes(&self) -> u64 {
        self.meter.lock().expect("measurements lock").bytes
    }

    /// SHA-256 over all plaintext measured.
    pub fn sha256(&self) -> [u8; 32] {
        let lock = self.meter.lock().expect("measurements lock");
        lock.hasher.clone().finalize().into()
    }
}

pin_project! {
    /// An [`AsyncRead`] wrapper that measures the plaintext flowing through it.
    ///
    /// Wrap the source reader with this *before* encrypting so the byte count
    /// and SHA-256 describe the plaintext, not the ciphertext that lands in
    /// storage. Read the results from the [`Measurements`] handle after the
    /// reader is drained.
    pub struct HashingReader<R> {
        #[pin]
        inner: R,
        measurements: Measurements,
    }
}

impl<R> HashingReader<R> {
    /// Wraps a reader, returning it alongside a handle to its measurements.
    pub fn new(inner: R) -> (Self, Measurements) {
        let measurements = Measurements::default();
        let reader = Self {
            inner,
            measurements: measurements.clone(),
        };
        (reader, measurements)
    }
}

impl<R: AsyncRead> AsyncRead for HashingReader<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.project();
        let before = buf.filled().len();
        let poll = this.inner.poll_read(cx, buf);
        if let Poll::Ready(Ok(())) = &poll {
            let new = &buf.filled()[before..];
            if !new.is_empty() {
                let mut meter = this.measurements.meter.lock().expect("measurements lock");
                meter.hasher.update(new);
                meter.bytes += new.len() as u64;
            }
        }
        poll
    }
}
