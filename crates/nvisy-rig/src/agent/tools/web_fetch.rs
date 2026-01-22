//! Web fetch tool for retrieving content from URLs.

use std::sync::Arc;

use async_trait::async_trait;
use rig::completion::ToolDefinition;
use rig::tool::Tool;
use serde::{Deserialize, Serialize};

/// Error type for web fetch operations.
#[derive(Debug, thiserror::Error)]
pub enum WebFetchError {
    #[error("request failed: {0}")]
    Request(String),
    #[error("invalid URL: {0}")]
    InvalidUrl(String),
    #[error("timeout")]
    Timeout,
    #[error("content too large: {size} bytes (max: {max})")]
    ContentTooLarge { size: usize, max: usize },
    #[error("unsupported content type: {0}")]
    UnsupportedContentType(String),
}

/// Arguments for web fetch.
#[derive(Debug, Deserialize)]
pub struct WebFetchArgs {
    /// The URL to fetch.
    pub url: String,
    /// Maximum content size in bytes.
    #[serde(default = "default_max_size")]
    pub max_size: usize,
    /// Whether to extract text only (strip HTML).
    #[serde(default = "default_extract_text")]
    pub extract_text: bool,
    /// Timeout in seconds.
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_max_size() -> usize {
    1_000_000 // 1MB
}

fn default_extract_text() -> bool {
    true
}

fn default_timeout() -> u64 {
    30
}

/// Result of a web fetch operation.
#[derive(Debug, Serialize)]
pub struct WebFetchResult {
    /// The fetched content.
    pub content: String,
    /// The content type.
    pub content_type: Option<String>,
    /// The final URL (after redirects).
    pub final_url: String,
    /// Content length in bytes.
    pub length: usize,
    /// Page title if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// Trait for fetching web content.
///
/// Implementations should handle HTTP requests, redirects, and content extraction.
#[async_trait]
pub trait WebFetcher: Send + Sync {
    /// Fetches content from a URL.
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to fetch
    /// * `max_size` - Maximum content size in bytes
    /// * `timeout_secs` - Request timeout in seconds
    ///
    /// # Returns
    ///
    /// The fetched content as bytes, the final URL, and the content type.
    async fn fetch(
        &self,
        url: &str,
        max_size: usize,
        timeout_secs: u64,
    ) -> Result<FetchResponse, WebFetchError>;
}

/// Raw response from a web fetch operation.
#[derive(Debug)]
pub struct FetchResponse {
    /// The raw content bytes.
    pub bytes: bytes::Bytes,
    /// The final URL after redirects.
    pub final_url: String,
    /// The content type header value.
    pub content_type: Option<String>,
}

/// Tool for fetching web content.
///
/// This tool uses a pluggable `WebFetcher` implementation for making HTTP requests.
pub struct WebFetchTool<F> {
    fetcher: Arc<F>,
    max_size: usize,
}

impl<F: WebFetcher> WebFetchTool<F> {
    /// Creates a new web fetch tool.
    pub fn new(fetcher: F) -> Self {
        Self {
            fetcher: Arc::new(fetcher),
            max_size: default_max_size(),
        }
    }

    /// Creates a new web fetch tool with a shared fetcher.
    pub fn with_arc(fetcher: Arc<F>) -> Self {
        Self {
            fetcher,
            max_size: default_max_size(),
        }
    }

    /// Creates a new web fetch tool with custom max size.
    pub fn with_max_size(fetcher: F, max_size: usize) -> Self {
        Self {
            fetcher: Arc::new(fetcher),
            max_size,
        }
    }

    /// Extracts text content from HTML.
    fn extract_text_from_html(html: &str) -> (String, Option<String>) {
        // Simple HTML text extraction
        // In production, you might want to use a proper HTML parser like scraper

        // Extract title
        let title = html.find("<title>").and_then(|start| {
            let start = start + 7;
            html[start..]
                .find("</title>")
                .map(|end| html[start..start + end].trim().to_string())
        });

        // Remove script and style tags
        let mut text = html.to_string();

        // Remove script tags
        while let Some(start) = text.find("<script") {
            if let Some(end) = text[start..].find("</script>") {
                text = format!("{}{}", &text[..start], &text[start + end + 9..]);
            } else {
                break;
            }
        }

        // Remove style tags
        while let Some(start) = text.find("<style") {
            if let Some(end) = text[start..].find("</style>") {
                text = format!("{}{}", &text[..start], &text[start + end + 8..]);
            } else {
                break;
            }
        }

        // Remove all HTML tags
        let mut result = String::new();
        let mut in_tag = false;
        for c in text.chars() {
            match c {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => result.push(c),
                _ => {}
            }
        }

        // Decode common HTML entities
        let result = result
            .replace("&nbsp;", " ")
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&#39;", "'");

        // Normalize whitespace
        let result: String = result.split_whitespace().collect::<Vec<_>>().join(" ");

        (result, title)
    }
}

impl<F: WebFetcher + 'static> Tool for WebFetchTool<F> {
    type Args = WebFetchArgs;
    type Error = WebFetchError;
    type Output = WebFetchResult;

    const NAME: &'static str = "web_fetch";

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Fetch content from a URL. Can retrieve web pages, APIs, or other HTTP resources. Optionally extracts text from HTML.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {
                        "type": "string",
                        "description": "The URL to fetch"
                    },
                    "max_size": {
                        "type": "integer",
                        "description": "Maximum content size in bytes (default: 1MB)",
                        "default": 1000000
                    },
                    "extract_text": {
                        "type": "boolean",
                        "description": "Extract text only from HTML (default: true)",
                        "default": true
                    },
                    "timeout_secs": {
                        "type": "integer",
                        "description": "Request timeout in seconds (default: 30)",
                        "default": 30
                    }
                },
                "required": ["url"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let max_size = args.max_size.min(self.max_size);

        let response = self
            .fetcher
            .fetch(&args.url, max_size, args.timeout_secs)
            .await?;

        let content = String::from_utf8_lossy(&response.bytes).to_string();
        let length = content.len();

        let is_html = response
            .content_type
            .as_ref()
            .map(|ct| ct.contains("text/html"))
            .unwrap_or(false)
            || content.trim_start().starts_with("<!DOCTYPE")
            || content.trim_start().starts_with("<html");

        let (content, title) = if args.extract_text && is_html {
            Self::extract_text_from_html(&content)
        } else {
            (content, None)
        };

        Ok(WebFetchResult {
            content,
            content_type: response.content_type,
            final_url: response.final_url,
            length,
            title,
        })
    }
}
