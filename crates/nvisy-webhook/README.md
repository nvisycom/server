# nvisy-webhook

Webhook delivery types and traits for nvisy services.

## Features

- `WebhookProvider`: Core trait for webhook delivery implementations
- `WebhookService`: Service wrapper with observability

For an HTTP-based client implementation, see the `nvisy-reqwest` crate.

## Usage

```rust
use nvisy_webhook::{WebhookPayload, WebhookProvider, WebhookService};

// Create a service from any WebhookProvider implementation
let service = WebhookService::new(my_provider);

// Create and send a webhook
let payload = WebhookPayload::test(webhook_id);
let request = payload.into_request("https://example.com/webhook");
let response = service.deliver(&request).await?;
```
