//! Text splitting for chunk creation.

mod chunk;
mod metadata;

use text_splitter::{ChunkConfig, TextSplitter};

pub use self::chunk::{OwnedSplitChunk, SplitChunk};
pub use self::metadata::SplitMetadata;

/// Text splitter service for creating document chunks.
pub struct TextSplitterService {
    max_characters: u32,
    trim: bool,
}

impl TextSplitterService {
    /// Creates a new text splitter.
    pub fn new(max_characters: u32, trim: bool) -> Self {
        Self {
            max_characters,
            trim,
        }
    }

    /// Returns the maximum characters per chunk.
    pub fn max_characters(&self) -> u32 {
        self.max_characters
    }

    /// Returns whether trimming is enabled.
    pub fn trim(&self) -> bool {
        self.trim
    }

    /// Splits text into chunks with byte offset tracking.
    pub fn split<'a>(&self, text: &'a str) -> Vec<SplitChunk<'a>> {
        let chunk_config = ChunkConfig::new(self.max_characters as usize).with_trim(self.trim);
        let splitter = TextSplitter::new(chunk_config);

        splitter
            .chunk_indices(text)
            .enumerate()
            .map(|(chunk_index, (byte_offset, chunk_text))| {
                let end_offset = byte_offset + chunk_text.len();

                SplitChunk {
                    text: chunk_text,
                    metadata: SplitMetadata::new(
                        chunk_index as u32,
                        byte_offset as u32,
                        end_offset as u32,
                    ),
                }
            })
            .collect()
    }

    /// Splits text and returns owned chunks.
    pub fn split_owned(&self, text: &str) -> Vec<OwnedSplitChunk> {
        self.split(text)
            .into_iter()
            .map(|c| c.into_owned())
            .collect()
    }

    /// Splits text with page awareness.
    ///
    /// Page breaks should be indicated by form feed characters (`\x0c`).
    pub fn split_with_pages<'a>(&self, text: &'a str) -> Vec<SplitChunk<'a>> {
        let mut chunks = self.split(text);

        let page_breaks: Vec<u32> = text
            .char_indices()
            .filter(|(_, c)| *c == '\x0c')
            .map(|(i, _)| i as u32)
            .collect();

        for chunk in &mut chunks {
            let page = page_breaks
                .iter()
                .filter(|&&pos| pos < chunk.metadata.start_offset)
                .count() as u32
                + 1;

            chunk.metadata.page = Some(page);
        }

        chunks
    }

    /// Estimates the token count for text (~4 chars per token).
    pub fn estimate_tokens(text: &str) -> u32 {
        (text.len() / 4) as u32
    }
}

impl Default for TextSplitterService {
    fn default() -> Self {
        Self::new(1000, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_preserves_offsets() {
        let splitter = TextSplitterService::new(20, true);
        let text = "Hello world. This is a test.";

        let chunks = splitter.split(text);

        assert_eq!(chunks[0].metadata.start_offset, 0);

        for chunk in &chunks {
            let range = chunk.metadata.byte_range();
            let extracted = &text[range];
            assert_eq!(extracted, chunk.text);
        }
    }

    #[test]
    fn split_with_page_breaks() {
        let splitter = TextSplitterService::new(100, true);
        let text = "Page one content.\x0cPage two content.\x0cPage three content.";

        let chunks = splitter.split_with_pages(text);

        assert!(!chunks.is_empty());
        assert!(chunks.iter().all(|c| c.metadata.page.is_some()));
    }
}
