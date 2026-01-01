//! Request types for all inference operations.
//!
//! This module provides request types for embedding, OCR, and VLM operations,
//! supporting both single and batch processing. It also includes context types
//! for managing processing sessions across different provider types.

use std::collections::{HashMap, HashSet};
use std::ops::{Add, AddAssign};
use std::sync::Arc;

use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use super::response::{EmbeddingResponse, OcrResponse, VlmResponse};
use super::{Chat, Content, Document, Message};

// ============================================================================
// Context Types
// ============================================================================

/// Context information for provider operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    /// Unique identifier for this context session.
    pub context_id: Uuid,
    /// Context creation timestamp.
    pub created_at: Timestamp,
    /// Usage statistics for this context.
    pub usage: UsageStats,
}

impl Context {
    /// Create a new context.
    pub fn new() -> Self {
        Self {
            context_id: Uuid::now_v7(),
            created_at: Timestamp::now(),
            usage: UsageStats::default(),
        }
    }

    /// Record usage statistics by adding the provided stats to the current stats.
    pub fn record(&mut self, stats: UsageStats) {
        self.usage += stats;
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe shared context for provider operations.
///
/// This wrapper provides convenient async access to a shared context
/// that can be passed to providers and used across async boundaries.
#[derive(Clone)]
pub struct SharedContext {
    inner: Arc<RwLock<Context>>,
}

impl SharedContext {
    /// Create a new shared context.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(Context::new())),
        }
    }

    /// Create a shared context from an existing context.
    pub fn from_context(context: Context) -> Self {
        Self {
            inner: Arc::new(RwLock::new(context)),
        }
    }

    /// Get read access to the context.
    pub async fn read(&self) -> tokio::sync::RwLockReadGuard<'_, Context> {
        self.inner.read().await
    }

    /// Get write access to the context.
    pub async fn write(&self) -> tokio::sync::RwLockWriteGuard<'_, Context> {
        self.inner.write().await
    }

    /// Get the context ID.
    pub async fn context_id(&self) -> Uuid {
        self.inner.read().await.context_id
    }

    /// Get the context creation timestamp.
    pub async fn created_at(&self) -> Timestamp {
        self.inner.read().await.created_at
    }

    /// Get a clone of the usage statistics.
    pub async fn usage(&self) -> UsageStats {
        self.inner.read().await.usage.clone()
    }

    /// Record usage statistics by adding the provided stats to the current stats.
    pub async fn record(&self, stats: UsageStats) {
        self.inner.write().await.record(stats);
    }

    /// Replace the inner context.
    pub async fn set_context(&self, context: Context) {
        *self.inner.write().await = context;
    }

    /// Get a clone of the inner context.
    pub async fn get_context(&self) -> Context {
        self.inner.read().await.clone()
    }
}

impl Default for SharedContext {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for SharedContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedContext").finish_non_exhaustive()
    }
}

/// Usage statistics for provider operations.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct UsageStats {
    /// Total tokens processed.
    pub total_tokens: u32,
    /// Total runs (embeddings generated, images processed, pages processed, etc.).
    pub total_runs: u32,
    /// Total processing time.
    pub total_processing_time: SignedDuration,
    /// Number of successful requests.
    pub successful_requests: u32,
    /// Number of failed requests.
    pub failed_requests: u32,
}

impl UsageStats {
    /// Create a new empty usage stats.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create usage stats for a successful request.
    pub fn success(tokens: u32, runs: u32, processing_time: SignedDuration) -> Self {
        Self {
            total_tokens: tokens,
            total_runs: runs,
            total_processing_time: processing_time,
            successful_requests: 1,
            failed_requests: 0,
        }
    }

    /// Create usage stats for a failed request.
    pub fn failure(tokens: u32, processing_time: SignedDuration) -> Self {
        Self {
            total_tokens: tokens,
            total_runs: 0,
            total_processing_time: processing_time,
            successful_requests: 0,
            failed_requests: 1,
        }
    }

    /// Builder method to set total tokens.
    pub fn with_tokens(mut self, tokens: u32) -> Self {
        self.total_tokens = tokens;
        self
    }

    /// Builder method to set total runs.
    pub fn with_runs(mut self, runs: u32) -> Self {
        self.total_runs = runs;
        self
    }

    /// Builder method to set processing time.
    pub fn with_processing_time(mut self, processing_time: SignedDuration) -> Self {
        self.total_processing_time = processing_time;
        self
    }

    /// Builder method to set successful requests count.
    pub fn with_successful_requests(mut self, count: u32) -> Self {
        self.successful_requests = count;
        self
    }

    /// Builder method to set failed requests count.
    pub fn with_failed_requests(mut self, count: u32) -> Self {
        self.failed_requests = count;
        self
    }

    /// Get total number of requests (successful + failed).
    pub fn total_requests(&self) -> u32 {
        self.successful_requests + self.failed_requests
    }

    /// Calculate success rate as a percentage (0.0 to 100.0).
    pub fn success_rate(&self) -> f32 {
        let total = self.total_requests();
        if total == 0 {
            0.0
        } else {
            (self.successful_requests as f32 / total as f32) * 100.0
        }
    }

    /// Calculate failure rate as a percentage (0.0 to 100.0).
    pub fn failure_rate(&self) -> f32 {
        let total = self.total_requests();
        if total == 0 {
            0.0
        } else {
            (self.failed_requests as f32 / total as f32) * 100.0
        }
    }

    /// Calculate average processing time per request.
    pub fn average_processing_time(&self) -> Option<SignedDuration> {
        let total = self.total_requests();
        if total == 0 {
            None
        } else {
            Some(self.total_processing_time / total as i32)
        }
    }

    /// Calculate average tokens per request.
    pub fn average_tokens_per_request(&self) -> Option<f32> {
        let total = self.total_requests();
        if total == 0 {
            None
        } else {
            Some(self.total_tokens as f32 / total as f32)
        }
    }

    /// Calculate average runs per successful request.
    pub fn average_runs_per_request(&self) -> Option<f32> {
        if self.successful_requests == 0 {
            None
        } else {
            Some(self.total_runs as f32 / self.successful_requests as f32)
        }
    }

    /// Check if there's any usage data.
    pub fn has_usage(&self) -> bool {
        self.total_requests() > 0
    }

    /// Check if all requests were successful.
    pub fn all_successful(&self) -> bool {
        self.failed_requests == 0 && self.successful_requests > 0
    }

    /// Check if all requests failed.
    pub fn all_failed(&self) -> bool {
        self.successful_requests == 0 && self.failed_requests > 0
    }

    /// Reset all statistics to zero.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Merge another UsageStats into this one.
    pub fn merge(&mut self, other: &UsageStats) {
        self.total_tokens += other.total_tokens;
        self.total_runs += other.total_runs;
        self.total_processing_time = self
            .total_processing_time
            .checked_add(other.total_processing_time)
            .unwrap_or(self.total_processing_time);
        self.successful_requests += other.successful_requests;
        self.failed_requests += other.failed_requests;
    }
}

impl Add for UsageStats {
    type Output = Self;

    fn add(mut self, rhs: Self) -> Self::Output {
        self.merge(&rhs);
        self
    }
}

impl AddAssign for UsageStats {
    fn add_assign(&mut self, rhs: Self) {
        self.merge(&rhs);
    }
}

// ============================================================================
// Embedding Request Types
// ============================================================================

/// Request for a single embedding operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    /// Unique identifier for this request.
    pub request_id: Uuid,
    /// Account identifier associated with this request.
    pub account_id: Option<Uuid>,
    /// The content to generate an embedding for.
    pub content: Content,
    /// Custom tags for categorization and filtering.
    pub tags: HashSet<String>,
    /// Whether to normalize the resulting embedding to unit length.
    pub normalize: bool,
}

impl EmbeddingRequest {
    /// Create a new embedding request with the given content.
    pub fn new(content: Content) -> Self {
        Self {
            request_id: Uuid::now_v7(),
            account_id: None,
            content,
            tags: HashSet::new(),
            normalize: false,
        }
    }

    /// Create a new embedding request from text.
    pub fn from_text(text: impl Into<String>) -> Self {
        Self::new(Content::text(text))
    }

    /// Create a new embedding request from a document.
    pub fn from_document(document: Document) -> Self {
        Self::new(Content::document(document))
    }

    /// Create a new embedding request from a chat.
    pub fn from_chat(chat: Chat) -> Self {
        Self::new(Content::chat(chat))
    }

    /// Create a new embedding request with a specific request ID.
    pub fn with_request_id(mut self, request_id: Uuid) -> Self {
        self.request_id = request_id;
        self
    }

    /// Set the account ID for this request.
    pub fn with_account_id(mut self, account_id: Uuid) -> Self {
        self.account_id = Some(account_id);
        self
    }

    /// Add a tag to this request.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.insert(tag.into());
        self
    }

    /// Set tags for this request.
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(|t| t.into()).collect();
        self
    }

    /// Enable normalization of the embedding to unit length.
    pub fn with_normalize(mut self, normalize: bool) -> Self {
        self.normalize = normalize;
        self
    }

    /// Check if the request has a specific tag.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(tag)
    }

    /// Get the text content if this is a text request.
    pub fn as_text(&self) -> Option<&str> {
        self.content.as_text()
    }

    /// Create a response for this request with the given embedding.
    pub fn reply(&self, embedding: Vec<f32>) -> EmbeddingResponse {
        EmbeddingResponse::new(self.request_id, embedding)
    }
}

/// Batch request for multiple embedding operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingBatchRequest {
    /// Unique identifier for this batch request.
    pub batch_id: Uuid,
    /// Account identifier associated with this batch.
    pub account_id: Option<Uuid>,
    /// The contents to generate embeddings for.
    pub contents: Vec<Content>,
    /// Custom tags for categorization and filtering.
    pub tags: HashSet<String>,
    /// Whether to normalize the resulting embeddings to unit length.
    pub normalize: bool,
}

impl EmbeddingBatchRequest {
    /// Create a new batch request.
    pub fn new() -> Self {
        Self {
            batch_id: Uuid::now_v7(),
            account_id: None,
            contents: Vec::new(),
            tags: HashSet::new(),
            normalize: false,
        }
    }

    /// Create a new batch request from contents.
    pub fn from_contents(contents: Vec<Content>) -> Self {
        Self {
            batch_id: Uuid::now_v7(),
            account_id: None,
            contents,
            tags: HashSet::new(),
            normalize: false,
        }
    }

    /// Set the account ID for this batch.
    pub fn with_account_id(mut self, account_id: Uuid) -> Self {
        self.account_id = Some(account_id);
        self
    }

    /// Add a content item to the batch.
    pub fn with_content(mut self, content: Content) -> Self {
        self.contents.push(content);
        self
    }

    /// Add a text input to the batch.
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.contents.push(Content::text(text));
        self
    }

    /// Add a document input to the batch.
    pub fn with_document(mut self, document: Document) -> Self {
        self.contents.push(Content::document(document));
        self
    }

    /// Add a chat input to the batch.
    pub fn with_chat(mut self, chat: Chat) -> Self {
        self.contents.push(Content::chat(chat));
        self
    }

    /// Add a tag to this batch request.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.insert(tag.into());
        self
    }

    /// Set tags for this batch request.
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(|t| t.into()).collect();
        self
    }

    /// Enable normalization of embeddings to unit length.
    pub fn with_normalize(mut self, normalize: bool) -> Self {
        self.normalize = normalize;
        self
    }

    /// Check if the batch request has a specific tag.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(tag)
    }

    /// Returns the number of contents in this batch.
    pub fn len(&self) -> usize {
        self.contents.len()
    }

    /// Returns true if this batch has no contents.
    pub fn is_empty(&self) -> bool {
        self.contents.is_empty()
    }

    /// Convert this batch request into individual requests.
    pub fn into_requests(self) -> Vec<EmbeddingRequest> {
        self.contents
            .into_iter()
            .map(|content| EmbeddingRequest {
                request_id: Uuid::now_v7(),
                account_id: self.account_id,
                content,
                tags: self.tags.clone(),
                normalize: self.normalize,
            })
            .collect()
    }

    /// Create individual requests from this batch.
    pub fn iter_requests(&self) -> Vec<EmbeddingRequest> {
        self.contents
            .iter()
            .cloned()
            .map(|content| EmbeddingRequest {
                request_id: Uuid::now_v7(),
                account_id: self.account_id,
                content,
                tags: self.tags.clone(),
                normalize: self.normalize,
            })
            .collect()
    }

    /// Estimates the total size of all contents for rate limiting.
    pub fn estimated_total_size(&self) -> usize {
        self.contents
            .iter()
            .map(|content| content.estimated_size())
            .sum()
    }
}

impl Default for EmbeddingBatchRequest {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// OCR Request Types
// ============================================================================

/// Request for a single OCR operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrRequest {
    /// Unique identifier for this request.
    pub request_id: Uuid,
    /// Account identifier associated with this request.
    pub account_id: Option<Uuid>,
    /// The document to process for text extraction.
    pub document: Document,
    /// Optional custom prompt for OCR processing.
    pub prompt: Option<String>,
    /// Language hint for OCR processing (ISO 639-1 code).
    pub language: Option<String>,
    /// Custom tags for categorization and filtering.
    pub tags: HashSet<String>,
    /// Whether to preserve layout information in the output.
    pub preserve_layout: bool,
    /// Minimum confidence threshold for text extraction.
    pub confidence_threshold: Option<f32>,
}

impl OcrRequest {
    /// Create a new OCR request with the given document.
    pub fn new(document: Document) -> Self {
        Self {
            request_id: Uuid::now_v7(),
            account_id: None,
            document,
            prompt: None,
            language: None,
            tags: HashSet::new(),
            preserve_layout: true,
            confidence_threshold: None,
        }
    }

    /// Create a new OCR request from a document (alias for `new`).
    pub fn from_document(document: Document) -> Self {
        Self::new(document)
    }

    /// Create a new OCR request with a specific request ID.
    pub fn with_request_id(mut self, request_id: Uuid) -> Self {
        self.request_id = request_id;
        self
    }

    /// Set the account ID for this request.
    pub fn with_account_id(mut self, account_id: Uuid) -> Self {
        self.account_id = Some(account_id);
        self
    }

    /// Set a custom prompt for OCR processing.
    pub fn with_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = Some(prompt.into());
        self
    }

    /// Set the language hint for OCR processing.
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// Add a tag to this request.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.insert(tag.into());
        self
    }

    /// Set tags for this request.
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(|t| t.into()).collect();
        self
    }

    /// Set whether to preserve layout information.
    pub fn with_preserve_layout(mut self, preserve: bool) -> Self {
        self.preserve_layout = preserve;
        self
    }

    /// Set the confidence threshold for text extraction.
    pub fn with_confidence_threshold(mut self, threshold: f32) -> Self {
        self.confidence_threshold = Some(threshold);
        self
    }

    /// Check if the request has a specific tag.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(tag)
    }

    /// Get the document's content type.
    pub fn content_type(&self) -> Option<&str> {
        self.document.content_type()
    }

    /// Get the document size in bytes.
    pub fn document_size(&self) -> usize {
        self.document.size()
    }

    /// Check if the document is empty.
    pub fn is_empty(&self) -> bool {
        self.document.is_empty()
    }

    /// Get the document bytes.
    pub fn as_bytes(&self) -> &[u8] {
        self.document.as_bytes()
    }

    /// Create a response for this request with the given text.
    pub fn reply(&self, text: impl Into<String>) -> OcrResponse {
        OcrResponse::new(self.request_id, text)
    }
}

/// Batch request for multiple OCR operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrBatchRequest {
    /// Unique identifier for this batch request.
    pub batch_id: Uuid,
    /// Account identifier associated with this batch.
    pub account_id: Option<Uuid>,
    /// The documents to process.
    pub documents: Vec<Document>,
    /// Optional custom prompt for OCR processing.
    pub prompt: Option<String>,
    /// Language hint for OCR processing (ISO 639-1 code).
    pub language: Option<String>,
    /// Custom tags for categorization and filtering.
    pub tags: HashSet<String>,
    /// Whether to preserve layout information.
    pub preserve_layout: bool,
    /// Minimum confidence threshold for text extraction.
    pub confidence_threshold: Option<f32>,
}

impl OcrBatchRequest {
    /// Create a new batch request.
    pub fn new() -> Self {
        Self {
            batch_id: Uuid::now_v7(),
            account_id: None,
            documents: Vec::new(),
            prompt: None,
            language: None,
            tags: HashSet::new(),
            preserve_layout: true,
            confidence_threshold: None,
        }
    }

    /// Create a new batch request from documents.
    pub fn from_documents(documents: Vec<Document>) -> Self {
        Self {
            batch_id: Uuid::now_v7(),
            account_id: None,
            documents,
            prompt: None,
            language: None,
            tags: HashSet::new(),
            preserve_layout: true,
            confidence_threshold: None,
        }
    }

    /// Set the account ID for this batch.
    pub fn with_account_id(mut self, account_id: Uuid) -> Self {
        self.account_id = Some(account_id);
        self
    }

    /// Add a document to the batch.
    pub fn with_document(mut self, document: Document) -> Self {
        self.documents.push(document);
        self
    }

    /// Set a custom prompt for OCR processing.
    pub fn with_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = Some(prompt.into());
        self
    }

    /// Set the language hint for OCR processing.
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// Add a tag to this batch request.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.insert(tag.into());
        self
    }

    /// Set tags for this batch request.
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(|t| t.into()).collect();
        self
    }

    /// Set whether to preserve layout information.
    pub fn with_preserve_layout(mut self, preserve: bool) -> Self {
        self.preserve_layout = preserve;
        self
    }

    /// Set the confidence threshold for text extraction.
    pub fn with_confidence_threshold(mut self, threshold: f32) -> Self {
        self.confidence_threshold = Some(threshold);
        self
    }

    /// Check if the batch request has a specific tag.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(tag)
    }

    /// Returns the number of documents in this batch.
    pub fn len(&self) -> usize {
        self.documents.len()
    }

    /// Returns true if this batch has no documents.
    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    /// Convert this batch request into individual requests.
    pub fn into_requests(self) -> Vec<OcrRequest> {
        self.documents
            .into_iter()
            .map(|document| OcrRequest {
                request_id: Uuid::now_v7(),
                account_id: self.account_id,
                document,
                prompt: self.prompt.clone(),
                language: self.language.clone(),
                tags: self.tags.clone(),
                preserve_layout: self.preserve_layout,
                confidence_threshold: self.confidence_threshold,
            })
            .collect()
    }

    /// Create individual requests from this batch.
    pub fn iter_requests(&self) -> Vec<OcrRequest> {
        self.documents
            .iter()
            .cloned()
            .map(|document| OcrRequest {
                request_id: Uuid::now_v7(),
                account_id: self.account_id,
                document,
                prompt: self.prompt.clone(),
                language: self.language.clone(),
                tags: self.tags.clone(),
                preserve_layout: self.preserve_layout,
                confidence_threshold: self.confidence_threshold,
            })
            .collect()
    }

    /// Estimates the total size of all documents.
    pub fn estimated_total_size(&self) -> usize {
        self.documents.iter().map(|doc| doc.size()).sum()
    }
}

impl Default for OcrBatchRequest {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// VLM Request Types
// ============================================================================

/// Request for a single VLM operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VlmRequest {
    /// Unique identifier for this request.
    pub request_id: Uuid,
    /// Account identifier associated with this request.
    pub account_id: Option<Uuid>,
    /// Text prompt for the VLM.
    pub prompt: String,
    /// Documents to analyze (images, PDFs, etc.).
    pub documents: Vec<Document>,
    /// Optional conversation history.
    pub messages: Vec<Message>,
    /// Custom tags for categorization and filtering.
    pub tags: HashSet<String>,
    /// Maximum number of tokens to generate.
    pub max_tokens: Option<u32>,
    /// Temperature for response generation (0.0 to 1.0).
    pub temperature: Option<f32>,
    /// Custom parameters for specific VLM engines.
    pub custom_parameters: HashMap<String, serde_json::Value>,
}

impl VlmRequest {
    /// Create a new VLM request with the given prompt.
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            request_id: Uuid::now_v7(),
            account_id: None,
            prompt: prompt.into(),
            documents: Vec::new(),
            messages: Vec::new(),
            tags: HashSet::new(),
            max_tokens: Some(1024),
            temperature: Some(0.7),
            custom_parameters: HashMap::new(),
        }
    }

    /// Create a new VLM request with prompt and document.
    pub fn with_document(prompt: impl Into<String>, document: Document) -> Self {
        Self {
            request_id: Uuid::now_v7(),
            account_id: None,
            prompt: prompt.into(),
            documents: vec![document],
            messages: Vec::new(),
            tags: HashSet::new(),
            max_tokens: Some(1024),
            temperature: Some(0.7),
            custom_parameters: HashMap::new(),
        }
    }

    /// Create a new VLM request with a specific request ID.
    pub fn with_request_id(mut self, request_id: Uuid) -> Self {
        self.request_id = request_id;
        self
    }

    /// Set the account ID for this request.
    pub fn with_account_id(mut self, account_id: Uuid) -> Self {
        self.account_id = Some(account_id);
        self
    }

    /// Add a document to this request.
    pub fn add_document(mut self, document: Document) -> Self {
        self.documents.push(document);
        self
    }

    /// Add multiple documents to this request.
    pub fn with_documents(mut self, documents: Vec<Document>) -> Self {
        self.documents = documents;
        self
    }

    /// Add a message to the conversation history.
    pub fn add_message(mut self, message: Message) -> Self {
        self.messages.push(message);
        self
    }

    /// Set the conversation history.
    pub fn with_messages(mut self, messages: Vec<Message>) -> Self {
        self.messages = messages;
        self
    }

    /// Add a tag to this request.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.insert(tag.into());
        self
    }

    /// Set tags for this request.
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(|t| t.into()).collect();
        self
    }

    /// Set maximum tokens to generate.
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Set temperature for response generation.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature.clamp(0.0, 1.0));
        self
    }

    /// Add a custom parameter.
    pub fn with_custom_parameter(
        mut self,
        key: impl Into<String>,
        value: serde_json::Value,
    ) -> Self {
        self.custom_parameters.insert(key.into(), value);
        self
    }

    /// Check if the request has a specific tag.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(tag)
    }

    /// Check if this request has documents.
    pub fn has_documents(&self) -> bool {
        !self.documents.is_empty()
    }

    /// Get the number of documents.
    pub fn document_count(&self) -> usize {
        self.documents.len()
    }

    /// Check if this request has messages.
    pub fn has_messages(&self) -> bool {
        !self.messages.is_empty()
    }

    /// Get the number of messages.
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Check if this is a text-only request.
    pub fn is_text_only(&self) -> bool {
        self.documents.is_empty()
    }

    /// Get the total size of all documents.
    pub fn total_document_size(&self) -> usize {
        self.documents.iter().map(|doc| doc.size()).sum()
    }

    /// Get the prompt length in characters.
    pub fn prompt_length(&self) -> usize {
        self.prompt.chars().count()
    }

    /// Create a response for this request with the given content.
    pub fn reply(&self, content: impl Into<String>) -> VlmResponse {
        VlmResponse::new(self.request_id, content)
    }
}

/// Batch request for multiple VLM operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VlmBatchRequest {
    /// Unique identifier for this batch request.
    pub batch_id: Uuid,
    /// Account identifier associated with this batch.
    pub account_id: Option<Uuid>,
    /// Individual requests in the batch.
    requests: Vec<VlmRequest>,
    /// Custom tags for the entire batch.
    pub tags: HashSet<String>,
}

impl VlmBatchRequest {
    /// Create a new batch request.
    pub fn new() -> Self {
        Self {
            batch_id: Uuid::now_v7(),
            account_id: None,
            requests: Vec::new(),
            tags: HashSet::new(),
        }
    }

    /// Create a new batch request from requests.
    pub fn from_requests(requests: Vec<VlmRequest>) -> Self {
        Self {
            batch_id: Uuid::now_v7(),
            account_id: None,
            requests,
            tags: HashSet::new(),
        }
    }

    /// Set the account ID for this batch.
    pub fn with_account_id(mut self, account_id: Uuid) -> Self {
        self.account_id = Some(account_id);
        self
    }

    /// Add a request to the batch.
    pub fn with_request(mut self, request: VlmRequest) -> Self {
        self.requests.push(request);
        self
    }

    /// Add a simple prompt request to the batch.
    pub fn with_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.requests.push(VlmRequest::new(prompt));
        self
    }

    /// Add a request with prompt and document to the batch.
    pub fn with_prompt_and_document(
        mut self,
        prompt: impl Into<String>,
        document: Document,
    ) -> Self {
        self.requests
            .push(VlmRequest::with_document(prompt, document));
        self
    }

    /// Add a tag to this batch request.
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.insert(tag.into());
        self
    }

    /// Set tags for this batch request.
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags = tags.into_iter().map(|t| t.into()).collect();
        self
    }

    /// Check if the batch request has a specific tag.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.contains(tag)
    }

    /// Returns the number of requests in this batch.
    pub fn len(&self) -> usize {
        self.requests.len()
    }

    /// Returns true if this batch has no requests.
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    /// Convert this batch request into individual requests.
    pub fn into_requests(self) -> Vec<VlmRequest> {
        self.requests
    }

    /// Get a reference to the requests.
    pub fn iter_requests(&self) -> &[VlmRequest] {
        &self.requests
    }

    /// Get the total number of documents across all requests.
    pub fn total_documents(&self) -> usize {
        self.requests.iter().map(|r| r.document_count()).sum()
    }

    /// Get the total size of all documents across all requests.
    pub fn total_document_size(&self) -> usize {
        self.requests.iter().map(|r| r.total_document_size()).sum()
    }
}

impl Default for VlmBatchRequest {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;

    use super::*;
    use crate::inference::MessageRole;

    // Embedding tests
    #[test]
    fn test_embedding_request_creation() {
        let request = EmbeddingRequest::from_text("Hello, world!");
        assert!(!request.request_id.is_nil());
        assert!(request.account_id.is_none());
        assert!(request.tags.is_empty());
        assert!(!request.normalize);
        assert_eq!(request.as_text(), Some("Hello, world!"));
    }

    #[test]
    fn test_embedding_batch_request() {
        let document = Document::new(Bytes::from("Hello, world!")).with_content_type("text/plain");
        let batch = EmbeddingBatchRequest::new()
            .with_text("Test text")
            .with_document(document);
        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
    }

    // OCR tests
    #[test]
    fn test_ocr_request_creation() {
        let document = Document::new(Bytes::from("test image data")).with_content_type("image/png");
        let request = OcrRequest::from_document(document);
        assert!(!request.request_id.is_nil());
        assert!(request.account_id.is_none());
        assert!(request.tags.is_empty());
        assert!(request.preserve_layout);
        assert_eq!(request.content_type(), Some("image/png"));
    }

    #[test]
    fn test_ocr_batch_request() {
        let doc1 = Document::new(Bytes::from("doc1")).with_content_type("image/png");
        let doc2 = Document::new(Bytes::from("doc2")).with_content_type("image/jpeg");
        let batch = OcrBatchRequest::new()
            .with_document(doc1)
            .with_document(doc2);
        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
    }

    // VLM tests
    #[test]
    fn test_vlm_request_creation() {
        let request = VlmRequest::new("Describe this image");
        assert!(!request.request_id.is_nil());
        assert!(request.account_id.is_none());
        assert!(request.tags.is_empty());
        assert_eq!(request.prompt, "Describe this image");
        assert!(request.is_text_only());
    }

    #[test]
    fn test_vlm_request_with_document() {
        let document = Document::new(Bytes::from("image data")).with_content_type("image/png");
        let request = VlmRequest::with_document("Describe this", document);
        assert!(request.has_documents());
        assert_eq!(request.document_count(), 1);
        assert!(!request.is_text_only());
    }

    #[test]
    fn test_vlm_request_with_messages() {
        let message1 = Message::new(MessageRole::User, "Previous question");
        let message2 = Message::new(MessageRole::Assistant, "Previous response");
        let request = VlmRequest::new("Continue")
            .add_message(message1)
            .add_message(message2);
        assert!(request.has_messages());
        assert_eq!(request.message_count(), 2);
    }

    #[test]
    fn test_vlm_batch_request() {
        let batch = VlmBatchRequest::new()
            .with_prompt("First prompt")
            .with_prompt("Second prompt");
        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
    }
}
