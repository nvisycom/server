# nvisy-worker

Background workers for document processing pipeline.

## Overview

Workers subscribe to NATS JetStream subjects and process document jobs concurrently:

- `PreprocessingWorker` - Runs on file upload (OCR, thumbnails, embeddings)
- `ProcessingWorker` - Runs on edit requests (VLM-based transformations)  
- `PostprocessingWorker` - Runs on download (format conversion, compression)

## Usage

```rust,ignore
use nvisy_worker::{WorkerConfig, WorkerHandles, WorkerState};

// Create config and state
let config = WorkerConfig::new(pg_config, nats_config);
let state = WorkerState::from_config(&config, inference_service).await?;

// Spawn all workers
let workers = WorkerHandles::spawn(&state, &config);

// Graceful shutdown
workers.shutdown();
workers.wait_all().await?;
```
