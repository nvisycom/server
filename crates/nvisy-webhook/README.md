# nvisy-webhook

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
