//! The assistant pipeline: answering comment-thread mentions of the AI assistant
//! with a background, at-least-once job queue.
//!
//! When a user addresses the assistant in a comment thread, the comment handler
//! enqueues a reply job (transactionally with the comment) via [`AssistantQueue`].
//! The [`AssistantOutboxDrainer`] relays pending jobs to the assistant NATS
//! work-queue, and the [`AssistantWorker`] consumes them: it reads the thread's
//! conversation, runs the workspace's language model, and posts the reply as a
//! comment authored by the reserved assistant account. The [`AssistantCoordinator`]
//! wakes the drainer the moment a job commits.

mod coordinator;
mod drainer;
mod job;
mod service;
mod worker;

pub use coordinator::AssistantCoordinator;
pub use drainer::AssistantOutboxDrainer;
pub use job::AssistantJob;
pub use service::AssistantQueue;
pub use worker::AssistantWorker;
