# nvisy-nats

[![Crates.io](https://img.shields.io/crates/v/nvisy-nats.svg)](https://crates.io/crates/nvisy-nats)
[![Documentation](https://docs.rs/nvisy-nats/badge.svg)](https://docs.rs/nvisy-nats)

Task-focused NATS client for the Nvisy platform.

## Overview

`nvisy-nats` provides a minimal, well-encapsulated NATS client with specialized modules for common use cases:

- **Client**: Connection management and configuration
- **KV**: Key-Value store for sessions and caching (NATS KV)
- **Stream**: Real-time updates via JetStream for WebSocket broadcasting
- **Queue**: Distributed job queues for background processing

## Features

- Simple connection management with automatic reconnection
- NATS KV for session management and caching
- JetStream for real-time event streaming
- Work queues for distributed job processing
- Access to underlying NATS client for extensibility
- Comprehensive error handling with retry logic

## Usage

### Connecting to NATS

```rust
use nvisy_nats::{NatsClient, NatsConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create configuration
    let config = NatsConfig::new("nats://localhost:4222")
        .with_name("my-service");

    // Connect to NATS
    let client = NatsClient::connect(config).await?;

    // Use the client
    client.ping().await?;

    Ok(())
}
```

### Session Management

```rust
use nvisy_nats::{SessionStore, UserSession, DeviceInfo};
use std::time::Duration;

async fn manage_sessions(client: &NatsClient) -> Result<(), Box<dyn std::error::Error>> {
    // Create session store
    let sessions = SessionStore::new(
        client.jetstream(),
        Some(Duration::from_secs(86400)) // 24 hours
    ).await?;

    // Create a new session
    let session = UserSession::new(
        user_id,
        "session_123".to_string(),
        DeviceInfo::from_user_agent(user_agent),
        "192.168.1.1".to_string(),
        user_agent.to_string(),
        Duration::from_secs(86400),
    );

    sessions.create("session_123", &session).await?;

    // Retrieve session
    if let Some(session) = sessions.get("session_123").await? {
        println!("Session valid for user: {}", session.user_id);
    }

    Ok(())
}
```

### Caching

```rust
use nvisy_nats::CacheStore;

async fn cache_data(client: &NatsClient) -> Result<(), Box<dyn std::error::Error>> {
    // Create cache store
    let cache = CacheStore::new(
        client.jetstream(),
        "users",
        Some(Duration::from_secs(3600)) // 1 hour TTL
    ).await?;

    // Cache a value
    cache.set("user:123", &user_data).await?;

    // Get from cache or compute
    let user = cache.get_or_compute("user:456", || async {
        fetch_user_from_db(456).await
    }).await?;

    Ok(())
}
```

### Real-time Updates (WebSocket)

```rust
use nvisy_nats::{StreamPublisher, UpdateEvent, UpdateType};

async fn publish_updates(client: &NatsClient) -> Result<(), Box<dyn std::error::Error>> {
    // Create stream publisher
    let publisher = StreamPublisher::new(client.jetstream(), "updates").await?;

    // Publish document progress update
    let event = UpdateEvent::new(UpdateType::DocumentProgress {
        document_id: doc_id,
        user_id: user_id,
        percentage: 50,
        stage: "Processing".to_string(),
        estimated_completion: None,
    });

    publisher.publish(&event).await?;

    Ok(())
}
```

### Job Queue

```rust
use nvisy_nats::{JobQueue, Job, JobType, JobPriority};

async fn process_jobs(client: &NatsClient) -> Result<(), Box<dyn std::error::Error>> {
    // Create job queue
    let queue = JobQueue::new(client.jetstream(), "documents", "worker-1").await?;

    // Submit a job
    let job = Job::new(JobType::DocumentProcessing, payload)
        .with_priority(JobPriority::High)
        .with_timeout(Duration::from_secs(300));

    queue.submit(&job).await?;

    // Create worker and process jobs
    let consumer = queue.create_worker(&[JobType::DocumentProcessing]).await?;

    queue.process_next(&consumer, |job| async move {
        // Process the job
        process_document(job.payload).await
    }).await?;

    Ok(())
}
```

## License

MIT
