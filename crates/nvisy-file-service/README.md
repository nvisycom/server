# nvisy-file-service

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/server/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/server/actions/workflows/build.yml)

OAuth-based cloud file-service providers for the Nvisy platform.

## Overview

Connects a workspace to a tenant's consumer file service — Google Drive,
Dropbox, OneDrive, and Box — behind a single `FileServiceClient`. It owns
the OAuth2 authorization-code flow (with refresh), driven through a shared
`reqwest` client, and the per-provider REST calls to list and stream files
in and out. Sibling to `nvisy-object-store`, which covers object stores
(S3, Azure, GCS) with static credentials.

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
