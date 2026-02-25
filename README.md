# Nvisy Server

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/server/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/server/actions/workflows/build.yml)

Open-source multimodal redaction API. Detect and redact PII and sensitive data
across documents, images, audio, and video.

## Features

- **Multimodal Redaction:** Detect and remove sensitive data across PDFs, images, audio, and video
- **AI-Powered Detection:** LLM-driven PII and entity recognition with configurable redaction policies
- **Workspace Isolation:** Multi-tenant workspaces with HKDF-derived credential encryption
- **Real-Time Collaboration:** WebSocket and NATS pub/sub for live document editing
- **Interactive Docs:** Auto-generated OpenAPI with Scalar UI

## Quick Start

The fastest way to get started is with [Nvisy Cloud](https://nvisy.com).

For self-hosted deployments, refer to [`docker/`](docker/) for compose files and
infrastructure requirements, and [`.env.example`](.env.example) for configuration.

## Documentation

See [`docs/`](docs/) for architecture, intelligence capabilities, provider
design, and security documentation.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](LICENSE.txt)

## Support

- **Documentation:** [docs.nvisy.com](https://docs.nvisy.com)
- **Issues:** [GitHub Issues](https://github.com/nvisycom/server/issues)
- **Email:** [support@nvisy.com](mailto:support@nvisy.com)
- **API Status:** [nvisy.openstatus.dev](https://nvisy.openstatus.dev)
