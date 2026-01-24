# nvisy-core

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/server/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/server/actions/workflows/build.yml)

Core types and utilities shared across nvisy crates.

## Overview

This crate provides foundational types used by other nvisy crates:

- **Error types**: Common error handling with `Error`, `ErrorKind`, and `Result`
- **Health types**: Service health status for monitoring
- **Timing**: Request/response timing utilities

## Usage

```rust
use nvisy_core::{Error, ErrorKind, Result};
use nvisy_core::types::{ServiceHealth, ServiceStatus};
```

## Features

- `schema` - Enable JSON Schema derives for API documentation
