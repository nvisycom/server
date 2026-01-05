# nvisy-inference

AI inference abstractions for embeddings, OCR, and vision language models.

## Features

- **Embeddings**: Generate vector embeddings from text, documents, and chats
- **OCR**: Extract text from images and documents
- **VLM**: Vision-language model operations for multimodal AI

## Usage

```rust,ignore
use nvisy_inference::{
    InferenceProvider, InferenceService, SharedContext,
    EmbeddingRequest, OcrRequest, VlmRequest,
};

// Create a unified service with a provider
let service = InferenceService::from_provider(my_provider);
let context = SharedContext::new();

// Use individual methods
let embedding = service.generate_embedding(&context, &embedding_request).await?;
let ocr_result = service.process_ocr(&context, &ocr_request).await?;
let vlm_result = service.process_vlm(&context, &vlm_request).await?;
```
