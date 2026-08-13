//! OpenAPI-documented wrapper around an [`Sse`] response.
//!
//! Axum's [`Sse`] does not implement aide's [`OperationOutput`], so a handler
//! returning it cannot be registered via `api_route`/`get_with` (no OpenAPI is
//! produced). [`SseResponse`] wraps it, delegating [`IntoResponse`] to the inner
//! stream while advertising a `200 text/event-stream` response whose media type
//! carries the JSON Schema of one event's `data` payload.

use std::convert::Infallible;
use std::marker::PhantomData;
use std::pin::Pin;

use aide::OperationOutput;
use aide::generate::GenContext;
use aide::openapi::{MediaType, Operation, Response, SchemaObject, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response as AxumResponse};
use futures::{Stream, StreamExt};
use schemars::JsonSchema;

/// The erased event stream an [`SseResponse`] serves.
///
/// The stream is boxed so a handler's return type is a plain `SseResponse<E>`
/// rather than leaking the anonymous `Stream` type of a `stream!` block. Its
/// item is `Result<Event, Infallible>` because that is what [`Sse`] consumes;
/// the streams we build never error, so the error half is uninhabited.
type EventStream = Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>;

/// An [`Sse`] response that also produces OpenAPI documentation.
///
/// Wrap a handler's event stream in this so the route can be registered with
/// `api_route`/`get_with` and appear in the generated schema as a
/// `text/event-stream` endpoint. The `E` type parameter is the payload carried
/// in each event's `data` field; its schema is attached to the media type so
/// consumers can see the shape of the events they will receive.
///
/// OpenAPI has no first-class model for the SSE wire framing (`event:`/`data:`
/// lines), so this documents the per-event `data` payload schema — the standard,
/// meaningful thing to expose for an event stream.
#[must_use = "responses do nothing unless returned from a handler"]
pub struct SseResponse<E> {
    inner: Sse<EventStream>,
    _event: PhantomData<fn() -> E>,
}

impl<E> SseResponse<E> {
    /// Builds an SSE response from a stream of [`Event`]s carrying an `E` payload
    /// in their `data` field.
    ///
    /// The stream is boxed and a default keep-alive is applied. The caller yields
    /// bare `Event`s — the `Result`/keep-alive framing [`Sse`] requires is added
    /// here so it stays out of every handler signature.
    pub fn new(stream: impl Stream<Item = Event> + Send + 'static) -> Self {
        let inner: EventStream = Box::pin(stream.map(Ok));
        Self {
            inner: Sse::new(inner),
            _event: PhantomData,
        }
    }
}

impl<E> IntoResponse for SseResponse<E> {
    fn into_response(self) -> AxumResponse {
        // Keep-alive is applied at response time: `.keep_alive()` changes the
        // `Sse` stream type, so it cannot be baked into the stored field.
        self.inner.keep_alive(KeepAlive::default()).into_response()
    }
}

impl<E> OperationOutput for SseResponse<E>
where
    E: JsonSchema,
{
    type Inner = E;

    fn operation_response(ctx: &mut GenContext, _operation: &mut Operation) -> Option<Response> {
        let json_schema = ctx.schema.subschema_for::<E>();

        let mut response = Response {
            description: "Server-sent event stream; each event's `data` is the payload below."
                .to_owned(),
            ..Default::default()
        };
        response.content.insert(
            "text/event-stream".to_owned(),
            MediaType {
                schema: Some(SchemaObject {
                    json_schema,
                    example: None,
                    external_docs: None,
                }),
                ..Default::default()
            },
        );
        Some(response)
    }

    fn inferred_responses(
        ctx: &mut GenContext,
        operation: &mut Operation,
    ) -> Vec<(Option<StatusCode>, Response)> {
        match Self::operation_response(ctx, operation) {
            Some(response) => Vec::from([(Some(StatusCode::Code(200)), response)]),
            None => Vec::new(),
        }
    }
}
