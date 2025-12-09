# api.nvisy.com/paddle

PaddleX HTTP API client for comprehensive document processing and OCR with
low-code AI development capabilities and multi-hardware support.

[![rust](https://img.shields.io/badge/Rust-1.89+-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![paddlex](https://img.shields.io/badge/PaddleX-3.0+-000000?style=flat-square&logo=paddle&logoColor=white)](https://github.com/PaddlePaddle/PaddleX)

## Features

- **Comprehensive Model Pipeline** - Access to 200+ pre-trained models across 33
  model pipelines via HTTP API
- **Advanced Document Understanding** - PaddleOCR-VL and PP-StructureV3
  integration for complex document layouts
- **Multilingual OCR Processing** - PP-OCRv5 support for 37+ languages including
  French, Spanish, Portuguese, Russian, and Korean
- **Intelligent Information Extraction** - PP-ChatOCRv4 with ERNIE 4.5 Turbo for
  context-aware document analysis
- **Async/Await Support** - Non-blocking HTTP requests with Tokio runtime
- **Multi-Hardware Support** - NVIDIA, Kunlun, Ascend, and Cambricon hardware
  acceleration
- **Error Handling** - Comprehensive error types with API response context
- **Low-Code Development** - Unified command interface for rapid AI integration

## Key Dependencies

- `reqwest` - HTTP client for making API requests to PaddleX service
- `tokio` - Async runtime for non-blocking operations
- `serde` - JSON serialization/deserialization for API payloads
- `serde_json` - JSON handling for structured OCR and analysis results

## PaddleX 3.0 Integration

The crate provides access to PaddleX's comprehensive AI model ecosystem:

### Model Capabilities

- **PaddleOCR-VL** - 0.9B vision-language model for 109+ language support
- **PP-OCRv5** - Universal scene text recognition with 13% accuracy improvement
- **PP-StructureV3** - Complex document structure analysis and conversion
- **PP-ChatOCRv4** - Intelligent information extraction with 15% accuracy boost
- **PP-DocTranslation** - Document translation with structure preservation

### Output Formats

- **Structured JSON** - Hierarchical document representation
- **Markdown Conversion** - Document-to-Markdown with preserved formatting
- **Table Data** - CSV and structured table extraction
- **Multi-modal Results** - Text, images, charts, and formula recognition

The PaddleX integration provides enterprise-grade document processing with
low-code development capabilities, supporting rapid AI application development
and deployment across various hardware platforms.
