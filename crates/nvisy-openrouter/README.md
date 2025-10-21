# api.nvisy.com/openrouter

OpenRouter AI client for intelligent document processing and analysis with async support and comprehensive error handling.

[![rust](https://img.shields.io/badge/Rust-1.89+-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![openrouter](https://img.shields.io/badge/OpenRouter-AI%20API-000000?style=flat-square&logo=openai&logoColor=white)](https://openrouter.ai/)

## Features

- **AI Model Access** - Integration with OpenRouter's unified AI model API
- **Async/Await Support** - Non-blocking HTTP requests with Tokio runtime
- **Multiple Models** - Support for various AI models through OpenRouter platform
- **Request/Response Types** - Type-safe structures for AI completions and chat
- **Error Handling** - Comprehensive error types with API response context
- **Streaming Support** - Real-time streaming responses for large AI outputs

## Key Dependencies

- `reqwest` - HTTP client for making API requests to OpenRouter
- `tokio` - Async runtime for non-blocking operations
- `serde` - JSON serialization/deserialization for API payloads
- `serde_json` - JSON handling for AI model responses

