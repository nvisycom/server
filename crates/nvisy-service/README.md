# nvisy-service

Core service abstractions and shared types for the Nvisy platform.

## Features

- **Inference Services** - Provider-agnostic traits for OCR, VLM, and embeddings
- **Webhook Services** - Webhook delivery abstractions with health checks
- **Shared Types** - Documents, messages, annotations, and health monitoring
- **Error Handling** - Comprehensive error types with context support
- **Mock Providers** - Test utilities for service testing (requires `test-utils` feature)

## Modules

- **inference** - AI inference abstractions (embeddings, OCR, VLM)
- **webhook** - Webhook delivery traits and service wrappers
- **types** - Shared data structures (health, timing)
- **prelude** - Common imports for convenience

## Usage

```rust
use nvisy_service::prelude::*;

// Or import specific modules
use nvisy_service::inference::{InferenceProvider, InferenceService};
use nvisy_service::webhook::{WebhookProvider, WebhookService};
use nvisy_service::types::{ServiceHealth, ServiceStatus};
```

## Key Dependencies

- `bytes` - Efficient byte buffer management for document content
- `uuid` - Type-safe identifiers for requests and entities
- `jiff` - Precision timestamps for processing metadata
- `serde` - Serialization for all data structures
- `async-trait` - Async trait support for providers
