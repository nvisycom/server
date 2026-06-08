# nvisy-webhook

Webhook delivery types and traits for the Nvisy platform.

## Overview

Defines the `WebhookProvider` trait for delivery implementations and the
`WebhookService` wrapper that adds observability. Application events such
as pipeline completion are delivered as HMAC-SHA256 signed HTTP callbacks.
For an HTTP-based client implementation, see the `nvisy-reqwest` crate.

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
