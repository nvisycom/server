# api.nvisy.com/olmocr2

High-performance OCR (Optical Character Recognition) client for the Nvisy platform 
with OLMo v2 model integration and comprehensive document processing capabilities.

[![rust](https://img.shields.io/badge/Rust-1.89+-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![ocr](https://img.shields.io/badge/OCR-OLMo_v2-000000?style=flat-square&logo=ai&logoColor=white)](https://olmo.ai/)
[![reqwest](https://img.shields.io/badge/reqwest-0.12+-000000?style=flat-square&logo=rust&logoColor=white)](https://github.com/seanmonstar/reqwest)

## Features

- **OLMo v2 Integration** - Advanced text recognition using OLMo v2 models
- **Multi-Format Support** - Process images in PNG, JPEG, WebP, and PDF formats
- **Async Processing** - High-throughput document processing with tokio async runtime
- **Batch Operations** - Process multiple documents concurrently with automatic batching
- **Error Recovery** - Comprehensive retry logic with exponential backoff
- **Type Safety** - Strong typing for OCR requests and responses with validation
- **Streaming Results** - Real-time processing updates for large document batches

## Key Dependencies

- `reqwest` - HTTP client for OCR API communication with multipart upload support
- `tokio` - Async runtime for concurrent document processing
- `serde` - Type-safe serialization for OCR requests and responses
- `base64` - Image encoding for API transmission

## Architecture

The crate provides specialized modules for OCR workflows:

- **Client** - OCR service connection and authentication management
- **Models** - OLMo v2 model configuration and selection
- **Processing** - Document preprocessing and text extraction pipeline
- **Results** - Structured OCR results with confidence scoring and metadata

All modules maintain type safety through generic parameters and provide comprehensive
error handling for production OCR workflows.