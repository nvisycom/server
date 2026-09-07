#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod client;
mod error;
pub mod provider;

pub use client::{
    AuthenticatedProvider, ChatTurn, InferenceClient, LlmConfig, Role, TokenStream,
    UnauthenticatedProvider,
};
pub use error::{Error, Result};
