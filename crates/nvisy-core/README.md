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
use nvisy_core::{Error, ErrorKind, Result, ServiceHealth, ServiceStatus};
```

## Features

- `schema` - Enable JSON Schema derives for API documentation

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License - see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/server/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
- **API Status**: [nvisy.openstatus.dev](https://nvisy.openstatus.dev)
