# nvisy-webhook

[![Build](https://img.shields.io/github/actions/workflow/status/nvisycom/server/build.yml?branch=main&label=build%20%26%20test&style=flat-square)](https://github.com/nvisycom/server/actions/workflows/build.yml)

Webhook delivery types and traits for nvisy services.

## Features

- `WebhookProvider`: Core trait for webhook delivery implementations
- `WebhookService`: Service wrapper with observability

For an HTTP-based client implementation, see the `nvisy-reqwest` crate.

## Usage

```rust,ignore
use nvisy_webhook::{WebhookRequest, WebhookProvider, WebhookService};

// Create a service from any WebhookProvider implementation
let service = WebhookService::new(my_provider);

// Create and send a webhook request
let request = WebhookRequest::new(url, event, payload, webhook_id, workspace_id);
let response = service.deliver(&request).await?;
```

## Changelog

See [CHANGELOG.md](../../CHANGELOG.md) for release notes and version history.

## License

Apache 2.0 License, see [LICENSE.txt](../../LICENSE.txt)

## Support

- **Documentation:** [docs.nvisy.com](https://docs.nvisy.com)
- **Issues:** [GitHub Issues](https://github.com/nvisycom/server/issues)
- **Email:** [support@nvisy.com](mailto:support@nvisy.com)
- **API Status:** [nvisy.openstatus.dev](https://nvisy.openstatus.dev)
