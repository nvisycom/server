//! Avatar service: stores, serves, and removes account and workspace avatars.
//!
//! Uploads are normalized to WebP (decoded, bounded, resized, re-encoded) and
//! stored unencrypted in the blob store, keyed by `(owner, content hash)`
//! so each version is its own object and the serve URL can be cached immutably.
//! Because the stored bytes are always WebP, the serve `Content-Type` is
//! constant and no per-object mime is persisted; re-encoding also strips
//! EXIF/metadata.

use std::io::Cursor;

use bytes::Bytes;
use image::{ImageFormat, ImageReader};
use nvisy_postgres::PgConn;
use nvisy_postgres::model::{UpdateAccount, UpdateWorkspace};
use nvisy_postgres::query::{AccountRepository, WorkspaceRepository};
use nvisy_s3::{AccountAvatarKey, BlobStore, WorkspaceAvatarKey};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::response::{ErrorKind, Result};
use crate::service::Infra;

/// The content type of every stored avatar.
pub const AVATAR_CONTENT_TYPE: &str = "image/webp";

/// Maximum accepted size of a raw avatar upload, before decoding. Exposed so the
/// avatar routes can reject oversized bodies at the transport layer with the same
/// bound the service enforces.
pub const MAX_AVATAR_UPLOAD_BYTES: usize = 2 * 1024 * 1024;

/// Maximum accepted dimension (width or height) of the decoded image.
const MAX_SOURCE_DIMENSION: u32 = 4096;

/// Longest side of the stored avatar; larger images are scaled down to fit.
const TARGET_DIMENSION: u32 = 512;

/// Tracing target for avatar operations.
const TRACING_TARGET: &str = "nvisy_server::service::avatar";

/// Whose avatar: an account's or a workspace's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvatarOwner {
    /// An account's avatar.
    Account(Uuid),
    /// A workspace's avatar.
    Workspace(Uuid),
}

impl AvatarOwner {
    /// The serve URL of a version, `/avatars/{kind}/{id}/{version}/`.
    fn url(self, version: &str) -> String {
        match self {
            Self::Account(id) => format!("/avatars/accounts/{id}/{version}/"),
            Self::Workspace(id) => format!("/avatars/workspaces/{id}/{version}/"),
        }
    }

    /// Writes a version's bytes.
    async fn put(self, blobs: &BlobStore, version: &str, webp: Vec<u8>) -> Result<()> {
        let bytes = Cursor::new(webp);
        match self {
            Self::Account(id) => {
                blobs
                    .put(&AccountAvatarKey::new(id, version), bytes)
                    .await?
            }
            Self::Workspace(id) => {
                blobs
                    .put(&WorkspaceAvatarKey::new(id, version), bytes)
                    .await?
            }
        };
        Ok(())
    }

    /// Reads a version's bytes, or `None` if absent.
    async fn get(self, blobs: &BlobStore, version: &str) -> Result<Option<Vec<u8>>> {
        Ok(match self {
            Self::Account(id) => blobs.get_bytes(&AccountAvatarKey::new(id, version)).await?,
            Self::Workspace(id) => {
                blobs
                    .get_bytes(&WorkspaceAvatarKey::new(id, version))
                    .await?
            }
        })
    }

    /// Deletes a version's bytes.
    async fn delete(self, blobs: &BlobStore, version: &str) -> Result<()> {
        match self {
            Self::Account(id) => blobs.delete(&AccountAvatarKey::new(id, version)).await?,
            Self::Workspace(id) => blobs.delete(&WorkspaceAvatarKey::new(id, version)).await?,
        }
        Ok(())
    }

    /// The owner's current avatar version, if it has one.
    async fn current_version(self, conn: &mut PgConn) -> Result<Option<String>> {
        let url = match self {
            Self::Account(id) => conn
                .find_account_by_id(id)
                .await?
                .and_then(|a| a.avatar_url),
            Self::Workspace(id) => conn
                .find_workspace_by_id(id)
                .await?
                .and_then(|w| w.avatar_url),
        };
        Ok(avatar_version(url.as_deref()))
    }

    /// Points the owner's `avatar_url` at `url`, or clears it.
    async fn set_url(self, conn: &mut PgConn, url: Option<String>) -> Result<()> {
        match self {
            Self::Account(id) => {
                conn.update_account(
                    id,
                    UpdateAccount {
                        avatar_url: Some(url),
                        ..Default::default()
                    },
                )
                .await?;
            }
            Self::Workspace(id) => {
                conn.update_workspace(
                    id,
                    UpdateWorkspace {
                        avatar_url: Some(url),
                        ..Default::default()
                    },
                )
                .await?;
            }
        }
        Ok(())
    }
}

/// Stores, serves, and removes account and workspace avatars.
#[derive(Clone)]
#[must_use = "service does nothing unless you use it"]
pub struct AvatarService {
    infra: Infra,
}

impl AvatarService {
    /// Creates a new [`AvatarService`].
    pub fn new(infra: Infra) -> Self {
        Self { infra }
    }

    /// Normalizes and stores an avatar as a new content-versioned object, points
    /// the owner's `avatar_url` at it, and deletes the previous version.
    ///
    /// The stored object is keyed by `(owner, content hash)` and the URL embeds
    /// the same hash, so a versioned URL maps to immutable bytes. The new object
    /// is written and the URL updated before the previous version is deleted, so
    /// a reader never sees a missing avatar. If the URL update fails, the new
    /// object is removed again; removing the previous version is best-effort, so
    /// a failure (or a crash before it) can leave that object orphaned (see #192).
    ///
    /// # Errors
    ///
    /// - `BadRequest` if the upload exceeds the size cap, does not decode as an
    ///   image, or exceeds the source-dimension cap.
    /// - `InternalServerError` if avatar processing fails (the blocking
    ///   decode/resize task, or WebP re-encoding).
    /// - A storage error if writing the avatar object fails.
    /// - A database error if reading or updating the owner fails.
    pub async fn set(&self, owner: AvatarOwner, upload: Bytes) -> Result<()> {
        let webp = process_avatar(upload).await?;
        let version = content_version(&webp);
        owner.put(&self.infra.blobs, &version, webp).await?;

        let previous = match self.point_at(owner, &version).await {
            Ok(previous) => previous,
            Err(err) => {
                // The URL still names the previous version, so the object just
                // written is unreferenced, unless it is that same version.
                if self.current_version(owner).await.ok().flatten().as_deref() != Some(&version) {
                    self.discard(owner, &version).await;
                }
                return Err(err);
            }
        };

        if let Some(previous) = previous
            && previous != version
        {
            self.discard(owner, &previous).await;
        }
        Ok(())
    }

    /// Points the owner's `avatar_url` at `version`, returning the version it
    /// named before.
    async fn point_at(&self, owner: AvatarOwner, version: &str) -> Result<Option<String>> {
        let mut conn = self.infra.postgres.get_connection().await?;
        let previous = owner.current_version(&mut conn).await?;
        owner.set_url(&mut conn, Some(owner.url(version))).await?;
        Ok(previous)
    }

    /// The version the owner's `avatar_url` currently names.
    async fn current_version(&self, owner: AvatarOwner) -> Result<Option<String>> {
        let mut conn = self.infra.postgres.get_connection().await?;
        owner.current_version(&mut conn).await
    }

    /// Removes an avatar object no URL references, best-effort: a failure only
    /// leaves the object orphaned, so it is logged rather than returned.
    async fn discard(&self, owner: AvatarOwner, version: &str) {
        if let Err(err) = owner.delete(&self.infra.blobs, version).await {
            tracing::warn!(target: TRACING_TARGET, ?owner, version, error = %err, "Failed to remove an unreferenced avatar object");
        }
    }

    /// Reads one version of an avatar, or `None` if absent. Avatars are small
    /// (bounded by the target dimension), so buffering is fine.
    ///
    /// # Errors
    ///
    /// - A storage error if the object store rejects the read.
    pub async fn get(&self, owner: AvatarOwner, version: &str) -> Result<Option<Vec<u8>>> {
        owner.get(&self.infra.blobs, version).await
    }

    /// Clears the owner's `avatar_url`, then removes the object it named. The
    /// URL goes first, so a failure never leaves it naming a missing object;
    /// removing the object is best-effort.
    ///
    /// # Errors
    ///
    /// - A database error if reading or updating the owner fails.
    pub async fn delete(&self, owner: AvatarOwner) -> Result<()> {
        let mut conn = self.infra.postgres.get_connection().await?;
        let current = owner.current_version(&mut conn).await?;
        owner.set_url(&mut conn, None).await?;
        drop(conn);

        if let Some(version) = current {
            self.discard(owner, &version).await;
        }
        Ok(())
    }
}

/// Returns a short content hash of the stored bytes, used as the avatar's
/// version segment.
///
/// The hash changes only when the image does, so an identical re-upload keeps
/// the same object key and URL (cache stays warm) while a new image produces a
/// new one (cache is busted), letting the serve route mark the response
/// immutable.
fn content_version(webp: &[u8]) -> String {
    let digest = Sha256::digest(webp);
    hex::encode(&digest[..8])
}

/// Extracts the version segment from a stored avatar URL of the form
/// `/avatars/{kind}/{id}/{version}/`, if present.
fn avatar_version(avatar_url: Option<&str>) -> Option<String> {
    avatar_url?
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .map(str::to_owned)
}

/// Rejects payloads that are too large, do not decode as an image, or exceed the
/// source-dimension cap. The result is resized to fit within [`TARGET_DIMENSION`]
/// on its longest side (never upscaled) and encoded as WebP. The CPU-bound
/// decode/encode runs on a blocking thread so it does not stall the runtime.
async fn process_avatar(bytes: Bytes) -> Result<Vec<u8>> {
    if bytes.len() > MAX_AVATAR_UPLOAD_BYTES {
        return Err(ErrorKind::BadRequest.with_message("Avatar must be at most 2 MiB"));
    }

    tokio::task::spawn_blocking(move || normalize(&bytes))
        .await
        .map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Avatar processing failed")
                .with_context(err.to_string())
        })?
}

/// Decodes, bounds, resizes, and WebP-encodes the image. Runs on a blocking
/// thread via [`process_avatar`].
fn normalize(bytes: &[u8]) -> Result<Vec<u8>> {
    // Decode by sniffed format, not the client's claim. A payload that does not
    // decode as a supported image is rejected here.
    let image = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|err| {
            ErrorKind::BadRequest
                .with_message("Avatar is not a readable image")
                .with_context(err.to_string())
        })?
        .decode()
        .map_err(|err| {
            ErrorKind::BadRequest
                .with_message("Avatar is not a supported image")
                .with_context(err.to_string())
        })?;

    let (width, height) = (image.width(), image.height());
    if width > MAX_SOURCE_DIMENSION || height > MAX_SOURCE_DIMENSION {
        return Err(
            ErrorKind::BadRequest.with_message("Avatar dimensions must be at most 4096x4096")
        );
    }

    // Scale down to fit the target box, preserving aspect ratio; never upscale.
    let normalized = if width > TARGET_DIMENSION || height > TARGET_DIMENSION {
        image.resize(
            TARGET_DIMENSION,
            TARGET_DIMENSION,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        image
    };

    let mut out = Cursor::new(Vec::new());
    normalized
        .write_to(&mut out, ImageFormat::WebP)
        .map_err(|err| {
            ErrorKind::InternalServerError
                .with_message("Failed to encode avatar")
                .with_context(err.to_string())
        })?;

    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use image::{DynamicImage, ImageFormat, RgbaImage};

    use super::*;

    /// Encodes a solid image of the given size in the given format for test input.
    fn encode(width: u32, height: u32, format: ImageFormat) -> Bytes {
        let img = DynamicImage::ImageRgba8(RgbaImage::new(width, height));
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, format).unwrap();
        Bytes::from(out.into_inner())
    }

    #[tokio::test]
    async fn accepts_and_normalizes_png_to_webp() {
        let png = encode(64, 64, ImageFormat::Png);
        let webp = process_avatar(png).await.unwrap();
        let decoded = ImageReader::new(Cursor::new(&webp))
            .with_guessed_format()
            .unwrap();
        assert_eq!(decoded.format(), Some(ImageFormat::WebP));
    }

    #[tokio::test]
    async fn resizes_down_to_target_box() {
        let big = encode(2000, 1000, ImageFormat::Png);
        let webp = process_avatar(big).await.unwrap();
        let img = image::load_from_memory(&webp).unwrap();
        assert!(img.width() <= TARGET_DIMENSION && img.height() <= TARGET_DIMENSION);
        // Aspect ratio preserved: 2:1 -> 512x256.
        assert_eq!(img.width(), TARGET_DIMENSION);
        assert_eq!(img.height(), TARGET_DIMENSION / 2);
    }

    #[tokio::test]
    async fn leaves_small_images_unscaled() {
        let small = encode(100, 80, ImageFormat::Png);
        let webp = process_avatar(small).await.unwrap();
        let img = image::load_from_memory(&webp).unwrap();
        assert_eq!((img.width(), img.height()), (100, 80));
    }

    #[tokio::test]
    async fn rejects_non_image() {
        let err = process_avatar(Bytes::from_static(b"not an image"))
            .await
            .unwrap_err();
        assert!(err.to_string().to_lowercase().contains("image"));
    }

    #[tokio::test]
    async fn rejects_oversized_upload() {
        let too_big = Bytes::from(vec![0u8; MAX_AVATAR_UPLOAD_BYTES + 1]);
        assert!(process_avatar(too_big).await.is_err());
    }

    #[tokio::test]
    async fn rejects_oversized_dimensions() {
        let huge = encode(MAX_SOURCE_DIMENSION + 1, 10, ImageFormat::Png);
        assert!(process_avatar(huge).await.is_err());
    }
}
