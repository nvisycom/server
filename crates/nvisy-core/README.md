# Nvisy Core

[![Crates.io](https://img.shields.io/crates/v/nvisy-core.svg)](https://crates.io/crates/nvisy-core)
[![Documentation](https://docs.rs/nvisy-core/badge.svg)](https://docs.rs/nvisy-core)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

**Foundational abstractions for AI services in the Nvisy ecosystem.**

`nvisy-core` provides the core traits and types for Large Language Models (LLM), Optical Character Recognition (OCR), and Vision Language Models (VLM) without depending on any concrete implementations. This crate serves as the foundation that all AI service implementations build upon.

## Features

- 🎯 **Provider Agnostic** - Abstract interfaces that work with any AI service provider
- 🔒 **Type Safety** - Strong typing with comprehensive error handling
- ⚡ **Async First** - All operations are async-ready for non-blocking I/O
- 🔧 **Extensible** - Traits can be extended with provider-specific functionality
- 📊 **Observable** - Built-in support for logging and metrics collection
- 🚫 **Zero Dependencies** - No dependencies on concrete AI service implementations

## Quick Start

Add this to your `Cargo.toml`:

```toml
[dependencies]
nvisy-core = "0.1.0"
```

## Usage

### Basic Example

```rust
use nvisy_core::prelude::*;

// Use any service that implements the traits
async fn example<L, O, V>(llm: &L, ocr: &O, vlm: &V) -> Result<()>
where
    L: LlmService,
    O: OcrService,
    V: VlmService,
{
    // Generate text with LLM
    let llm_request = LlmRequest::completion("Explain quantum computing");
    let response = llm.generate(&llm_request).await?;
    println!("LLM: {}", response.text());

    // Extract text with OCR
    let image_data = bytes::Bytes::from("..."); // Your image data
    let ocr_request = OcrRequest::from_image(image_data, "image/png");
    let ocr_response = ocr.extract_text(&ocr_request).await?;
    println!("OCR: {}", ocr_response.text());

    // Analyze image with VLM
    let vlm_request = VlmRequest::describe_image(bytes::Bytes::from("..."), "image/jpeg");
    let vlm_response = vlm.process(&vlm_request).await?;
    println!("VLM: {}", vlm_response.text());

    Ok(())
}
```

### Large Language Models (LLM)

```rust
use nvisy_core::llm::*;
use nvisy_core::error::LlmResult;

async fn llm_example<T: LlmService>(service: &T) -> LlmResult<()> {
    // Simple text completion
    let request = LlmRequest::completion("Write a haiku about programming")
        .with_model("gpt-4")
        .with_max_tokens(100)
        .with_temperature(0.8);

    let response = service.generate(&request).await?;
    println!("Generated: {}", response.text());

    // Chat conversation
    let messages = vec![
        ChatMessage::system("You are a helpful programming assistant"),
        ChatMessage::user("How do I implement error handling in Rust?"),
    ];
    
    let chat_request = LlmRequest::chat(messages)
        .with_model("gpt-4")
        .with_max_tokens(500);

    let chat_response = service.generate(&chat_request).await?;
    println!("Assistant: {}", chat_response.text());

    // Streaming response
    use futures_util::StreamExt;
    
    let stream_request = LlmRequest::completion("Tell me a story")
        .with_streaming(true);
        
    let mut stream = service.generate_stream(&stream_request).await?;
    while let Some(chunk) = stream.next().await {
        let response = chunk?;
        print!("{}", response.text());
    }

    Ok(())
}
```

### Optical Character Recognition (OCR)

```rust
use nvisy_core::ocr::*;
use nvisy_core::error::OcrResult;
use bytes::Bytes;

async fn ocr_example<T: OcrService>(service: &T) -> OcrResult<()> {
    let image_data = Bytes::from("..."); // Your image data

    // Basic text extraction
    let request = OcrRequest::from_image(image_data.clone(), "image/png")
        .with_language("en")
        .with_confidence_threshold(0.8);

    let response = service.extract_text(&request).await?;
    println!("Extracted: {}", response.text());
    println!("Confidence: {}", response.confidence());

    // Structured extraction with layout
    let structured_request = OcrRequest::from_image(image_data, "image/png")
        .with_output_format(OutputFormat::FullLayout)
        .with_table_detection(true)
        .with_bounding_boxes(true);

    let structured_response = service.extract_structured(&structured_request).await?;
    
    if let Some(structured) = structured_response.structured() {
        println!("Found {} text blocks", structured.blocks.len());
        println!("Found {} tables", structured.tables.len());
    }

    Ok(())
}
```

### Vision Language Models (VLM)

```rust
use nvisy_core::vlm::*;
use nvisy_core::error::VlmResult;
use bytes::Bytes;

async fn vlm_example<T: VlmService>(service: &T) -> VlmResult<()> {
    let image_data = Bytes::from("..."); // Your image data

    // Image description
    let request = VlmRequest::describe_image(image_data.clone(), "image/jpeg")
        .with_detail_level(DetailLevel::High)
        .with_max_tokens(500);

    let response = service.process(&request).await?;
    println!("Description: {}", response.text());

    // Visual question answering
    let vqa_request = VlmRequest::visual_qa(
        image_data.clone(),
        "image/jpeg", 
        "What objects are visible in this image?"
    ).with_detail_level(DetailLevel::Medium);

    let vqa_response = service.process(&vqa_request).await?;
    println!("Answer: {}", vqa_response.text());

    // Object detection
    let detection_request = VlmRequest::detect_objects(image_data, "image/jpeg");
    let detection_response = service.process(&detection_request).await?;
    
    if let Some(objects) = detection_response.objects() {
        println!("Detected {} objects", objects.len());
        for obj in objects {
            println!("- {}: {:.2}% confidence", obj.class, obj.confidence * 100.0);
        }
    }

    Ok(())
}
```

## Error Handling

Each service has specific error types that provide detailed information about failures:

```rust
use nvisy_core::error::*;

fn handle_errors() {
    // Service-specific error handling
    let llm_error = LlmError::RateLimited { 
        retry_after: Some(std::time::Duration::from_secs(30)) 
    };
    
    match llm_error {
        LlmError::RateLimited { retry_after } => {
            println!("Rate limited, retry after: {:?}", retry_after);
        }
        LlmError::ContextLengthExceeded { current, max } => {
            println!("Context too long: {}/{} tokens", current, max);
        }
        _ => println!("Other LLM error: {}", llm_error),
    }

    // Unified error handling
    let unified_error: Error = llm_error.into();
    println!("Service: {}", unified_error.service_type());
    println!("Retryable: {}", unified_error.is_retryable());
    
    if let Some(delay) = unified_error.retry_delay() {
        println!("Retry after: {:?}", delay);
    }
}
```

## Architecture

The crate is organized into several modules:

### Core Modules

- **[`error`]** - Comprehensive error types for all AI operations
- **[`llm`]** - Large Language Model abstractions and types
- **[`ocr`]** - Optical Character Recognition abstractions and types  
- **[`vlm`]** - Vision Language Model abstractions and types
- **[`prelude`]** - Convenient re-exports for common usage

### Design Principles

1. **Trait-based Architecture** - All functionality is defined through traits
2. **Provider Agnostic** - No dependencies on specific AI service implementations
3. **Type Safety** - Strong typing prevents common errors at compile time
4. **Async Native** - Built for non-blocking, concurrent operations
5. **Comprehensive Error Handling** - Detailed error types with retry logic
6. **Observability** - Built-in tracing targets and metadata collection

## Implementing Services

To implement an AI service, create a struct that implements the appropriate trait:

```rust,no_run
use nvisy_core::{LlmService, LlmRequest, LlmResponse};
use nvisy_core::llm::{ModelInfo, ServiceHealth};
use nvisy_core::error::LlmResult;
use async_trait::async_trait;
use std::pin::Pin;
use futures_util::Stream;

pub struct MyLlmService {
    api_key: String,
    base_url: String,
}

#[async_trait]
impl LlmService for MyLlmService {
    async fn generate(&self, request: &LlmRequest) -> LlmResult<LlmResponse> {
        // Your implementation here
        todo!("Implement LLM generation")
    }

    async fn generate_stream(&self, request: &LlmRequest) -> LlmResult<Pin<Box<dyn Stream<Item = LlmResult<LlmResponse>> + Send>>> {
        // Your streaming implementation here
        todo!("Implement streaming generation")
    }

    async fn validate_request(&self, request: &LlmRequest) -> LlmResult<()> {
        // Validate the request
        Ok(())
    }

    async fn list_models(&self) -> LlmResult<Vec<ModelInfo>> {
        // Return available models
        todo!("List available models")
    }

    fn service_name(&self) -> &str {
        "MyLlmService"
    }

    async fn health_check(&self) -> LlmResult<ServiceHealth> {
        // Check service health
        todo!("Implement health check")
    }
}
```

## Tracing

The crate provides predefined tracing targets for consistent logging:

```rust,no_run
use tracing::info;
use nvisy_core::{TRACING_TARGET_LLM, TRACING_TARGET_OCR, TRACING_TARGET_VLM};

info!(target: TRACING_TARGET_LLM, "Processing LLM request");
info!(target: TRACING_TARGET_OCR, "Extracting text from image");  
info!(target: TRACING_TARGET_VLM, "Analyzing image content");
```

## Contributing

We welcome contributions! Please see our [Contributing Guidelines](../../CONTRIBUTING.md) for details.

## License

This project is licensed under the MIT License - see the [LICENSE](../../LICENSE.txt) file for details.