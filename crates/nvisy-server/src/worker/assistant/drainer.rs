//! Assistant-job outbox handler.
//!
//! Relays each pending assistant-reply outbox row onto the assistant work-queue,
//! so a reply queued transactionally with the triggering comment reaches the
//! worker even if the process crashes between the commit and the publish. A
//! dead-lettered reply job just means the assistant never answers that message.

use nvisy_nats::stream::EventPublisher;
use nvisy_postgres::PgConn;
use nvisy_postgres::model::WorkspaceAssistantJob;

use super::job::{AssistantJob, AssistantStream};
use crate::service::Infra;
use crate::worker::outbox::{Handled, OutboxDrainer, OutboxHandler, relay};

/// Drains the assistant-job outbox onto the assistant work-queue.
pub type AssistantOutboxDrainer = OutboxDrainer<AssistantJobRelay>;

/// Relays assistant-reply jobs.
pub struct AssistantJobRelay {
    publisher: EventPublisher<AssistantStream>,
}

impl AssistantJobRelay {
    /// Creates a new [`AssistantJobRelay`].
    #[must_use]
    pub fn new(infra: &Infra) -> Self {
        Self {
            publisher: infra.nats.event_publisher::<AssistantStream>(),
        }
    }
}

impl OutboxHandler for AssistantJobRelay {
    type Done = ();
    type Row = WorkspaceAssistantJob;

    const NAME: &'static str = "assistant_outbox_drainer";

    async fn handle(&self, _conn: &mut PgConn, row: &WorkspaceAssistantJob) -> Handled<()> {
        relay(row.id, &row.job, async |job: AssistantJob| {
            self.publisher.publish(&job).await
        })
        .await
    }
}
