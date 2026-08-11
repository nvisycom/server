//! OpenAPI-documented wrapper around an [`Sse`] response.
//!
//! Axum's [`Sse`] does not implement aide's [`OperationOutput`], so a handler
//! returning it cannot be registered via `api_route`/`get_with` (no OpenAPI is
//! produced). [`SseResponse`] wraps it, delegating [`IntoResponse`] to the inner
//! stream while advertising a `200 text/event-stream` response whose media type
//! carries the JSON Schema of one event's `data` payload.

use std::marker::PhantomData;

use aide::OperationOutput;
use aide::generate::GenContext;
use aide::openapi::{MediaType, Operation, Response, SchemaObject, StatusCode};
use axum::response::sse::Sse;
use axum::response::{IntoResponse, Response as AxumResponse};
use schemars::JsonSchema;

/// An [`Sse`] response that also produces OpenAPI documentation.
///
/// Wrap a handler's [`Sse`] in this so the route can be registered with
/// `api_route`/`get_with` and appear in the generated schema as a
/// `text/event-stream` endpoint. The `E` type parameter is the payload carried
/// in each event's `data` field; its schema is attached to the media type so
/// consumers can see the shape of the events they will receive.
///
/// OpenAPI has no first-class model for the SSE wire framing (`event:`/`data:`
/// lines), so this documents the per-event `data` payload schema — the standard,
/// meaningful thing to expose for an event stream.
#[must_use = "responses do nothing unless returned from a handler"]
pub struct SseResponse<S, E> {
    inner: Sse<S>,
    _event: PhantomData<fn() -> E>,
}

impl<S, E> SseResponse<S, E> {
    /// Wraps an [`Sse`] whose events carry an `E` payload in their `data` field.
    pub fn new(inner: Sse<S>) -> Self {
        Self {
            inner,
            _event: PhantomData,
        }
    }
}

impl<S, E> IntoResponse for SseResponse<S, E>
where
    Sse<S>: IntoResponse,
{
    fn into_response(self) -> AxumResponse {
        self.inner.into_response()
    }
}

impl<S, E> OperationOutput for SseResponse<S, E>
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
