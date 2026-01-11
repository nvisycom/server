//! Text splitting for chunk creation.

mod chunk;
mod metadata;

use text_splitter::{ChunkConfig, TextSplitter};

pub(crate) use self::chunk::{OwnedSplitChunk, SplitChunk};
pub(crate) use self::metadata::SplitMetadata;

/// Estimates the token count (~4 chars per token).
pub fn estimate_tokens(text: &str) -> u32 {
    (text.len() / 4) as u32
}

/// Text splitter service for creating document chunks.
#[derive(Clone)]
pub struct Splitter {
    max_characters: u32,
    overlap: u32,
    trim: bool,
}

impl Splitter {
    /// Creates a new text splitter.
    pub fn new(max_characters: u32, overlap: u32, trim: bool) -> Self {
        Self {
            max_characters,
            overlap,
            trim,
        }
    }

    /// Splits text into chunks with byte offset tracking.
    pub fn split<'a>(&self, text: &'a str) -> Vec<SplitChunk<'a>> {
        let chunk_config = ChunkConfig::new(self.max_characters as usize)
            .with_overlap(self.overlap as usize)
            .expect("overlap must be less than max_characters")
            .with_trim(self.trim);
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
        let page_breaks: Vec<u32> = text
            .char_indices()
            .filter(|(_, c)| *c == '\x0c')
            .map(|(i, _)| i as u32)
            .collect();

        self.split(text)
            .into_iter()
            .map(|mut chunk| {
                let page = page_breaks
                    .iter()
                    .take_while(|&&pos| pos < chunk.metadata.start_offset)
                    .count() as u32
                    + 1;
                chunk.metadata.page = Some(page);
                chunk
            })
            .collect()
    }

    /// Splits text with page awareness and returns owned chunks.
    pub fn split_with_pages_owned(&self, text: &str) -> Vec<OwnedSplitChunk> {
        self.split_with_pages(text)
            .into_iter()
            .map(|c| c.into_owned())
            .collect()
    }
}

impl Default for Splitter {
    fn default() -> Self {
        Self::new(512, 0, true)
    }
}
