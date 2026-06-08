# nvisy-nats

Task-focused NATS client for the Nvisy platform with JetStream, KV, and
object storage.

## Overview

A type-safe wrapper around `async-nats` for the platform's messaging
needs. JetStream powers a unified stream for real-time updates and
durable background jobs, the KV store holds distributed state, and
object storage handles uploaded files. Generic parameters keep payloads
type-safe, with automatic reconnection and retry-aware error handling.

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
