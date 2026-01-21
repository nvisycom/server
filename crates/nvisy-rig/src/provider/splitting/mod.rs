//! Text splitting for chunk creation.

mod chunk;
mod metadata;
mod splitter;

pub use chunk::{Chunk, OwnedChunk};
pub use metadata::ChunkMetadata;
pub use splitter::TextSplitter;
