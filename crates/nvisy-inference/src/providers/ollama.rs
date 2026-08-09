//! Ollama provider, backed by rig's Ollama client.
//!
//! Ollama is typically self-hosted and unauthenticated, so it carries no
//! credentials — only a server base URL.

use derive_more::Deref;
use rig::client::Nothing;
use rig::providers::ollama;

use super::{Client, OllamaCredentials};
use crate::error::Error;

/// Ollama-backed inference client.
#[derive(Deref)]
pub struct OllamaProvider(ollama::Client);

impl Client for OllamaProvider {
    type Credentials = OllamaCredentials;

    const ID: &str = "ollama";

    fn connect(_credentials: &Self::Credentials, base_url: Option<&str>) -> Result<Self, Error> {
        let mut builder = ollama::Client::builder().api_key(Nothing);
        if let Some(base) = base_url {
            builder = builder.base_url(base);
        }
        let client = builder
            .build()
            .map_err(|err| Error::Build(err.to_string()))?;
        Ok(Self(client))
    }
}
