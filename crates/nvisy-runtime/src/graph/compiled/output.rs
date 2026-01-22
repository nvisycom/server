//! Compiled output node types.

use super::stream::OutputStream;

/// Compiled output node - ready to receive data.
///
/// This is the runtime representation of an output node after compilation.
/// Cache slots are resolved during compilation, so compiled outputs always
/// wrap concrete output streams.
#[derive(Debug)]
pub struct CompiledOutput {
    /// The output stream for writing data.
    stream: OutputStream,
}

impl CompiledOutput {
    /// Creates a new compiled output from an output stream.
    pub fn new(stream: OutputStream) -> Self {
        Self { stream }
    }

    /// Returns a reference to the output stream.
    pub fn stream(&self) -> &OutputStream {
        &self.stream
    }

    /// Returns a mutable reference to the output stream.
    pub fn stream_mut(&mut self) -> &mut OutputStream {
        &mut self.stream
    }

    /// Consumes this compiled output and returns the underlying stream.
    pub fn into_stream(self) -> OutputStream {
        self.stream
    }
}
