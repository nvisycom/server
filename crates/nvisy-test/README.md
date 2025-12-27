# nvisy-test

Test utilities and mock implementations for nvisy crates.

## Overview

This crate provides mock implementations of AI services defined in `nvisy-core` for use in unit and integration tests.

## Mock Providers

- `MockEmbeddingProvider` - Returns default embedding responses
- `MockOpticalProvider` - Returns default OCR responses  
- `MockLanguageProvider` - Returns default VLM responses

All mock providers implement health checks that return healthy status.

## Features

- `config` - Enables clap derives for configuration types
