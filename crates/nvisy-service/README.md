# nvisy-core

Core abstractions and shared types for AI services in the Nvisy platform.

[![Rust](https://img.shields.io/badge/Rust-1.89+-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)

## Features

- **Service Abstractions** - Provider-agnostic traits for OCR, VLM, and embeddings
- **Shared Types** - Documents, messages, annotations, and health monitoring
- **Error Handling** - Comprehensive error types with retry policies
- **Mock Providers** - Test utilities for AI service testing

## Key Dependencies

- `bytes` - Efficient byte buffer management for document content
- `uuid` - Type-safe identifiers for requests and entities
- `jiff` - Precision timestamps for processing metadata
- `serde` - Serialization for all data structures

## Architecture

- **ocr** - Optical character recognition service abstractions
- **vlm** - Vision language model traits for multimodal AI
- **emb** - Embedding service interfaces for vectorization
- **types** - Shared data structures (documents, messages, annotations)
- **mock** - Mock implementations for testing (requires `test-utils` feature)
