//! OpenAI provider, backed by rig's OpenAI client.
//!
//! Works with OpenAI and any OpenAI-compatible endpoint (Azure OpenAI, a proxy).

use derive_more::Deref;
use rig::providers::openai;

use super::Client;
use crate::error::{Error, Result};

/// OpenAI-backed inference client.
#[derive(Deref)]
pub struct OpenAiProvider(openai::Client);

impl Client for OpenAiProvider {
    /// The OpenAI API key.
    type Credentials = str;

    const ID: &str = "openai";

    fn connect(api_key: &Self::Credentials, base_url: Option<&str>) -> Result<Self> {
        let mut builder = openai::Client::builder().api_key(api_key);
        if let Some(base) = base_url {
            builder = builder.base_url(base);
        }
        let client = builder
            .build()
            .map_err(|err| Error::Build(err.to_string()))?;
        Ok(Self(client))
    }
}
