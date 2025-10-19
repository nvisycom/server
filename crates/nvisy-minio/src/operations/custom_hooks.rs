//! Includes all callbacks and hooks for [`minio`].

use std::sync::Arc;

use crate::operations::{DownloadResult, UploadResult};
use crate::types::{DownloadContext, UploadContext};
use crate::{Error, Result};

/// Hook called before and after upload operations.
pub trait UploadHook: Send + Sync {
    /// Called before starting an upload operation.
    async fn on_upload_start(&self, context: &UploadContext) -> Result<()> {
        let _ = context;
        Ok(())
    }

    /// Called after a successful upload operation.
    async fn on_upload_success(
        &self,
        context: &UploadContext,
        result: &UploadResult,
    ) -> Result<()> {
        let _ = (context, result);
        Ok(())
    }

    /// Called after a failed upload operation.
    async fn on_upload_error(&self, context: &UploadContext, error: &Error) -> Result<()> {
        let _ = (context, error);
        Ok(())
    }
}

/// Hook called before and after download operations.
pub trait DownloadHook: Send + Sync {
    /// Called before starting a download operation.
    async fn on_download_start(&self, context: &DownloadContext) -> Result<()> {
        let _ = context;
        Ok(())
    }

    /// Called after a successful download operation.
    async fn on_download_success(
        &self,
        context: &DownloadContext,
        result: &DownloadResult,
    ) -> Result<()> {
        let _ = (context, result);
        Ok(())
    }

    /// Called after a failed download operation.
    async fn on_download_error(&self, context: &DownloadContext, error: &Error) -> Result<()> {
        let _ = (context, error);
        Ok(())
    }
}

/// Hook manager that handles all registered hooks.
#[derive(Default, Clone)]
pub struct HookManager {
    upload_hooks: Arc<Vec<Arc<dyn UploadHook>>>,
    download_hooks: Arc<Vec<Arc<dyn DownloadHook>>>,
}

impl HookManager {
    /// Creates a new hook manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an upload hook.
    pub fn add_upload_hook(mut self, hook: Arc<dyn UploadHook>) -> Self {
        let mut hooks = (*self.upload_hooks).clone();
        hooks.push(hook);
        self.upload_hooks = Arc::new(hooks);
        self
    }

    /// Adds a download hook.
    pub fn add_download_hook(mut self, hook: Arc<dyn DownloadHook>) -> Self {
        let mut hooks = (*self.download_hooks).clone();
        hooks.push(hook);
        self.download_hooks = Arc::new(hooks);
        self
    }

    /// Executes all upload start hooks.
    pub async fn execute_upload_start(&self, context: &UploadContext) -> Result<()> {
        for hook in self.upload_hooks.iter() {
            hook.on_upload_start(context).await?;
        }
        Ok(())
    }

    /// Executes all upload success hooks.
    pub async fn execute_upload_success(
        &self,
        context: &UploadContext,
        result: &UploadResult,
    ) -> Result<()> {
        for hook in self.upload_hooks.iter() {
            hook.on_upload_success(context, result).await?;
        }
        Ok(())
    }

    /// Executes all upload error hooks.
    pub async fn execute_upload_error(&self, context: &UploadContext, error: &Error) -> Result<()> {
        for hook in self.upload_hooks.iter() {
            if let Err(hook_error) = hook.on_upload_error(context, error).await {
                tracing::warn!(
                    target: crate::TRACING_TARGET_OPERATIONS,
                    error = %hook_error,
                    "Upload error hook failed"
                );
            }
        }
        Ok(())
    }

    /// Executes all download start hooks.
    pub async fn execute_download_start(&self, context: &DownloadContext) -> Result<()> {
        for hook in self.download_hooks.iter() {
            hook.on_download_start(context).await?;
        }
        Ok(())
    }

    /// Executes all download success hooks.
    pub async fn execute_download_success(
        &self,
        context: &DownloadContext,
        result: &DownloadResult,
    ) -> Result<()> {
        for hook in self.download_hooks.iter() {
            hook.on_download_success(context, result).await?;
        }
        Ok(())
    }

    /// Executes all download error hooks.
    pub async fn execute_download_error(
        &self,
        context: &DownloadContext,
        error: &Error,
    ) -> Result<()> {
        for hook in self.download_hooks.iter() {
            if let Err(hook_error) = hook.on_download_error(context, error).await {
                tracing::warn!(
                    target: crate::TRACING_TARGET_OPERATIONS,
                    error = %hook_error,
                    "Download error hook failed"
                );
            }
        }
        Ok(())
    }
}

impl std::fmt::Debug for HookManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HookManager")
            .field(
                "upload_hooks",
                &format!("{} hooks", self.upload_hooks.len()),
            )
            .field(
                "download_hooks",
                &format!("{} hooks", self.download_hooks.len()),
            )
            .finish()
    }
}
