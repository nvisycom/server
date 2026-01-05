//! Streaming reader that computes SHA-256 hash on-the-fly.

use std::pin::Pin;
use std::task::{Context, Poll};

use pin_project_lite::pin_project;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, ReadBuf};

pin_project! {
    /// An async reader wrapper that computes SHA-256 hash as data flows through.
    ///
    /// This allows computing the hash during streaming uploads without
    /// buffering the entire content in memory.
    pub struct HashingReader<R> {
        #[pin]
        inner: R,
        hasher: Sha256,
    }
}

impl<R> HashingReader<R> {
    /// Creates a new hashing reader wrapping the given reader.
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
        }
    }

    /// Consumes the reader and returns the SHA-256 hash as bytes.
    pub fn finalize(self) -> [u8; 32] {
        self.hasher.finalize().into()
    }

    /// Consumes the reader and returns the SHA-256 hash as a hex string.
    #[cfg(test)]
    pub fn finalize_hex(self) -> String {
        hex::encode(self.finalize())
    }
}

impl<R: AsyncRead> AsyncRead for HashingReader<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.project();
        let before = buf.filled().len();

        match this.inner.poll_read(cx, buf) {
            Poll::Ready(Ok(())) => {
                let new_bytes = &buf.filled()[before..];
                if !new_bytes.is_empty() {
                    this.hasher.update(new_bytes);
                }
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::AsyncReadExt;

    use super::*;

    #[tokio::test]
    async fn test_hashing_reader_empty() {
        let data: &[u8] = &[];
        let reader = HashingReader::new(data);

        // Empty SHA-256 hash
        let expected = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert_eq!(reader.finalize_hex(), expected);
    }

    #[tokio::test]
    async fn test_hashing_reader_small_data() {
        let data = b"Hello, World!";
        let mut reader = HashingReader::new(&data[..]);

        let mut buf = Vec::new();
        reader.read_to_end(&mut buf).await.unwrap();

        assert_eq!(buf, data);

        // SHA-256 of "Hello, World!"
        let expected = "dffd6021bb2bd5b0af676290809ec3a53191dd81c7f70a4b28688a362182986f";
        assert_eq!(reader.finalize_hex(), expected);
    }

    #[tokio::test]
    async fn test_hashing_reader_chunked() {
        let data = b"Hello, World!";
        let mut reader = HashingReader::new(&data[..]);

        // Read in small chunks
        let mut buf = [0u8; 5];
        let mut total = Vec::new();

        loop {
            let n = reader.read(&mut buf).await.unwrap();
            if n == 0 {
                break;
            }
            total.extend_from_slice(&buf[..n]);
        }

        assert_eq!(total, data);

        let expected = "dffd6021bb2bd5b0af676290809ec3a53191dd81c7f70a4b28688a362182986f";
        assert_eq!(reader.finalize_hex(), expected);
    }
}
