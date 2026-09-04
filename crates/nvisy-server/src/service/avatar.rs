//! Avatar service: stores, serves, and removes account and workspace avatars.
//!
//! Uploads are normalized to WebP (decoded, bounded, resized, re-encoded) and
//! stored unencrypted in the blob store, keyed by `(owner, content hash)`
//! so each version is its own object and the serve URL can be cached immutably.
//! Because the stored bytes are always WebP, the serve `Content-Type` is
//! constant and no per-object mime is persisted; re-encoding also strips
//! EXIF/metadata.

use std::io::Cursor;

use image::{ImageFormat, ImageReader};
use nvisy_postgres::model::{Account, UpdateAccount, UpdateWorkspace};
use nvisy_postgres::query::{AccountRepository, WorkspaceRepository};
use nvisy_s3::{AccountAvatarKey, GetObject, WorkspaceAvatarKey};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::handler::{ErrorKind, Result};
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

    /// Normalizes and stores an account avatar as a new content-versioned object,
    /// deletes the previous version, and points the account's `avatar_url` at it.
    /// Returns the updated account.
    ///
    /// The stored object is keyed by `(account_id, content hash)` and the URL
    /// embeds the same hash, so a versioned URL maps to immutable bytes. The new
    /// object is written and the URL updated before the previous version is
    /// deleted, so a reader never sees a missing avatar; a crash between the
    /// update and the delete can leave the previous object orphaned (see #192).
    pub async fn set_account_avatar(&self, account_id: Uuid, upload: Vec<u8>) -> Result<Account> {
        let webp = process_avatar(upload).await?;
        let version = content_version(&webp);

        self.infra
            .blobs
            .put(
                &AccountAvatarKey::new(account_id, &version),
                Cursor::new(webp),
            )
            .await?;

        let mut conn = self.infra.postgres.get_connection().await?;
        let previous = conn.find_account_by_id(account_id).await?;

        let avatar_url = format!("/avatars/accounts/{account_id}/{version}/");
        let account = conn
            .update_account(
                account_id,
                UpdateAccount {
                    avatar_url: Some(Some(avatar_url)),
                    ..Default::default()
                },
            )
            .await?;

        if let Some(old_version) = previous.and_then(|a| avatar_version(a.avatar_url.as_deref()))
            && old_version != version
        {
            self.infra
                .blobs
                .delete(&AccountAvatarKey::new(account_id, old_version))
                .await?;
        }

        Ok(account)
    }

    /// Streams the account avatar for a specific version, or `None` if absent.
    pub async fn account_avatar(&self, account_id: Uuid, version: &str) -> Result<Option<Vec<u8>>> {
        read_object(
            self.infra
                .blobs
                .get(&AccountAvatarKey::new(account_id, version))
                .await?,
        )
        .await
    }

    /// Removes an account's current avatar object and clears its `avatar_url`.
    pub async fn delete_account_avatar(&self, account_id: Uuid) -> Result<()> {
        let mut conn = self.infra.postgres.get_connection().await?;
        let account = conn.find_account_by_id(account_id).await?;

        if let Some(version) = account.and_then(|a| avatar_version(a.avatar_url.as_deref())) {
            self.infra
                .blobs
                .delete(&AccountAvatarKey::new(account_id, version))
                .await?;
        }

        conn.update_account(
            account_id,
            UpdateAccount {
                avatar_url: Some(None),
                ..Default::default()
            },
        )
        .await?;
        Ok(())
    }

    /// Normalizes and stores a workspace avatar as a new content-versioned
    /// object, deletes the previous version, and points the workspace's
    /// `avatar_url` at it.
    ///
    /// The stored object is keyed by `(workspace_id, content hash)` and the URL
    /// embeds the same hash, so a versioned URL maps to immutable bytes. The new
    /// object is written and the URL updated before the previous version is
    /// deleted, so a reader never sees a missing avatar; a crash between the
    /// update and the delete can leave the previous object orphaned (see #192).
    pub async fn set_workspace_avatar(&self, workspace_id: Uuid, upload: Vec<u8>) -> Result<()> {
        let webp = process_avatar(upload).await?;
        let version = content_version(&webp);

        self.infra
            .blobs
            .put(
                &WorkspaceAvatarKey::new(workspace_id, &version),
                Cursor::new(webp),
            )
            .await?;

        let mut conn = self.infra.postgres.get_connection().await?;
        let previous = conn.find_workspace_by_id(workspace_id).await?;

        let avatar_url = format!("/avatars/workspaces/{workspace_id}/{version}/");
        conn.update_workspace(
            workspace_id,
            UpdateWorkspace {
                avatar_url: Some(Some(avatar_url)),
                ..Default::default()
            },
        )
        .await?;

        if let Some(old_version) = previous.and_then(|w| avatar_version(w.avatar_url.as_deref()))
            && old_version != version
        {
            self.infra
                .blobs
                .delete(&WorkspaceAvatarKey::new(workspace_id, old_version))
                .await?;
        }

        Ok(())
    }

    /// Streams the workspace avatar for a specific version, or `None` if absent.
    pub async fn workspace_avatar(
        &self,
        workspace_id: Uuid,
        version: &str,
    ) -> Result<Option<Vec<u8>>> {
        read_object(
            self.infra
                .blobs
                .get(&WorkspaceAvatarKey::new(workspace_id, version))
                .await?,
        )
        .await
    }

    /// Removes a workspace's current avatar object and clears its `avatar_url`.
    pub async fn delete_workspace_avatar(&self, workspace_id: Uuid) -> Result<()> {
        let mut conn = self.infra.postgres.get_connection().await?;
        let workspace = conn.find_workspace_by_id(workspace_id).await?;

        if let Some(version) = workspace.and_then(|w| avatar_version(w.avatar_url.as_deref())) {
            self.infra
                .blobs
                .delete(&WorkspaceAvatarKey::new(workspace_id, version))
                .await?;
        }

        conn.update_workspace(
            workspace_id,
            UpdateWorkspace {
                avatar_url: Some(None),
                ..Default::default()
            },
        )
        .await?;
        Ok(())
    }
}

/// Reads a stored object's bytes into memory, or `None` when absent. Avatars are
/// small (bounded by the target dimension), so buffering is fine.
async fn read_object(result: Option<GetObject>) -> Result<Option<Vec<u8>>> {
    use tokio::io::AsyncReadExt;

    let Some(stored) = result else {
        return Ok(None);
    };
    let mut reader = stored.into_reader();
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).await.map_err(|err| {
        ErrorKind::InternalServerError
            .with_message("Failed to read avatar")
            .with_context(err.to_string())
    })?;
    Ok(Some(buf))
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
async fn process_avatar(bytes: Vec<u8>) -> Result<Vec<u8>> {
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
    fn encode(width: u32, height: u32, format: ImageFormat) -> Vec<u8> {
        let img = DynamicImage::ImageRgba8(RgbaImage::new(width, height));
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, format).unwrap();
        out.into_inner()
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
        let err = process_avatar(b"not an image".to_vec()).await.unwrap_err();
        assert!(err.to_string().to_lowercase().contains("image"));
    }

    #[tokio::test]
    async fn rejects_oversized_upload() {
        let too_big = vec![0u8; MAX_AVATAR_UPLOAD_BYTES + 1];
        assert!(process_avatar(too_big).await.is_err());
    }

    #[tokio::test]
    async fn rejects_oversized_dimensions() {
        let huge = encode(MAX_SOURCE_DIMENSION + 1, 10, ImageFormat::Png);
        assert!(process_avatar(huge).await.is_err());
    }
}
