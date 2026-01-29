# Nvisy Server

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/server/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/server/actions/workflows/build.yml)

Open-source ETL platform for building intelligent data pipelines with pluggable sources, AI-powered transforms, and configurable sinks.

## Features

- **Workflow Pipelines** — Declarative DAG-based workflows compiled to optimized execution graphs
- **Pluggable Providers** — Uniform interface for databases, object stores, vector DBs, and more
- **AI-Native Transforms** — Extraction, enrichment, embedding, entity resolution, and analysis as pipeline nodes
- **Resumable Streaming** — Incremental processing with per-item pagination context
- **Encrypted Connections** — Workspace-isolated credential encryption with HKDF-derived keys
- **Interactive Docs** — Auto-generated OpenAPI with Scalar UI

## Quick Start

```bash
# Install tools and make scripts executable
make install-all

# Generate keys, env, and migration files
make generate-all

# Start infrastructure (PostgreSQL + NATS)
docker compose -f docker/docker-compose.dev.yml up -d

# Start the server
cargo run --features dotenv --bin nvisy-server
```

## Configuration

See [.env.example](.env.example) for all available environment variables.

## API Documentation

- Scalar UI: `http://localhost:8080/api/scalar`
- OpenAPI JSON: `http://localhost:8080/api/openapi.json`

## Docker

See [`docker/`](docker/) for development and production compose files, infrastructure requirements, and configuration reference.

## Documentation

See [`docs/`](docs/) for architecture, intelligence capabilities, provider design, and security documentation.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License — see [LICENSE.txt](LICENSE.txt)

## Support

- **Issues**: [GitHub Issues](https://github.com/nvisycom/server/issues)
- **Email**: [support@nvisy.com](mailto:support@nvisy.com)
