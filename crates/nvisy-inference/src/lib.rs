#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod client;
mod error;
pub mod providers;

pub use client::{ChatTurn, InferenceClient, Role, TokenStream, verify};
pub use error::Error;
