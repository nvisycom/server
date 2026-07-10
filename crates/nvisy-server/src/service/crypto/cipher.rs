//! XChaCha20-Poly1305 encryption over one authenticated wire format.
//!
//! Data is encrypted in fixed-size authenticated chunks (the STREAM
//! construction, `aead-stream` `LE31` variant): each chunk is its own AEAD
//! frame and the final chunk is flagged, so a truncated stream fails to
//! decrypt. Every entry point produces and consumes this same framing, so they
//! interoperate freely.
//!
//! Three shapes over that format: [`encrypt`] / [`decrypt`] for buffers already
//! in memory, [`encrypt_json`] / [`decrypt_json`] for serializable values, and
//! the constant-memory [`encrypt_reader`] / [`decrypt_reader`] adapters for
//! piping large files through upload and download without buffering.
//!
//! # Wire Format
//!
//! ```text
//! nonce_prefix (20 bytes) || frame*
//! frame       := ciphertext_chunk || tag (16 bytes)
//! ```
//!
//! Every frame but the last carries exactly [`CHUNK_SIZE`] plaintext bytes; the
//! last carries the remainder (possibly zero) and is authenticated as the final
//! block. The 20-byte prefix is the XChaCha20-Poly1305 nonce (24 bytes) minus
//! the 4 bytes `LE31` reserves for its per-chunk counter and last-block flag.

use std::io;

use aead_stream::{DecryptorLE31, EncryptorLE31};
use bytes::Bytes;
use chacha20poly1305::XChaCha20Poly1305;
use chacha20poly1305::aead::KeyInit;
use futures::Stream;
use rand::Rng;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio_util::io::StreamReader;

use super::error::{CryptoError, CryptoResult};
use super::key::EncryptionKey;

/// Plaintext bytes per authenticated chunk (64 KiB).
const CHUNK_SIZE: usize = 64 * 1024;

/// Size of the XChaCha20-Poly1305 nonce in bytes.
const NONCE_SIZE: usize = 24;

/// Bytes `LE31` reserves within the nonce for its per-chunk counter and
/// last-block flag.
const LE31_OVERHEAD: usize = 4;

/// Size of the Poly1305 tag appended to each chunk.
const TAG_SIZE: usize = 16;

/// Ciphertext bytes per full (non-final) chunk.
const ENCRYPTED_CHUNK_SIZE: usize = CHUNK_SIZE + TAG_SIZE;

/// Length of the stream nonce prefix: the nonce less the `LE31` overhead.
const NONCE_PREFIX_SIZE: usize = NONCE_SIZE - LE31_OVERHEAD;

/// Encrypts `plaintext` as a chunked STREAM, returning the framed ciphertext.
///
/// The returned bytes begin with the [`NONCE_PREFIX_SIZE`]-byte nonce prefix,
/// followed by one authenticated frame per [`CHUNK_SIZE`] chunk. This is the
/// in-memory counterpart to the reader-based path; a true streaming pipe reuses
/// the same framing chunk by chunk.
pub fn encrypt(key: &EncryptionKey, plaintext: &[u8]) -> CryptoResult<Vec<u8>> {
    let mut prefix = [0u8; NONCE_PREFIX_SIZE];
    rand::rng().fill_bytes(&mut prefix);

    let cipher = XChaCha20Poly1305::new(key.as_bytes().into());
    let mut encryptor = EncryptorLE31::from_aead(cipher, (&prefix).into());

    let mut out = Vec::with_capacity(NONCE_PREFIX_SIZE + plaintext.len() + TAG_SIZE);
    out.extend_from_slice(&prefix);

    // All chunks but the last are sealed as intermediate blocks; the final
    // chunk (even if empty) is sealed as the last block so truncation fails.
    let mut chunks = plaintext.chunks(CHUNK_SIZE).peekable();
    loop {
        let chunk = chunks.next().unwrap_or_default();
        if chunks.peek().is_none() {
            let frame = encryptor
                .encrypt_last(chunk)
                .map_err(|_| CryptoError::EncryptionFailed)?;
            out.extend_from_slice(&frame);
            break;
        }
        let frame = encryptor
            .encrypt_next(chunk)
            .map_err(|_| CryptoError::EncryptionFailed)?;
        out.extend_from_slice(&frame);
    }

    Ok(out)
}

/// Decrypts a chunked STREAM produced by [`encrypt`].
///
/// Fails if any frame's tag is invalid or the stream was truncated (the final
/// frame authenticates as the last block, so a dropped tail is detected).
pub fn decrypt(key: &EncryptionKey, ciphertext: &[u8]) -> CryptoResult<Vec<u8>> {
    if ciphertext.len() < NONCE_PREFIX_SIZE + TAG_SIZE {
        return Err(CryptoError::CiphertextTooShort);
    }

    let (prefix, mut frames) = ciphertext.split_at(NONCE_PREFIX_SIZE);
    let prefix: [u8; NONCE_PREFIX_SIZE] = prefix
        .try_into()
        .map_err(|_| CryptoError::CiphertextTooShort)?;
    let cipher = XChaCha20Poly1305::new(key.as_bytes().into());
    let mut decryptor = DecryptorLE31::from_aead(cipher, (&prefix).into());

    let mut out = Vec::with_capacity(ciphertext.len());
    // Consume full frames as intermediate blocks; whatever remains is the last.
    while frames.len() > ENCRYPTED_CHUNK_SIZE {
        let (frame, rest) = frames.split_at(ENCRYPTED_CHUNK_SIZE);
        let chunk = decryptor
            .decrypt_next(frame)
            .map_err(|_| CryptoError::DecryptionFailed)?;
        out.extend_from_slice(&chunk);
        frames = rest;
    }
    let chunk = decryptor
        .decrypt_last(frames)
        .map_err(|_| CryptoError::DecryptionFailed)?;
    out.extend_from_slice(&chunk);

    Ok(out)
}

/// Encrypts a serializable value as JSON under the same framing as [`encrypt`].
pub fn encrypt_json<T: Serialize>(key: &EncryptionKey, value: &T) -> CryptoResult<Vec<u8>> {
    let json = serde_json::to_vec(value).map_err(|e| CryptoError::Json(e.to_string()))?;
    encrypt(key, &json)
}

/// Decrypts and deserializes a JSON value produced by [`encrypt_json`].
pub fn decrypt_json<T: DeserializeOwned>(
    key: &EncryptionKey,
    ciphertext: &[u8],
) -> CryptoResult<T> {
    let plaintext = decrypt(key, ciphertext)?;
    serde_json::from_slice(&plaintext).map_err(|e| CryptoError::Json(e.to_string()))
}

/// Wraps a plaintext reader as an [`AsyncRead`] yielding the encrypted STREAM.
///
/// Reads one [`CHUNK_SIZE`] plaintext frame at a time and emits its sealed
/// frame, so at most one chunk is held in memory regardless of file size. The
/// nonce prefix leads the output; the final frame is sealed as the last block.
pub fn encrypt_reader<R>(key: EncryptionKey, reader: R) -> impl AsyncRead
where
    R: AsyncRead + Unpin + Send,
{
    StreamReader::new(encrypt_frames(key, reader))
}

/// Wraps a ciphertext reader as an [`AsyncRead`] yielding the decrypted stream.
///
/// The inverse of [`encrypt_reader`]: reads the nonce prefix then one encrypted
/// frame at a time, emitting verified plaintext. A truncated or tampered stream
/// surfaces as an [`io::Error`] mid-read.
pub fn decrypt_reader<R>(key: EncryptionKey, reader: R) -> impl AsyncRead
where
    R: AsyncRead + Unpin + Send,
{
    StreamReader::new(decrypt_frames(key, reader))
}

/// Streams sealed frames from a plaintext reader.
///
/// Reads one full [`CHUNK_SIZE`] plaintext chunk per iteration; a short read
/// (only reached at EOF, since [`read_chunk`] drains the source) marks the last
/// frame. An exact-multiple input therefore ends on an empty last frame.
fn encrypt_frames<R>(key: EncryptionKey, mut reader: R) -> impl Stream<Item = io::Result<Bytes>>
where
    R: AsyncRead + Unpin + Send,
{
    async_stream::try_stream! {
        let mut prefix = [0u8; NONCE_PREFIX_SIZE];
        rand::rng().fill_bytes(&mut prefix);
        yield Bytes::copy_from_slice(&prefix);

        let cipher = XChaCha20Poly1305::new(key.as_bytes().into());
        let mut encryptor = EncryptorLE31::from_aead(cipher, (&prefix).into());

        let mut chunk = vec![0u8; CHUNK_SIZE];
        loop {
            let n = read_chunk(&mut reader, &mut chunk).await?;
            if n < CHUNK_SIZE {
                let frame = encryptor.encrypt_last(&chunk[..n]).map_err(encrypt_io_error)?;
                yield Bytes::from(frame);
                break;
            }
            let frame = encryptor.encrypt_next(&chunk[..n]).map_err(encrypt_io_error)?;
            yield Bytes::from(frame);
        }
    }
}

/// Streams verified plaintext from a ciphertext reader.
///
/// LE31 requires knowing which frame is last before opening it, so this holds
/// one full frame back: it opens the held frame as the last block once the next
/// read comes up short. Two buffers are swapped to avoid per-frame allocation.
fn decrypt_frames<R>(key: EncryptionKey, mut reader: R) -> impl Stream<Item = io::Result<Bytes>>
where
    R: AsyncRead + Unpin + Send,
{
    async_stream::try_stream! {
        let mut prefix = [0u8; NONCE_PREFIX_SIZE];
        reader.read_exact(&mut prefix).await?;

        let cipher = XChaCha20Poly1305::new(key.as_bytes().into());
        let mut decryptor = DecryptorLE31::from_aead(cipher, (&prefix).into());

        let mut held = vec![0u8; ENCRYPTED_CHUNK_SIZE];
        let mut ahead = vec![0u8; ENCRYPTED_CHUNK_SIZE];
        let mut held_len = read_chunk(&mut reader, &mut held).await?;
        loop {
            // A short frame is unambiguously the last one.
            if held_len < ENCRYPTED_CHUNK_SIZE {
                let chunk = decryptor.decrypt_last(&held[..held_len]).map_err(decrypt_io_error)?;
                yield Bytes::from(chunk);
                break;
            }
            // A full frame is the last one only if nothing follows it.
            let ahead_len = read_chunk(&mut reader, &mut ahead).await?;
            if ahead_len == 0 {
                let chunk = decryptor.decrypt_last(&held[..held_len]).map_err(decrypt_io_error)?;
                yield Bytes::from(chunk);
                break;
            }
            let chunk = decryptor.decrypt_next(&held[..held_len]).map_err(decrypt_io_error)?;
            yield Bytes::from(chunk);
            std::mem::swap(&mut held, &mut ahead);
            held_len = ahead_len;
        }
    }
}

/// Reads until `buf` is full or the source hits EOF, returning the byte count.
///
/// Unlike a single `read`, this coalesces partial reads so a caller can treat a
/// returned length below `buf.len()` as a definitive end of input.
async fn read_chunk<R: AsyncRead + Unpin>(reader: &mut R, buf: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match reader.read(&mut buf[filled..]).await? {
            0 => break,
            n => filled += n,
        }
    }
    Ok(filled)
}

/// Maps a seal failure into an [`io::Error`] for the encrypting adapter.
fn encrypt_io_error(_: aead_stream::aead::Error) -> io::Error {
    io::Error::other(CryptoError::EncryptionFailed)
}

/// Maps an open failure into an [`io::Error`] for the decrypting adapter.
fn decrypt_io_error(_: aead_stream::aead::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, CryptoError::DecryptionFailed)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn key() -> EncryptionKey {
        EncryptionKey::generate()
    }

    #[test]
    fn round_trips_empty() {
        let k = key();
        let ct = encrypt(&k, b"").unwrap();
        assert_eq!(decrypt(&k, &ct).unwrap(), b"");
    }

    #[test]
    fn round_trips_small() {
        let k = key();
        let msg = b"the quick brown fox";
        let ct = encrypt(&k, msg).unwrap();
        assert_eq!(decrypt(&k, &ct).unwrap(), msg);
    }

    #[test]
    fn round_trips_exactly_one_chunk() {
        let k = key();
        let msg = vec![7u8; CHUNK_SIZE];
        let ct = encrypt(&k, &msg).unwrap();
        assert_eq!(decrypt(&k, &ct).unwrap(), msg);
    }

    #[test]
    fn round_trips_multi_chunk_with_remainder() {
        let k = key();
        let msg = vec![3u8; CHUNK_SIZE * 2 + 123];
        let ct = encrypt(&k, &msg).unwrap();
        assert_eq!(decrypt(&k, &ct).unwrap(), msg);
    }

    #[test]
    fn wrong_key_fails() {
        let msg = b"secret";
        let ct = encrypt(&key(), msg).unwrap();
        assert!(matches!(
            decrypt(&key(), &ct),
            Err(CryptoError::DecryptionFailed)
        ));
    }

    #[test]
    fn truncated_tail_fails() {
        let k = key();
        let msg = vec![1u8; CHUNK_SIZE * 2 + 5];
        let ct = encrypt(&k, &msg).unwrap();
        // Drop the final frame: the stream no longer ends on its last block.
        let truncated = &ct[..NONCE_PREFIX_SIZE + ENCRYPTED_CHUNK_SIZE];
        assert!(decrypt(&k, truncated).is_err());
    }

    async fn read_all(reader: impl AsyncRead) -> Vec<u8> {
        let mut reader = std::pin::pin!(reader);
        let mut out = Vec::new();
        reader.read_to_end(&mut out).await.unwrap();
        out
    }

    /// Encrypts via the reader adapter and reads it back with the adapter.
    async fn reader_round_trip(msg: &[u8]) {
        let k = key();
        let ct = read_all(encrypt_reader(k.clone(), Cursor::new(msg.to_vec()))).await;
        let pt = read_all(decrypt_reader(k.clone(), Cursor::new(ct))).await;
        assert_eq!(pt, msg);
    }

    #[tokio::test]
    async fn reader_round_trips_across_sizes() {
        reader_round_trip(b"").await;
        reader_round_trip(b"small").await;
        reader_round_trip(&vec![7u8; CHUNK_SIZE - 1]).await;
        reader_round_trip(&vec![7u8; CHUNK_SIZE]).await;
        reader_round_trip(&vec![7u8; CHUNK_SIZE + 1]).await;
        reader_round_trip(&vec![5u8; CHUNK_SIZE * 3]).await;
        reader_round_trip(&vec![3u8; CHUNK_SIZE * 2 + 123]).await;
    }

    /// The reader and buffered paths share one wire format, so they interoperate
    /// in both directions.
    #[tokio::test]
    async fn reader_and_buffered_interoperate() {
        let k = key();
        let msg = vec![42u8; CHUNK_SIZE * 2 + 7];

        // buffered encrypt -> reader decrypt
        let ct = encrypt(&k, &msg).unwrap();
        let pt = read_all(decrypt_reader(k.clone(), Cursor::new(ct))).await;
        assert_eq!(pt, msg);

        // reader encrypt -> buffered decrypt
        let ct = read_all(encrypt_reader(k.clone(), Cursor::new(msg.clone()))).await;
        assert_eq!(decrypt(&k, &ct).unwrap(), msg);
    }

    #[tokio::test]
    async fn reader_decrypt_rejects_tamper() {
        let k = key();
        let msg = vec![1u8; CHUNK_SIZE + 10];
        let mut ct = read_all(encrypt_reader(k.clone(), Cursor::new(msg))).await;
        let last = ct.len() - 1;
        ct[last] ^= 0x01;

        let mut reader = std::pin::pin!(decrypt_reader(k.clone(), Cursor::new(ct)));
        let mut out = Vec::new();
        assert!(reader.read_to_end(&mut out).await.is_err());
    }

    /// The buffered and reader encoders frame exact multiples differently (a
    /// full last frame vs a trailing empty one); both must still cross-decode.
    #[tokio::test]
    async fn exact_multiple_interoperates_across_paths() {
        let k = key();
        for chunks in [1usize, 2, 3] {
            let msg = vec![9u8; CHUNK_SIZE * chunks];

            // buffered encrypt -> reader decrypt
            let ct = encrypt(&k, &msg).unwrap();
            assert_eq!(
                read_all(decrypt_reader(k.clone(), Cursor::new(ct))).await,
                msg
            );

            // reader encrypt -> buffered decrypt
            let ct = read_all(encrypt_reader(k.clone(), Cursor::new(msg.clone()))).await;
            assert_eq!(decrypt(&k, &ct).unwrap(), msg);
        }
    }

    /// A source that hands out only a few bytes per read must still round-trip:
    /// the adapters coalesce partial reads into full frames.
    #[tokio::test]
    async fn round_trips_over_a_partial_read_source() {
        let k = key();
        let msg = vec![4u8; CHUNK_SIZE * 2 + 77];

        let ct = read_all(encrypt_reader(
            k.clone(),
            ChunkedReader::new(msg.clone(), 7),
        ))
        .await;
        let pt = read_all(decrypt_reader(k.clone(), ChunkedReader::new(ct, 13))).await;
        assert_eq!(pt, msg);
    }

    /// A reader that yields at most `step` bytes per `poll_read`, to exercise the
    /// partial-read coalescing the real multipart and object-store readers hit.
    struct ChunkedReader {
        data: Vec<u8>,
        pos: usize,
        step: usize,
    }

    impl ChunkedReader {
        fn new(data: Vec<u8>, step: usize) -> Self {
            Self { data, pos: 0, step }
        }
    }

    impl AsyncRead for ChunkedReader {
        fn poll_read(
            mut self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<io::Result<()>> {
            let remaining = self.data.len() - self.pos;
            let n = remaining.min(self.step).min(buf.remaining());
            let start = self.pos;
            buf.put_slice(&self.data[start..start + n]);
            self.pos += n;
            std::task::Poll::Ready(Ok(()))
        }
    }
}
