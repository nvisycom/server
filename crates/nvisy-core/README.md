# api.nvisy.com/core

Foundational abstractions for AI services in the Nvisy ecosystem with generic
traits for OCR and VLM implementations.

[![rust](https://img.shields.io/badge/Rust-1.89+-000000?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![crates.io](https://img.shields.io/crates/v/nvisy-core.svg?style=flat-square)](https://crates.io/crates/nvisy-core)
[![docs.rs](https://img.shields.io/badge/docs.rs-nvisy--core-000000?style=flat-square)](https://docs.rs/nvisy-core)

## Features

- **Provider Agnostic** - Abstract interfaces that work with any AI service provider
- **Generic Traits** - Type-safe abstractions with generic request/response handling
- **Cheap Cloning** - Services wrapped in `Arc` for efficient sharing across tasks
- **Streaming Support** - Built-in streaming for OCR and VLM responses

## Key Dependencies

- `uuid` - Type-safe identifiers for requests, responses, and context tracking
- `jiff` - High-precision timestamps for processing metadata
- `base64` - Encoding for binary data payloads

## Architecture

The crate provides specialized modules for AI service abstractions:

- **OCR** - Generic Optical Character Recognition with `Ocr<Req, Resp>` trait
- **VLM** - Vision Language Model trait for image analysis
