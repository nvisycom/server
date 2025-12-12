# api.nvisy.com/portkey

Portkey AI Gateway client for intelligent document processing and analysis with async
support and comprehensive error handling.

[![rust](https://img.shields.io/badge/Rust-1.89+-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![portkey](https://img.shields.io/badge/Portkey-AI%20Gateway-000000?style=flat-square&logo=openai&logoColor=white)](https://portkey.ai/)

## Features

- **AI Gateway Integration** - Unified access to 200+ AI providers through Portkey
- **Async/Await Support** - Non-blocking HTTP requests with Tokio runtime
- **Multiple Providers** - Support for various AI providers through Portkey Gateway
- **Request/Response Types** - Type-safe structures for AI completions and chat
- **Error Handling** - Comprehensive error types with API response context
- **Streaming Support** - Real-time streaming responses for large AI outputs
- **Caching** - Built-in caching support via Portkey's gateway features
- **Observability** - Request tracking with trace IDs and structured logging

## Key Dependencies

- `portkey-sdk` - Official Portkey SDK for AI Gateway integration
- `tokio` - Async runtime for non-blocking operations
- `serde` - JSON serialization/deserialization for API payloads
- `serde_json` - JSON handling for AI model responses

## Quick Start

### Basic Usage

```rust
use nvisy_portkey::{LlmClient, Result};

#[tokio::main]
async fn main() -> Result<()> {
    // Simple client with just an API key
    let client = LlmClient::from_api_key("your-portkey-api-key")?;
    
    // Or with API key and virtual key for routing
    let client = LlmClient::from_keys(
        "your-portkey-api-key",
        "your-virtual-key"
    )?;
    
    Ok(())
}
```

### Advanced Configuration

```rust
use nvisy_portkey::{LlmConfig, Result};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
    let client = LlmConfig::builder()
        .with_api_key("your-portkey-api-key")
        .with_virtual_key("your-virtual-key")
        .with_default_model("gpt-4")
        .with_request_timeout(Duration::from_secs(60))
        .with_trace_id("custom-trace-id")
        .with_cache_namespace("my-app")
        .build_client()?;
    
    Ok(())
}
```

## Configuration Options

The `LlmConfig` builder supports the following options:

- `with_api_key(key)` - **Required**: Your Portkey API key
- `with_virtual_key(key)` - Virtual key for routing requests to specific providers
- `with_base_url(url)` - Custom API endpoint (default: `https://api.portkey.ai/v1`)
- `with_request_timeout(duration)` - Request timeout (default: 30 seconds, max: 300 seconds)
- `with_default_model(model)` - Default model name for completions
- `with_default_max_tokens(tokens)` - Default maximum tokens for responses
- `with_default_temperature(temp)` - Temperature for randomness (0.0-2.0)
- `with_default_presence_penalty(penalty)` - Presence penalty (-2.0 to 2.0)
- `with_default_frequency_penalty(penalty)` - Frequency penalty (-2.0 to 2.0)
- `with_default_top_p(value)` - Top-p sampling (0.001-1.0)
- `with_trace_id(id)` - Custom trace ID for request tracking
- `with_cache_namespace(namespace)` - Cache namespace for scoping
- `with_cache_force_refresh(bool)` - Force cache refresh

## Environment Variables

Alternatively, configure the client using environment variables:

```bash
export PORTKEY_API_KEY="your-portkey-api-key"
export PORTKEY_VIRTUAL_KEY="your-virtual-key"
export PORTKEY_BASE_URL="https://api.portkey.ai/v1"
export PORTKEY_TIMEOUT_SECS="60"
export PORTKEY_TRACE_ID="custom-trace"
export PORTKEY_CACHE_NAMESPACE="my-app"
export PORTKEY_CACHE_FORCE_REFRESH="false"
```

Then create a client from the environment:

```rust
use portkey_sdk::PortkeyClient;

let client = PortkeyClient::from_env()?;
```

## Examples

### Direct Access to Portkey SDK

For advanced use cases, access the underlying Portkey SDK client:

```rust
use nvisy_portkey::LlmClient;

let client = LlmClient::from_api_key("your-api-key")?;

// Access the underlying Portkey client
let portkey_client = client.as_client();

// Use portkey_client for direct SDK operations
```

### Error Handling

```rust
use nvisy_portkey::{LlmClient, Error};

match LlmClient::from_api_key("your-api-key") {
    Ok(client) => println!("Client created successfully"),
    Err(Error::Config(e)) => println!("Configuration error: {}", e),
    Err(Error::Api(e)) => println!("API error: {}", e),
    Err(e) => println!("Other error: {}", e),
}
```

## Architecture

The crate is organized into several modules:

- `client` - Core client implementation and configuration
- `completion` - Chat completion and request/response types
- `typed` - Type-safe serialization/deserialization utilities

## Security

- API keys are masked in debug output to prevent accidental leakage
- All configuration validation happens at build time
- Secure defaults are applied for timeouts and rate limits

## License

MIT
