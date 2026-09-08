#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod client;
mod error;
pub mod oauth;
pub mod provider;

pub use client::{ConnectedFileService, FileService, FreshToken, OAuthApps, PickerAccessToken};
pub use error::{Error, ErrorKind, Result};
