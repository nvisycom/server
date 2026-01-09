# nvisy-reqwest

HTTP client for webhook delivery with HMAC-SHA256 request signing and
configurable timeouts.

[![Rust](https://img.shields.io/badge/Rust-1.89+-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Reqwest](https://img.shields.io/badge/Reqwest-0.12+-000000?style=flat-square&logo=rust&logoColor=white)](https://docs.rs/reqwest/)

## Features

- **Webhook Delivery** - HTTP POST delivery with structured payloads and headers
- **Request Signing** - HMAC-SHA256 payload signing for webhook authentication
- **Configurable Timeouts** - Global and per-request timeout configuration
- **Custom Headers** - Support for custom HTTP headers per request
- **Dependency Injection** - Converts to `WebhookService` for use with DI patterns
- **Observability** - Structured tracing for request/response monitoring

## Key Dependencies

- `reqwest` - HTTP client with async support and connection pooling
- `hmac` / `sha2` - Cryptographic signing for webhook payloads
- `nvisy-webhook` - Webhook traits and types for service abstraction

## Usage

```rust,ignore
use nvisy_reqwest::{ReqwestClient, ReqwestConfig};
use nvisy_webhook::{WebhookRequest, WebhookService};

// Create a client with default configuration
let client = ReqwestClient::default();

// Convert to a service for dependency injection
let service: WebhookService = client.into_service();

// Or use directly for webhook delivery
let request = WebhookRequest::new(url, event, payload, webhook_id, workspace_id);
let response = client.deliver(&request).await?;
```
