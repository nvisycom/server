//! Text splitting service for document chunking.
//!
//! Provides semantic text splitting for creating document chunks suitable
//! for embedding and retrieval operations.

use std::ops::Range;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::Stream;
use text_splitter::{ChunkConfig, TextSplitter};

/// Default maximum chunk size in characters.
pub const DEFAULT_MAX_CHUNK_SIZE: usize = 1000;

/// Default minimum chunk size in characters.
pub const DEFAULT_MIN_CHUNK_SIZE: usize = 100;

/// Default overlap between chunks in characters.
pub const DEFAULT_CHUNK_OVERLAP: usize = 200;

/// Configuration for the text splitter service.
#[derive(Debug, Clone)]
pub struct TextSplitterConfig {
    /// Maximum chunk size in characters.
    pub max_chunk_size: usize,
    /// Minimum chunk size in characters.
    pub min_chunk_size: usize,
    /// Overlap between consecutive chunks.
    pub overlap: usize,
}

impl Default for TextSplitterConfig {
    fn default() -> Self {
        Self {
            max_chunk_size: DEFAULT_MAX_CHUNK_SIZE,
            min_chunk_size: DEFAULT_MIN_CHUNK_SIZE,
            overlap: DEFAULT_CHUNK_OVERLAP,
        }
    }
}

impl TextSplitterConfig {
    /// Creates a new text splitter configuration.
    pub fn new(max_chunk_size: usize) -> Self {
        Self {
            max_chunk_size,
            ..Default::default()
        }
    }

    /// Sets the minimum chunk size.
    pub fn with_min_chunk_size(mut self, min_chunk_size: usize) -> Self {
        self.min_chunk_size = min_chunk_size;
        self
    }

    /// Sets the overlap between chunks.
    pub fn with_overlap(mut self, overlap: usize) -> Self {
        self.overlap = overlap;
        self
    }

    /// Returns the chunk size range for text-splitter.
    fn chunk_range(&self) -> Range<usize> {
        self.min_chunk_size..self.max_chunk_size
    }
}

/// A text chunk with its byte offset in the original text.
#[derive(Debug, Clone)]
pub struct TextChunk {
    /// Byte offset in the original text.
    pub offset: usize,
    /// The chunk content.
    pub content: String,
}

/// A stream of text chunks.
pub struct ChunkStream {
    chunks: std::vec::IntoIter<TextChunk>,
}

impl Stream for ChunkStream {
    type Item = TextChunk;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(self.chunks.next())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.chunks.size_hint()
    }
}

/// Service for splitting text into semantic chunks.
///
/// Uses the `text-splitter` crate to split text at semantic boundaries
/// (sentences, paragraphs) while respecting size constraints.
#[derive(Clone)]
pub struct TextSplitterService {
    config: TextSplitterConfig,
}

impl Default for TextSplitterService {
    fn default() -> Self {
        Self::new(TextSplitterConfig::default())
    }
}

impl TextSplitterService {
    /// Creates a new text splitter service with the given configuration.
    pub fn new(config: TextSplitterConfig) -> Self {
        Self { config }
    }

    /// Creates a text splitter service with default settings.
    pub fn with_defaults() -> Self {
        Self::default()
    }

    /// Splits text into chunks and returns them as a stream.
    pub fn split_stream(&self, text: &str) -> ChunkStream {
        let chunks = self.split_with_offsets(text);
        ChunkStream {
            chunks: chunks.into_iter(),
        }
    }

    /// Splits text into chunks and returns them as owned strings.
    pub fn split(&self, text: &str) -> Vec<String> {
        let chunk_config = ChunkConfig::new(self.config.chunk_range())
            .with_overlap(self.config.overlap)
            .expect("valid overlap configuration");

        TextSplitter::new(chunk_config)
            .chunks(text)
            .map(String::from)
            .collect()
    }

    /// Splits text and returns chunks with their byte offsets.
    pub fn split_with_offsets(&self, text: &str) -> Vec<TextChunk> {
        let chunk_config = ChunkConfig::new(self.config.chunk_range())
            .with_overlap(self.config.overlap)
            .expect("valid overlap configuration");

        TextSplitter::new(chunk_config)
            .chunk_indices(text)
            .map(|(offset, chunk)| TextChunk {
                offset,
                content: chunk.to_string(),
            })
            .collect()
    }

    /// Returns the current configuration.
    pub fn config(&self) -> &TextSplitterConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use futures::StreamExt;

    use super::*;

    #[test]
    fn test_default_config() {
        let config = TextSplitterConfig::default();
        assert_eq!(config.max_chunk_size, DEFAULT_MAX_CHUNK_SIZE);
        assert_eq!(config.min_chunk_size, DEFAULT_MIN_CHUNK_SIZE);
        assert_eq!(config.overlap, DEFAULT_CHUNK_OVERLAP);
    }

    #[test]
    fn test_split_text() {
        let service = TextSplitterService::with_defaults();
        let text = "Hello world. This is a test. Another sentence here.";
        let chunks = service.split(text);
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_split_with_offsets() {
        let service = TextSplitterService::with_defaults();
        let text = "Hello world. This is a test.";
        let chunks = service.split_with_offsets(text);
        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].offset, 0);
    }

    #[tokio::test]
    async fn test_split_stream() {
        let service = TextSplitterService::with_defaults();
        let text = "Hello world. This is a test. Another sentence here.";
        let chunks: Vec<_> = service.split_stream(text).collect().await;
        assert!(!chunks.is_empty());
    }
}
