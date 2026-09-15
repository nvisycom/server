//! Thread timeline event-kind enumeration.

use super::db_enum;

db_enum! {
    /// The kind of a non-message entry in a discussion thread's timeline.
    ///
    /// Corresponds to the `THREAD_EVENT_KIND` `PostgreSQL` enum. A thread's stream
    /// interleaves comments (messages) with these events. A thread is a pure
    /// discussion primitive, so this is only the discussion lifecycle; review
    /// activity lives in [`ReviewEventKind`](super::ReviewEventKind).
    pub enum ThreadEventKind = "crate::schema::sql_types::ThreadEventKind" {
        /// The thread was opened.
        Opened = "thread.opened",
        /// The thread was closed.
        Closed = "thread.closed",
        /// The thread was reopened.
        Reopened = "thread.reopened",
        /// The thread's display name was changed.
        Renamed = "thread.renamed",
    }
}
