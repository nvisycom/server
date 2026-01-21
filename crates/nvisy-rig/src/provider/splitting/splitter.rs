//! Text splitting implementation.

use std::num::NonZeroU32;

use text_splitter::{ChunkConfig, TextSplitter as TextSplitterImpl};

use super::{Chunk, ChunkMetadata, OwnedChunk};

/// Text splitter for creating document chunks.
#[derive(Debug, Clone)]
pub struct TextSplitter {
    max_characters: u32,
    overlap_characters: Option<NonZeroU32>,
    trim_whitespace: bool,
}

impl TextSplitter {
    /// Creates a new text splitter.
    pub fn new(
        max_characters: u32,
        overlap_characters: Option<NonZeroU32>,
        trim_whitespace: bool,
    ) -> Self {
        tracing::debug!(
            max_characters,
            ?overlap_characters,
            trim_whitespace,
            "created text splitter"
        );
        Self {
            max_characters,
            overlap_characters,
            trim_whitespace,
        }
    }

    /// Creates a splitter with default settings (512 chars, no overlap, trimmed).
    pub fn with_defaults() -> Self {
        Self::new(512, None, true)
    }

    /// Returns the maximum characters per chunk.
    pub fn max_characters(&self) -> u32 {
        self.max_characters
    }

    /// Returns the overlap between chunks.
    pub fn overlap_characters(&self) -> Option<NonZeroU32> {
        self.overlap_characters
    }

    /// Splits text into chunks with byte offset tracking.
    #[tracing::instrument(skip(self, text), fields(text_len = text.len()))]
    pub fn split<'a>(&self, text: &'a str) -> Vec<Chunk<'a>> {
        let overlap = self.overlap_characters.map_or(0, |v| v.get() as usize);
        let chunk_config = ChunkConfig::new(self.max_characters as usize)
            .with_overlap(overlap)
            .expect("overlap must be less than max_characters")
            .with_trim(self.trim_whitespace);

        let splitter = TextSplitterImpl::new(chunk_config);

        let chunks: Vec<_> = splitter
            .chunk_indices(text)
            .enumerate()
            .map(|(index, (byte_offset, chunk_text))| {
                let end_offset = byte_offset + chunk_text.len();
                Chunk::new(
                    chunk_text,
                    ChunkMetadata::new(index as u32, byte_offset as u32, end_offset as u32),
                )
            })
            .collect();

        tracing::debug!(chunk_count = chunks.len(), "split text into chunks");
        chunks
    }

    /// Splits text and returns owned chunks.
    #[tracing::instrument(skip(self, text), fields(text_len = text.len()))]
    pub fn split_owned(&self, text: &str) -> Vec<OwnedChunk> {
        self.split(text)
            .into_iter()
            .map(|c| c.into_owned())
            .collect()
    }

    /// Splits text with page awareness.
    ///
    /// Page breaks are indicated by form feed characters (`\x0c`).
    #[tracing::instrument(skip(self, text), fields(text_len = text.len()))]
    pub fn split_with_pages<'a>(&self, text: &'a str) -> Vec<Chunk<'a>> {
        let page_breaks: Vec<u32> = text
            .char_indices()
            .filter(|(_, c)| *c == '\x0c')
            .map(|(i, _)| i as u32)
            .collect();

        tracing::debug!(page_count = page_breaks.len() + 1, "detected pages");

        self.split(text)
            .into_iter()
            .map(|chunk| {
                let page_num = page_breaks
                    .iter()
                    .take_while(|&&pos| pos < chunk.metadata.start_offset)
                    .count() as u32
                    + 1;

                // SAFETY: page_num is always >= 1
                let page = NonZeroU32::new(page_num).expect("page number is always >= 1");

                Chunk {
                    text: chunk.text,
                    metadata: chunk.metadata.with_page(page),
                }
            })
            .collect()
    }

    /// Splits text with page awareness and returns owned chunks.
    #[tracing::instrument(skip(self, text), fields(text_len = text.len()))]
    pub fn split_with_pages_owned(&self, text: &str) -> Vec<OwnedChunk> {
        self.split_with_pages(text)
            .into_iter()
            .map(|c| c.into_owned())
            .collect()
    }
}

impl Default for TextSplitter {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_basic() {
        let splitter = TextSplitter::new(50, None, true);
        let text = "Hello world. This is a test. Another sentence here.";
        let chunks = splitter.split(text);

        assert!(!chunks.is_empty());
        for chunk in &chunks {
            assert!(chunk.text.len() <= 50);
        }
    }

    #[test]
    fn test_split_with_overlap() {
        let splitter = TextSplitter::new(20, NonZeroU32::new(5), true);
        let text = "The quick brown fox jumps over the lazy dog.";
        let chunks = splitter.split(text);

        assert!(chunks.len() > 1);
    }

    #[test]
    fn test_split_with_pages() {
        let splitter = TextSplitter::new(100, None, true);
        let text = "Page one content.\x0cPage two content.\x0cPage three.";
        let chunks = splitter.split_with_pages(text);

        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].metadata.page, NonZeroU32::new(1));
    }

    #[test]
    fn test_metadata_offsets() {
        let splitter = TextSplitter::new(500, None, false);
        let text = "Hello world";
        let chunks = splitter.split(text);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].metadata.start_offset, 0);
        assert_eq!(chunks[0].metadata.end_offset, text.len() as u32);
    }
}
