//! Workspace activity request types.

use jiff::civil::Date;
use nvisy_postgres::query::ActivityFilter;
use nvisy_postgres::types::{ActivityType, Handle};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::handler::Result;
use crate::handler::request::{CursorPagination, DateWindow, ExportFormat, ResolvedWindow};

/// Most rows a single export returns. A hard ceiling so one request cannot
/// materialize an unbounded result; a truncated export says so in its response.
pub const MAX_EXPORT_ROWS: usize = 100_000;

/// Query parameters for the activity feed: the type/actor filter, an optional date
/// window (narrows only when given — the feed is otherwise all-time), and cursor
/// pagination.
///
/// Every field is a top-level query key rather than a `#[serde(flatten)]`ed
/// sub-struct: the query extractor (`serde_html_form`) mis-handles flattened
/// structs — a flattened `CursorPagination` fails to deserialize even a bare
/// `?limit=` — so the window and pagination fields are inlined and their helper
/// types rebuilt in the accessors.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityListQuery {
    /// Keep only these activity types (e.g. `file.created`). Repeat the `type`
    /// parameter for several; omit for no type constraint.
    // Named `types` because `type` is a reserved word; exposed as `type`.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub types: Option<Vec<ActivityType>>,
    /// Username of the account whose activities to keep. Omit for any actor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<Handle>,
    /// First day of the range (inclusive), `YYYY-MM-DD`. Narrows the feed only if
    /// given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Date>,
    /// Last day of the range (inclusive), `YYYY-MM-DD`. Narrows the feed only if
    /// given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Date>,
    /// Maximum number of records to return (1-100, default 20).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    /// Cursor pointing to the last item of the previous page (from `nextCursor`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    /// Whether to include the total item count in the response's `total` field.
    #[serde(default)]
    pub include_count: bool,
}

impl ActivityListQuery {
    /// Resolves the repository filter, validating the optional window bounds.
    /// `actor_id` is the account id the [`actor`](Self::actor) username resolved to
    /// (the handler resolves it); `None` means no actor constraint.
    pub fn to_filter(&self, actor_id: Option<Uuid>) -> Result<ActivityFilter> {
        let (from, to) = self.window().resolve_optional_bounds()?;
        Ok(ActivityFilter {
            types: self.types.clone().unwrap_or_default(),
            actor: actor_id,
            from,
            to,
        })
    }

    /// The cursor pagination for this request.
    pub fn pagination(&self) -> CursorPagination {
        CursorPagination {
            limit: self.limit,
            after: self.after.clone(),
            include_count: self.include_count,
        }
    }

    fn window(&self) -> DateWindow {
        DateWindow {
            from: self.from,
            to: self.to,
        }
    }
}

/// Query parameters for the activity export: the type/actor filter, a bounded date
/// window (defaulted and capped, since the export materializes rows), and the
/// output `format`. See [`DateWindow`] for the range defaults and bounds.
///
/// Fields are inlined rather than `#[serde(flatten)]`ed for the same reason as
/// [`ActivityListQuery`].
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityExportQuery {
    /// Keep only these activity types (e.g. `file.created`). Repeat the `type`
    /// parameter for several; omit for no type constraint.
    // Named `types` because `type` is a reserved word; exposed as `type`.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub types: Option<Vec<ActivityType>>,
    /// Username of the account whose activities to keep. Omit for any actor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<Handle>,
    /// First day of the range (inclusive), `YYYY-MM-DD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<Date>,
    /// Last day of the range (inclusive), `YYYY-MM-DD`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<Date>,
    /// Output format; defaults to `csv`.
    #[serde(default)]
    pub format: ExportFormat,
}

/// A validated export request: the repository filter (with the resolved window
/// folded in) and the chosen format.
#[derive(Debug, Clone)]
pub struct ResolvedExport {
    /// The repository filter, including the export's resolved time window.
    pub filter: ActivityFilter,
    /// The validated day range (for naming the file).
    pub window: ResolvedWindow,
    /// Output format.
    pub format: ExportFormat,
}

impl ActivityExportQuery {
    /// Resolves the request: validates and defaults the window, folds it plus the
    /// type/actor filter into one repository filter, and carries the format.
    /// `actor_id` is the account id the [`actor`](Self::actor) username resolved to.
    pub fn resolve(&self, actor_id: Option<Uuid>) -> Result<ResolvedExport> {
        let window = DateWindow {
            from: self.from,
            to: self.to,
        }
        .resolve()?;
        let filter = ActivityFilter {
            types: self.types.clone().unwrap_or_default(),
            actor: actor_id,
            from: Some(window.from_timestamp()?),
            to: Some(window.to_timestamp()?),
        };
        Ok(ResolvedExport {
            filter,
            window,
            format: self.format,
        })
    }
}

#[cfg(test)]
mod tests {
    use axum::extract::FromRequestParts;
    use axum::http::Request;

    use super::*;
    use crate::extract::Query;

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
    async fn list_query_reads_a_single_repeated_type_key() {
        let query: ActivityListQuery = extract("type=file.created").await;
        assert_eq!(
            query.types,
            Some(vec![ActivityType::FileCreated]),
            "a single `type` value should populate the filter",
        );
    }

    #[tokio::test]
    async fn list_query_reads_several_repeated_type_keys() {
        let query: ActivityListQuery = extract("type=file.created&type=file.deleted").await;
        assert_eq!(
            query.types,
            Some(vec![ActivityType::FileCreated, ActivityType::FileDeleted]),
            "repeated `type` values should all land in the filter",
        );
    }

    #[tokio::test]
    async fn export_query_reads_a_single_repeated_type_key() {
        let query: ActivityExportQuery = extract("type=member.added").await;
        assert_eq!(
            query.types,
            Some(vec![ActivityType::MemberAdded]),
            "a single `type` value should populate the filter",
        );
    }

    #[tokio::test]
    async fn export_query_reads_several_repeated_type_keys() {
        let query: ActivityExportQuery = extract("type=member.added&type=member.deleted").await;
        assert_eq!(
            query.types,
            Some(vec![ActivityType::MemberAdded, ActivityType::MemberDeleted]),
            "repeated `type` values should all land in the filter",
        );
    }

    #[tokio::test]
    async fn absent_type_leaves_the_filter_unconstrained() {
        let list: ActivityListQuery = extract("").await;
        assert_eq!(list.types, None, "no `type` means no type constraint");
        let export: ActivityExportQuery = extract("").await;
        assert_eq!(export.types, None);
    }

    #[tokio::test]
    async fn list_query_parses_pagination_and_window_without_flatten() {
        // Regression: with the fields flattened, `serde_html_form` rejected even a
        // bare `?limit=`. Inlining them fixes it.
        let query: ActivityListQuery = extract("limit=50").await;
        assert_eq!(query.limit, Some(50));
        assert_eq!(query.pagination().limit, Some(50));

        let query: ActivityListQuery = extract("limit=25&after=abc&includeCount=true").await;
        assert_eq!(query.limit, Some(25));
        assert_eq!(query.after.as_deref(), Some("abc"));
        assert!(query.include_count);

        let query: ActivityListQuery = extract("from=2026-01-01&to=2026-01-31").await;
        assert!(query.from.is_some() && query.to.is_some());
    }

    #[tokio::test]
    async fn actor_is_parsed_as_a_username_handle() {
        let query: ActivityListQuery = extract("actor=alice").await;
        assert_eq!(query.actor.map(|h| h.to_string()), Some("alice".to_owned()));
    }
}
