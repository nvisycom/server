//! PaddleX HTTP client implementation.

use std::path::Path;

use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

use crate::{Error, PADDLEOCR_VL_TARGET, PADDLEX_TARGET, PdConfig, Result};

/// HTTP client for PaddleX services.
///
/// This client provides methods for interacting with PaddleOCR-VL and other
/// PaddleX pipelines via their HTTP APIs.
///
/// # Examples
///
/// ```ignore
/// use nvisy_paddle::{PdClient, PdConfig};
///
/// #[tokio::main]
/// async fn main() -> Result<(), nvisy_paddle::Error> {
///     let config = PdConfig::new("http://localhost:8080")?;
///     let client = PdClient::new(config)?;
///
///     // Parse a document
///     let result = client.parse_document("document.pdf").await?;
///     println!("Content: {}", result.content);
///
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone)]
pub struct PdClient {
    /// HTTP client
    http_client: Client,

    /// Configuration
    config: PdConfig,
}

impl PdClient {
    /// Create a new PaddleX client with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Client configuration
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use nvisy_paddle::{PdClient, PdConfig};
    ///
    /// let config = PdConfig::new("http://localhost:8080")?;
    /// let client = PdClient::new(config)?;
    /// ```
    pub fn new(config: PdConfig) -> Result<Self> {
        let mut client_builder = Client::builder()
            .timeout(config.timeout())
            .user_agent(config.user_agent())
            .danger_accept_invalid_certs(!config.verify_ssl());

        // Add default headers
        let mut headers = reqwest::header::HeaderMap::new();
        for (key, value) in config.custom_headers() {
            let header_name = reqwest::header::HeaderName::from_bytes(key.as_bytes())
                .map_err(|e| Error::config(format!("Invalid header name '{}': {}", key, e)))?;
            let header_value = reqwest::header::HeaderValue::from_str(value)
                .map_err(|e| Error::config(format!("Invalid header value '{}': {}", value, e)))?;
            headers.insert(header_name, header_value);
        }

        // Add API key to headers if present
        if let Some(api_key) = config.api_key() {
            headers.insert(
                reqwest::header::AUTHORIZATION,
                reqwest::header::HeaderValue::from_str(&format!("Bearer {}", api_key))
                    .map_err(|e| Error::config(format!("Invalid API key: {}", e)))?,
            );
        }

        if !headers.is_empty() {
            client_builder = client_builder.default_headers(headers);
        }

        let http_client = client_builder
            .build()
            .map_err(|e| Error::config(format!("Failed to build HTTP client: {}", e)))?;

        debug!(
            target: PADDLEX_TARGET,
            base_url = %config.base_url(),
            timeout = ?config.timeout(),
            "PaddleX client initialized"
        );

        Ok(Self {
            http_client,
            config,
        })
    }

    /// Get a reference to the client configuration.
    pub fn config(&self) -> &PdConfig {
        &self.config
    }

    /// Parse a document using PaddleOCR-VL.
    ///
    /// This method sends a document to the PaddleOCR-VL service for parsing
    /// and returns structured data including text, tables, formulas, etc.
    ///
    /// # Arguments
    ///
    /// * `file_path` - Path to the document file (PDF, image, etc.)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let result = client.parse_document("invoice.pdf").await?;
    /// println!("Extracted text: {}", result.content);
    /// ```
    pub async fn parse_document(&self, file_path: impl AsRef<Path>) -> Result<OcrResult> {
        let file_path = file_path.as_ref();

        info!(
            target: PADDLEOCR_VL_TARGET,
            path = ?file_path,
            "Parsing document with PaddleOCR-VL"
        );

        // Read file contents
        let file_bytes = tokio::fs::read(file_path).await.map_err(|e| {
            Error::Io(std::io::Error::new(
                e.kind(),
                format!("Failed to read file '{}': {}", file_path.display(), e),
            ))
        })?;

        // Determine MIME type from file extension
        let mime_type = self.get_mime_type(file_path)?;

        self.parse_document_bytes(&file_bytes, mime_type.as_str())
            .await
    }

    /// Parse a document from bytes using PaddleOCR-VL.
    ///
    /// # Arguments
    ///
    /// * `file_bytes` - Raw bytes of the document
    /// * `mime_type` - MIME type of the document (e.g., "application/pdf", "image/png")
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let bytes = std::fs::read("document.pdf")?;
    /// let result = client.parse_document_bytes(&bytes, "application/pdf").await?;
    /// ```
    pub async fn parse_document_bytes(
        &self,
        file_bytes: &[u8],
        mime_type: &str,
    ) -> Result<OcrResult> {
        let url = self
            .config
            .base_url()
            .join("/api/v1/ocr/parse")
            .map_err(|e| Error::config(format!("Failed to construct API URL: {}", e)))?;

        debug!(
            target: PADDLEOCR_VL_TARGET,
            url = %url,
            size = file_bytes.len(),
            mime_type = mime_type,
            "Sending document to PaddleOCR-VL"
        );

        // Store bytes as owned Vec for retry logic
        let bytes = file_bytes.to_vec();
        let mime_type = mime_type.to_string();

        self.execute_with_retry(bytes, mime_type, url).await
    }

    /// Execute a request with automatic retry on retryable errors.
    async fn execute_with_retry(
        &self,
        bytes: Vec<u8>,
        mime_type: String,
        url: url::Url,
    ) -> Result<OcrResult> {
        let mut attempt = 0;
        let max_retries = self.config.max_retries();

        loop {
            // Build multipart form for each attempt
            let form = reqwest::multipart::Form::new().part(
                "file",
                reqwest::multipart::Part::bytes(bytes.clone())
                    .mime_str(&mime_type)
                    .map_err(|e| {
                        Error::config(format!("Invalid MIME type '{}': {}", mime_type, e))
                    })?,
            );

            // Send request
            let result = async {
                let response = self
                    .http_client
                    .post(url.clone())
                    .multipart(form)
                    .send()
                    .await?;

                self.handle_response(response).await
            }
            .await;

            match result {
                Ok(ocr_result) => {
                    if attempt > 0 {
                        info!(
                            target: PADDLEX_TARGET,
                            attempt = attempt + 1,
                            "Request succeeded after retry"
                        );
                    }
                    return Ok(ocr_result);
                }
                Err(e) if e.is_retryable() && attempt < max_retries => {
                    attempt += 1;
                    let backoff = self.config.retry_backoff() * attempt;

                    warn!(
                        target: PADDLEX_TARGET,
                        attempt = attempt,
                        max_retries = max_retries,
                        backoff_ms = backoff.as_millis(),
                        error = %e,
                        "Request failed, retrying"
                    );

                    // Use retry_after if provided by the error
                    let delay = e.retry_after().unwrap_or(backoff);
                    tokio::time::sleep(delay).await;
                }
                Err(e) => {
                    error!(
                        target: PADDLEX_TARGET,
                        attempt = attempt + 1,
                        error = %e,
                        "Request failed permanently"
                    );
                    return Err(e);
                }
            }
        }
    }

    /// Handle HTTP response and convert to result.
    async fn handle_response(&self, response: reqwest::Response) -> Result<OcrResult> {
        let status = response.status();

        debug!(
            target: PADDLEX_TARGET,
            status = status.as_u16(),
            "Received response from PaddleX"
        );

        // Check for successful response
        if status.is_success() {
            let result: ApiResponse<OcrResult> = response.json().await.map_err(|e| {
                Error::invalid_response(format!("Failed to parse success response: {}", e), None)
            })?;

            if result.success {
                Ok(result.data.unwrap_or_else(|| {
                    warn!(target: PADDLEX_TARGET, "API returned success but no data");
                    OcrResult::default()
                }))
            } else {
                Err(Error::api(
                    status.as_u16(),
                    result
                        .message
                        .unwrap_or_else(|| "Unknown error".to_string()),
                    result.code,
                ))
            }
        } else {
            // Try to parse error response
            let body_text = response.text().await.ok();

            let error = match status {
                StatusCode::TOO_MANY_REQUESTS => {
                    // Try to extract retry-after header
                    let retry_after = None; // Could parse from headers
                    Error::rate_limit("Rate limit exceeded", retry_after)
                }
                StatusCode::SERVICE_UNAVAILABLE | StatusCode::BAD_GATEWAY => {
                    Error::service_unavailable("Service temporarily unavailable", None)
                }
                StatusCode::REQUEST_TIMEOUT | StatusCode::GATEWAY_TIMEOUT => {
                    Error::timeout(self.config.timeout())
                }
                _ => {
                    // Try to parse structured error
                    if let Some(body) = &body_text {
                        if let Ok(api_error) = serde_json::from_str::<ApiResponse<()>>(body) {
                            Error::api(
                                status.as_u16(),
                                api_error
                                    .message
                                    .unwrap_or_else(|| "Unknown error".to_string()),
                                api_error.code,
                            )
                        } else {
                            Error::api(status.as_u16(), body.clone(), None)
                        }
                    } else {
                        Error::api(status.as_u16(), status.to_string(), None)
                    }
                }
            };

            Err(error)
        }
    }

    /// Determine MIME type from file path.
    fn get_mime_type(&self, path: &Path) -> Result<String> {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .ok_or_else(|| Error::invalid_input("File has no extension"))?;

        let mime_type = match extension.to_lowercase().as_str() {
            "pdf" => "application/pdf",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "tiff" | "tif" => "image/tiff",
            "bmp" => "image/bmp",
            "gif" => "image/gif",
            "webp" => "image/webp",
            ext => {
                return Err(Error::unsupported(format!(
                    "Unsupported file extension: {}",
                    ext
                )));
            }
        };

        Ok(mime_type.to_string())
    }

    /// Health check for the PaddleX service.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// if client.health_check().await.is_ok() {
    ///     println!("Service is healthy");
    /// }
    /// ```
    pub async fn health_check(&self) -> Result<()> {
        let url =
            self.config.base_url().join("/health").map_err(|e| {
                Error::config(format!("Failed to construct health check URL: {}", e))
            })?;

        debug!(target: PADDLEX_TARGET, url = %url, "Performing health check");

        let response = self.http_client.get(url).send().await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(Error::service_unavailable(
                format!("Health check failed with status {}", response.status()),
                None,
            ))
        }
    }
}

/// Generic API response structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiResponse<T> {
    /// Whether the request was successful
    success: bool,

    /// Optional response data
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,

    /// Optional error message
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,

    /// Optional error code
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
}

/// Result from PaddleOCR-VL document parsing.
///
/// Contains the extracted content and metadata from the parsed document.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OcrResult {
    /// Extracted text content (could be Markdown, JSON, or plain text)
    pub content: String,

    /// Content format (e.g., "markdown", "json", "text")
    #[serde(default = "default_format")]
    pub format: String,

    /// Number of pages processed (for multi-page documents)
    #[serde(default)]
    pub num_pages: u32,

    /// Processing time in milliseconds
    #[serde(default)]
    pub processing_time_ms: u64,

    /// Detected language(s)
    #[serde(default)]
    pub languages: Vec<String>,

    /// Confidence score (0.0 to 1.0)
    #[serde(default)]
    pub confidence: f32,

    /// Additional metadata
    #[serde(default)]
    pub metadata: serde_json::Value,
}

fn default_format() -> String {
    "text".to_string()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn test_mime_type_detection() {
        let config = PdConfig::new("http://localhost:8080").unwrap();
        let client = PdClient::new(config).unwrap();

        assert_eq!(
            client.get_mime_type(Path::new("test.pdf")).unwrap(),
            "application/pdf"
        );
        assert_eq!(
            client.get_mime_type(Path::new("image.png")).unwrap(),
            "image/png"
        );
        assert_eq!(
            client.get_mime_type(Path::new("photo.jpg")).unwrap(),
            "image/jpeg"
        );
        assert_eq!(
            client.get_mime_type(Path::new("photo.JPEG")).unwrap(),
            "image/jpeg"
        );

        assert!(client.get_mime_type(Path::new("file.txt")).is_err());
        assert!(client.get_mime_type(Path::new("noextension")).is_err());
    }

    #[test]
    fn test_client_creation() {
        let config = PdConfig::new("http://localhost:8080")
            .unwrap()
            .with_timeout(Duration::from_secs(60))
            .with_api_key("test-key");

        let client = PdClient::new(config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_client_with_invalid_config() {
        let config = PdConfig::new("http://localhost:8080")
            .unwrap()
            .with_header("Invalid\nHeader", "value");

        let result = PdClient::new(config);
        assert!(result.is_err());
    }
}
