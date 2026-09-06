#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

pub mod client;
mod error;
pub mod oauth;
pub mod providers;

pub use client::{CloudFileService, ConnectedFileService, OAuthApps};
pub use error::{Error, ErrorKind, Result};
