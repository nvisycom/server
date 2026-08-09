//! OpenAI provider, backed by rig's OpenAI client.
//!
//! Works with OpenAI and any OpenAI-compatible endpoint (Azure OpenAI, a proxy).

use derive_more::Deref;
use rig::providers::openai;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Client;
use crate::error::Error;

/// OpenAI API credentials.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct OpenAiCredentials {
    /// OpenAI API key.
    pub api_key: String,
}

/// OpenAI-backed inference client.
#[derive(Deref)]
pub struct OpenAiProvider(openai::Client);

impl Client for OpenAiProvider {
    type Credentials = OpenAiCredentials;

    const ID: &str = "openai";

    fn connect(credentials: &Self::Credentials, base_url: Option<&str>) -> Result<Self, Error> {
        let mut builder = openai::Client::builder().api_key(&credentials.api_key);
        if let Some(base) = base_url {
            builder = builder.base_url(base);
        }
        let client = builder
            .build()
            .map_err(|err| Error::Build(err.to_string()))?;
        Ok(Self(client))
    }
}
