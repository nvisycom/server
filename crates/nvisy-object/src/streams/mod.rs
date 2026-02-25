//! Streaming traits and object store adapters.

mod read_object;
mod source_stream;
mod target_stream;
mod write_object;

pub use read_object::{ObjectReadParams, ObjectReadStream};
pub use source_stream::StreamSource;
pub use target_stream::StreamTarget;
pub use write_object::{ObjectWriteParams, ObjectWriteStream};
