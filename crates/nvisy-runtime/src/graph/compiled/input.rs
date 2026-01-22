//! Compiled input node types.

use super::stream::InputStream;

/// Compiled input node - ready to stream data.
///
/// This is the runtime representation of an input node after compilation.
/// Cache slots are resolved during compilation, so compiled inputs always
/// wrap concrete input streams.
#[derive(Debug)]
pub struct CompiledInput {
    /// The input stream for reading data.
    stream: InputStream,
}

impl CompiledInput {
    /// Creates a new compiled input from an input stream.
    pub fn new(stream: InputStream) -> Self {
        Self { stream }
    }

    /// Returns a reference to the input stream.
    pub fn stream(&self) -> &InputStream {
        &self.stream
    }

    /// Returns a mutable reference to the input stream.
    pub fn stream_mut(&mut self) -> &mut InputStream {
        &mut self.stream
    }

    /// Consumes this compiled input and returns the underlying stream.
    pub fn into_stream(self) -> InputStream {
        self.stream
    }
}
