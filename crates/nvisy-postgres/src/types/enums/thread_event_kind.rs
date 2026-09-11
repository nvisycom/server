//! Thread timeline event-kind enumeration.

use super::db_enum;

db_enum! {
    /// The kind of a non-message entry in a comment thread's timeline.
    ///
    /// Corresponds to the `THREAD_EVENT_KIND` PostgreSQL enum. A thread's stream
    /// interleaves comments (messages) with these events, so a reader sees who
    /// opened, closed, reopened, or renamed the thread, and when anchors were
    /// added or removed, between the messages.
    pub enum ThreadEventKind = "crate::schema::sql_types::ThreadEventKind" {
        /// The thread was opened.
        Opened = "thread.opened",
        /// The thread was closed.
        Closed = "thread.closed",
        /// The thread was reopened.
        Reopened = "thread.reopened",
        /// The thread's display name was changed.
        Renamed = "thread.renamed",
        /// An anchor (location pin) was added to the thread.
        AnchorAdded = "thread.anchor.added",
        /// An anchor was removed from the thread.
        AnchorRemoved = "thread.anchor.removed",
    }
}
