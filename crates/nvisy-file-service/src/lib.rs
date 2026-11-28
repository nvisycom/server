#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod client;
pub mod oauth;
pub mod provider;

pub use client::{ConnectedFileService, FileService, FreshToken, OAuthApps, PickerAccessToken};
pub use nvisy_core::{Error, ErrorKind, Result};
