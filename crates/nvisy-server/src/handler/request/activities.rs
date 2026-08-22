//! Workspace activity request types.

use nvisy_postgres::query::ActivityFilter;
use nvisy_postgres::types::{ActivityType, Handle};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::handler::Result;
use crate::handler::request::{DateWindow, ExportFormat, ResolvedWindow};

/// Most rows a single export returns. A hard ceiling so one request cannot
/// materialize an unbounded result; a truncated export says so in its response.
pub const MAX_EXPORT_ROWS: usize = 100_000;

/// The activity-specific filter parameters: which activity types to keep and
/// whose activities to keep.
///
/// This is its own query struct so an endpoint composes it alongside the shared
/// [`CursorPagination`](crate::handler::request::CursorPagination) and
/// [`DateWindow`] as separate query extractors, rather than `#[serde(flatten)]`ing
/// them into one struct: the query extractor (`serde_html_form`) mis-handles
/// flattened sub-structs — a flattened pagination struct fails to deserialize even
/// a bare `?limit=` — so each concern is extracted on its own.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityFilterQuery {
    /// Keep only these activity types (e.g. `file.created`). Repeat the `type`
    /// parameter for several; omit for no type constraint.
    // Named `types` because `type` is a reserved word; exposed as `type`.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub types: Option<Vec<ActivityType>>,
    /// Username of the account whose activities to keep. Omit for any actor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<Handle>,
}

/// The export-only query parameter: the output format. Kept separate from the
/// shared filter and window so each is extracted on its own (see
/// [`ActivityFilterQuery`] for why flattening is avoided).
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityExportOptions {
    /// Output format; defaults to `csv`.
    #[serde(default)]
    pub format: ExportFormat,
}

impl ActivityFilterQuery {
    /// Builds the repository filter for the paginated feed, folding in the actor
    /// resolved by the handler and the optional date window.
    ///
    /// `actor_id` is the account the [`actor`](Self::actor) username resolved to;
    /// `None` means no actor constraint. The window narrows the feed only where its
    /// bounds are given.
    pub fn to_filter(&self, actor_id: Option<Uuid>, window: &DateWindow) -> Result<ActivityFilter> {
        let (from, to) = window.resolve_optional_bounds()?;
        Ok(ActivityFilter {
            types: self.types.clone().unwrap_or_default(),
            actor: actor_id,
            from,
            to,
        })
    }

    /// Builds the repository filter for the export, folding in the actor resolved
    /// by the handler and the resolved, bounded date window.
    ///
    /// `actor_id` is the account the [`actor`](Self::actor) username resolved to;
    /// `None` means no actor constraint. Unlike the feed, the export's window is
    /// always applied (defaulted and capped) so the result stays bounded.
    pub fn to_export_filter(
        &self,
        actor_id: Option<Uuid>,
        window: &ResolvedWindow,
    ) -> Result<ActivityFilter> {
        Ok(ActivityFilter {
            types: self.types.clone().unwrap_or_default(),
            actor: actor_id,
            from: Some(window.from_timestamp()?),
            to: Some(window.to_timestamp()?),
        })
    }
}

#[cfg(test)]
mod tests {
    use axum::extract::FromRequestParts;
    use axum::http::Request;

    use super::*;
    use crate::extract::Query;
    use crate::handler::request::{CursorPagination, DateWindow};

    /// Extracts a query struct through the real request pipeline — the project's
    /// [`Query`] extractor over `axum_extra::extract::Query` — so the test parses a
    /// `?type=a&type=b` string exactly as the endpoints do (the only path that
    /// collects a repeated key into a `Vec`).
    async fn extract<T: serde::de::DeserializeOwned + Send>(query: &str) -> T {
        let (mut parts, ()) = Request::builder()
            .uri(format!("/?{query}"))
            .body(())
            .expect("request should build")
            .into_parts();
        Query::<T>::from_request_parts(&mut parts, &())
            .await
            .expect("query should extract")
            .into_inner()
    }

    #[tokio::test]
    async fn reads_a_single_repeated_type_key() {
        let query: ActivityFilterQuery = extract("type=file.created").await;
        assert_eq!(
            query.types,
            Some(vec![ActivityType::FileCreated]),
            "a single `type` value should populate the filter",
        );
    }

    #[tokio::test]
    async fn reads_several_repeated_type_keys() {
        let query: ActivityFilterQuery = extract("type=file.created&type=file.deleted").await;
        assert_eq!(
            query.types,
            Some(vec![ActivityType::FileCreated, ActivityType::FileDeleted]),
            "repeated `type` values should all land in the filter",
        );
    }

    #[tokio::test]
    async fn absent_type_leaves_the_filter_unconstrained() {
        let query: ActivityFilterQuery = extract("").await;
        assert_eq!(query.types, None, "no `type` means no type constraint");
    }

    #[tokio::test]
    async fn actor_is_parsed_as_a_username_handle() {
        let query: ActivityFilterQuery = extract("actor=alice").await;
        assert_eq!(query.actor.map(|h| h.to_string()), Some("alice".to_owned()));
    }

    #[tokio::test]
    async fn shared_pagination_parses_alongside_the_filter() {
        // Regression: with the pagination fields flattened into this struct,
        // `serde_html_form` rejected even a bare `?limit=`. Extracting the shared
        // `CursorPagination` on its own keeps it working.
        let pagination: CursorPagination = extract("limit=50").await;
        assert_eq!(pagination.limit, Some(50));

        let pagination: CursorPagination = extract("limit=25&after=abc&includeCount=true").await;
        assert_eq!(pagination.limit, Some(25));
        assert_eq!(pagination.after.as_deref(), Some("abc"));
        assert!(pagination.include_count);
    }

    #[tokio::test]
    async fn shared_window_parses_alongside_the_filter() {
        let window: DateWindow = extract("from=2026-01-01&to=2026-01-31").await;
        assert!(window.from.is_some() && window.to.is_some());
    }
}
