//! Request structures for API token operations.
//!
//! This module provides request DTOs for API token management including
//! creation and updates.

use std::net::IpAddr;
use std::time::Duration;

use nvisy_postgres::model::NewAccountApiToken;
use nvisy_postgres::types::ApiTokenType;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::handler::Result;
use crate::handler::tokens::ip_to_net;

/// Expiration options for API tokens.
#[must_use]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum TokenExpiration {
    /// Token never expires.
    Never,
    /// Expires in 7 days.
    #[default]
    In7Days,
    /// Expires in 30 days.
    In30Days,
    /// Expires in 90 days.
    In90Days,
    /// Expires in 1 year.
    In1Year,
}

impl TokenExpiration {
    /// Returns the duration until expiration, or None if never expires.
    pub fn to_span(self) -> Option<jiff::Span> {
        match self {
            Self::Never => None,
            Self::In7Days => Some(jiff::Span::new().days(7)),
            Self::In30Days => Some(jiff::Span::new().days(30)),
            Self::In90Days => Some(jiff::Span::new().days(90)),
            Self::In1Year => Some(jiff::Span::new().days(365)),
        }
    }

    /// Returns the expiry timestamp from now, or None if never expires.
    pub fn to_expiry_timestamp(self) -> Option<jiff::Timestamp> {
        self.to_span().map(|span| jiff::Timestamp::now() + span)
    }

    /// Returns the duration until expiration, or None if never expires.
    pub fn to_duration(self) -> Option<Duration> {
        match self {
            Self::Never => None,
            Self::In7Days => Some(Duration::from_secs(7 * 24 * 60 * 60)),
            Self::In30Days => Some(Duration::from_secs(30 * 24 * 60 * 60)),
            Self::In90Days => Some(Duration::from_secs(90 * 24 * 60 * 60)),
            Self::In1Year => Some(Duration::from_secs(365 * 24 * 60 * 60)),
        }
    }
}

/// Request to create a new API token.
#[derive(Debug, Clone, Serialize, Deserialize, Validate, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct CreateApiToken {
    /// Human-readable name for the API token (1-100 characters).
    #[validate(length(min = 1, max = 100))]
    pub name: String,

    /// Optional description for the API token (max 500 characters).
    #[validate(length(max = 500))]
    pub description: Option<String>,

    /// When the token expires.
    #[serde(default)]
    pub expires: TokenExpiration,
}

impl CreateApiToken {
    /// Converts this request into a [`NewAccountApiToken`] model.
    ///
    /// # Arguments
    ///
    /// * `account_id` - The account this token belongs to.
    /// * `ip_address` - The IP address of the client creating the token.
    /// * `user_agent` - The user agent string of the client.
    pub fn into_model(
        self,
        account_id: Uuid,
        ip_address: IpAddr,
        user_agent: String,
    ) -> Result<NewAccountApiToken> {
        let sanitized_name = self.name.trim().to_string();
        if sanitized_name.is_empty() {
            return Err(crate::handler::ErrorKind::BadRequest
                .with_resource("api_token")
                .with_message("Token name cannot be empty or whitespace only"));
        }

        let expires_at = self.expires.to_expiry_timestamp();
        let ip_net = ip_to_net(ip_address)?;

        Ok(NewAccountApiToken {
            account_id,
            name: sanitized_name,
            description: self.description,
            region_code: None,
            country_code: None,
            city_name: None,
            ip_address: ip_net,
            user_agent,
            device_id: None,
            session_type: Some(ApiTokenType::Api),
            is_remembered: Some(true),
            expired_at: expires_at.map(Into::into),
        })
    }
}

/// Request to update an existing API token.
#[derive(Debug, Clone, Serialize, Deserialize, Validate, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateApiToken {
    /// Updated name for the API token (1-100 characters).
    #[validate(length(min = 1, max = 100))]
    pub name: Option<String>,

    /// Updated description for the API token (max 500 characters).
    #[validate(length(max = 500))]
    pub description: Option<String>,
}
