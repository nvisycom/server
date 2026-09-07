//! The client layer: the DI entry point, its config, and the connected client.
//!
//! [`FileService`] is the entry point and dependency-injected service;
//! [`FileServiceClient`] is the provider-neutral surface the sync engine drives.

mod apps;
mod config;
mod connected;
mod service;

pub use self::apps::OAuthApps;
pub use self::config::{
    BoxConfig, DropboxConfig, GoogleDriveConfig, OAuthAppsConfig, OneDriveConfig,
};
pub use self::connected::ConnectedFileService;
pub use self::service::{ByteStream, FileService, FileServiceClient};
