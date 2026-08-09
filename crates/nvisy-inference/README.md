# nvisy-inference

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/server/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/server/actions/workflows/build.yml)

LLM inference provider configuration and clients for the Nvisy platform.

## Overview

A provider-agnostic inference layer over
[`rig`](https://crates.io/crates/rig), supporting OpenAI, Ollama, and
Anthropic behind a single provider-tagged `LlmConfig`. The config carries each
provider's credentials and is stored encrypted at rest by a workspace
connection; `validate` proves those credentials work by verifying them against
the provider, backing the connection test endpoint.

## Documentation

See [`docs/`](../../docs/) for architecture, security, and API documentation.

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation**: [docs.nvisy.com](https://docs.nvisy.com)
- **Issues**: [GitHub Issues](https://github.com/nvisycom/server/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
- **API Status**: [nvisy.openstatus.dev](https://nvisy.openstatus.dev)
