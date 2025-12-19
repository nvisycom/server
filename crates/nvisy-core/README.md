# api.nvisy.com/core

Core abstractions and shared types for AI services in the Nvisy ecosystem. This crate provides foundational building blocks for Vision Language Models (VLMs), Optical Character Recognition (OCR), embedding services, and document processing without depending on concrete implementations.

[![rust](https://img.shields.io/badge/Rust-1.89+-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![crates.io](https://img.shields.io/crates/v/nvisy-core.svg?style=flat-square)](https://crates.io/crates/nvisy-core)
[![docs.rs](https://img.shields.io/badge/docs.rs-nvisy--core-000000?style=flat-square)](https://docs.rs/nvisy-core)

## Overview

Nvisy Core serves as the foundation for building AI-powered applications by providing:

- **Service Abstractions** - Provider-agnostic traits for OCR, VLM, and embedding services
- **Shared Data Types** - Common types for documents, messages, annotations, and health monitoring
- **Error Handling** - Comprehensive error types with retry policies and structured messaging
- **Type Safety** - Strong typing with validation and builder patterns throughout

## Architecture

### Core Modules

- **`ocr`** - Optical Character Recognition service abstractions with request/response types
- **`vlm`** - Vision Language Model traits for multimodal AI interactions  
- **`emb`** - Embedding service interfaces for text and image vectorization
- **`types`** - Shared data structures including documents, messages, annotations, and health monitoring

### Design Principles

- **Provider Agnostic** - Work with any AI service provider through common interfaces
- **Memory Efficient** - Use `bytes::Bytes` for zero-copy data handling and `Arc` for cheap cloning
- **Comprehensive Error Handling** - Structured errors with optional messages and retry guidance
- **Observable** - Built-in tracing support with dedicated targets for each service type

## Key Features

### Document Processing
```rust
use nvisy_core::types::Document;
use bytes::Bytes;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let doc = Document::builder()
    .text_content("Hello, world!")
    .content_type("text/plain")
    .filename("greeting.txt")
    .attribute("source", "user_input")
    .build()?;
# Ok(())
# }
```

### Service Health Monitoring
```rust
use nvisy_core::types::{ServiceHealth, ServiceStatus};
use std::time::Duration;
use serde_json::json;

let health = ServiceHealth::healthy()
    .with_response_time(Duration::from_millis(150))
    .with_metric("requests_per_second", json!(42.5));
```

### Error Handling with Context
```rust
use nvisy_core::ocr::Error;

let error = Error::authentication()
    .with_message("API key expired or invalid");
```

## Dependencies

Core dependencies that enable the abstractions:

- **`bytes`** - Efficient byte buffer management for document content
- **`uuid`** - Type-safe identifiers for requests, responses, and entities
- **`jiff`** - Precision timestamps for processing metadata and health checks
- **`serde`** - Serialization support for all data structures
- **`thiserror`** - Ergonomic error types with structured information

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
nvisy-core = "0.1.0"
```

The crate is designed to be extended by concrete implementations while providing a consistent interface across different AI service providers.

## Service Integration

Each service module provides:

1. **Service Trait** - Core interface for service implementations
2. **Request/Response Types** - Structured data for service communication  
3. **Error Types** - Comprehensive error handling with retry policies
4. **Context Management** - Configuration and metadata handling

This design enables seamless switching between providers while maintaining type safety and comprehensive error handling throughout your application.