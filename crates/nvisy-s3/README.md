# nvisy-s3

First-party S3-compatible blob storage for Nvisy: uploaded files, detection
audits, enrichment intermediates, redacted output, and avatars.

A single S3 bucket holds every object; a logical `Bucket` (files, audits,
artifacts, account/workspace avatars) maps to a key prefix within it. Backed by
the AWS S3 SDK, so it works against AWS S3 or any S3-compatible server (`RustFS`,
`MinIO`, Cloudflare R2, …) selected by endpoint.

Objects are encrypted by the caller before they reach the store, so the store
only ever holds ciphertext.

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
