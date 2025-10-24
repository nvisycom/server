# api.nvisy.com/mistral

Mistral AI OCR client for high-accuracy document understanding and text
extraction with async support and comprehensive error handling.

[![rust](https://img.shields.io/badge/Rust-1.89+-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![mistral](https://img.shields.io/badge/Mistral-OCR%20API-000000?style=flat-square&logo=ai&logoColor=white)](https://mistral.ai/)

## Features

- **State-of-the-Art OCR Processing** - Advanced document understanding with
  unprecedented accuracy and cognition
- **Structured Data Extraction** - Preserves document layout, tables, images,
  and mathematical formulas in ordered format
- **Multi-format Support** - Process PDFs up to 1000 pages and images up to 50MB
  with high accuracy
- **Async/Await Support** - Non-blocking HTTP requests with Tokio runtime
- **Multilingual Recognition** - Native support for 109+ languages including
  complex scripts (Arabic, Hindi, Chinese)
- **Mathematical Content** - Advanced handling of equations, LaTeX formatting,
  and scientific notation
- **Error Handling** - Comprehensive error types with API response context
- **Interleaved Output** - Ordered text and image extraction in Markdown format

## Key Dependencies

- `reqwest` - HTTP client for making API requests to Mistral OCR
- `tokio` - Async runtime for non-blocking operations
- `serde` - JSON serialization/deserialization for API payloads
- `serde_json` - JSON handling for OCR responses and structured data

## Mistral OCR Capabilities

The crate provides cloud-based OCR processing with industry-leading accuracy:

### Input Processing

- **PDF Documents** - Up to 1000 pages, 50MB maximum file size
- **Images** - PNG, JPEG, TIFF, and other common image formats
- **Scanned Documents** - 98.96% accuracy on digitized paper documents
- **Complex Layouts** - Scientific papers, charts, graphs, and equations
- **Multilingual Content** - 109+ languages with native script support

### Output Formats

- **Interleaved Markdown** - Text and images in ordered document structure
- **Structured JSON** - Hierarchical document representation with metadata
- **Table Extraction** - Preserved table structure and relationships
- **Mathematical Notation** - LaTeX-compatible equation extraction
- **Image References** - Embedded images with contextual positioning

The Mistral OCR API integration provides enterprise-grade document processing
with world-class accuracy, enabling intelligent document analysis and automated
redaction workflows across multiple languages and complex document formats.
