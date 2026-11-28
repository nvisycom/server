#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc = include_str!("../README.md")]

mod client;
mod error;

pub use client::{
    AssistantClient, AssistantConfig, AuthenticatedProvider, ChatTurn, LlmConfig, Role,
    UnauthenticatedProvider,
};
pub use error::{Error, Result};
