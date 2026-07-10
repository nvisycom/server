//! Streaming XChaCha20-Poly1305 encryption for large payloads.
//!
//! Where [`cipher`](super::cipher) seals a whole buffer with a single tag —
//! right for small blobs, but requiring the entire plaintext in memory — this
//! module encrypts a byte stream in fixed-size authenticated chunks so files
//! never have to be buffered whole. It uses the STREAM construction
//! (`aead::stream`, `LE31` variant): each chunk is its own AEAD frame, and the
//! final chunk is flagged so a truncated stream fails to decrypt.
//!
//! # Wire Format
//!
//! ```text
//! nonce_prefix (19 bytes) || frame*
//! frame       := ciphertext_chunk || tag (16 bytes)
//! ```
//!
//! Every frame but the last carries exactly [`CHUNK_SIZE`] plaintext bytes; the
//! last carries the remainder (possibly zero) and is authenticated as the final
//! block. The 20-byte prefix is the XChaCha20-Poly1305 nonce (24 bytes) minus
//! the 4 bytes `LE31` reserves for its per-chunk counter and last-block flag.

use aead_stream::{DecryptorLE31, EncryptorLE31};
use chacha20poly1305::XChaCha20Poly1305;
use chacha20poly1305::aead::KeyInit;
use rand::Rng;

use super::error::{CryptoError, CryptoResult};
use super::key::EncryptionKey;

/// Plaintext bytes per authenticated chunk (64 KiB).
pub const CHUNK_SIZE: usize = 64 * 1024;

/// Size of the Poly1305 tag appended to each chunk.
const TAG_SIZE: usize = 16;

/// Ciphertext bytes per full (non-final) chunk.
const ENCRYPTED_CHUNK_SIZE: usize = CHUNK_SIZE + TAG_SIZE;

/// Length of the stream nonce prefix: the 24-byte XChaCha20-Poly1305 nonce less
/// the 4 bytes `LE31` uses for its counter and last-block flag.
pub const NONCE_PREFIX_SIZE: usize = 24 - 4;

/// Encrypts `plaintext` as a chunked STREAM, returning the framed ciphertext.
///
/// The returned bytes begin with the [`NONCE_PREFIX_SIZE`]-byte nonce prefix,
/// followed by one authenticated frame per [`CHUNK_SIZE`] chunk. This is the
/// in-memory counterpart to the reader-based path; a true streaming pipe reuses
/// the same framing chunk by chunk.
pub fn encrypt_stream(key: &EncryptionKey, plaintext: &[u8]) -> CryptoResult<Vec<u8>> {
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
                .map_err(|_| CryptoError::RandomGenerationFailed)?;
            out.extend_from_slice(&frame);
            break;
        }
        let frame = encryptor
            .encrypt_next(chunk)
            .map_err(|_| CryptoError::RandomGenerationFailed)?;
        out.extend_from_slice(&frame);
    }

    Ok(out)
}

/// Decrypts a chunked STREAM produced by [`encrypt_stream`].
///
/// Fails if any frame's tag is invalid or the stream was truncated (the final
/// frame authenticates as the last block, so a dropped tail is detected).
pub fn decrypt_stream(key: &EncryptionKey, ciphertext: &[u8]) -> CryptoResult<Vec<u8>> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> EncryptionKey {
        EncryptionKey::generate()
    }

    #[test]
    fn round_trips_empty() {
        let k = key();
        let ct = encrypt_stream(&k, b"").unwrap();
        assert_eq!(decrypt_stream(&k, &ct).unwrap(), b"");
    }

    #[test]
    fn round_trips_small() {
        let k = key();
        let msg = b"the quick brown fox";
        let ct = encrypt_stream(&k, msg).unwrap();
        assert_eq!(decrypt_stream(&k, &ct).unwrap(), msg);
    }

    #[test]
    fn round_trips_exactly_one_chunk() {
        let k = key();
        let msg = vec![7u8; CHUNK_SIZE];
        let ct = encrypt_stream(&k, &msg).unwrap();
        assert_eq!(decrypt_stream(&k, &ct).unwrap(), msg);
    }

    #[test]
    fn round_trips_multi_chunk_with_remainder() {
        let k = key();
        let msg = vec![3u8; CHUNK_SIZE * 2 + 123];
        let ct = encrypt_stream(&k, &msg).unwrap();
        assert_eq!(decrypt_stream(&k, &ct).unwrap(), msg);
    }

    #[test]
    fn wrong_key_fails() {
        let msg = b"secret";
        let ct = encrypt_stream(&key(), msg).unwrap();
        assert!(matches!(
            decrypt_stream(&key(), &ct),
            Err(CryptoError::DecryptionFailed)
        ));
    }

    #[test]
    fn truncated_tail_fails() {
        let k = key();
        let msg = vec![1u8; CHUNK_SIZE * 2 + 5];
        let ct = encrypt_stream(&k, &msg).unwrap();
        // Drop the final frame: the stream no longer ends on its last block.
        let truncated = &ct[..NONCE_PREFIX_SIZE + ENCRYPTED_CHUNK_SIZE];
        assert!(decrypt_stream(&k, truncated).is_err());
    }

    #[test]
    fn tampered_frame_fails() {
        let k = key();
        let msg = vec![9u8; 512];
        let mut ct = encrypt_stream(&k, &msg).unwrap();
        let last = ct.len() - 1;
        ct[last] ^= 0x01;
        assert!(matches!(
            decrypt_stream(&k, &ct),
            Err(CryptoError::DecryptionFailed)
        ));
    }
}
