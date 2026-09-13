//! Thread service inputs.

/// Input for opening a workspace discussion thread with its first message.
pub struct OpenThreadInput {
    /// Optional title for the thread.
    pub display_name: Option<String>,
    /// The opening message text.
    pub body: String,
}
