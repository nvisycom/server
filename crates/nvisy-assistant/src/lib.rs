#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod client;
mod error;
pub mod provider;

pub use client::{
    AssistantClient, AssistantConfig, AuthenticatedProvider, ChatTurn, LlmConfig, Role,
    TokenStream, UnauthenticatedProvider,
};
pub use error::{Error, Result};
