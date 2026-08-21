//! Workspace activity request types.

use nvisy_postgres::query::ActivityFilter;
use nvisy_postgres::types::ActivityType;
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::handler::Result;
use crate::handler::request::{CursorPagination, DateWindow, ExportFormat, ResolvedWindow};

/// Most rows a single export returns. A hard ceiling so one request cannot
/// materialize an unbounded result; a truncated export says so in its response.
pub const MAX_EXPORT_ROWS: usize = 100_000;

/// The non-window activity filters — type and actor — shared by the feed and the
/// export so both narrow the log the same way.
///
/// The two fields are inlined into each query struct rather than `#[serde(flatten)]`ed
/// as a nested group: the query extractor only collects a repeated key like
/// `type=a&type=b` into a `Vec` for a top-level field, not one reached through a
/// flattened map. The shared conversion lives here, in
/// [`build`](ActivityFilterFields::build), instead.
struct ActivityFilterFields {
    types: Option<Vec<ActivityType>>,
    actor: Option<Uuid>,
}

impl ActivityFilterFields {
    /// Builds the repository filter, attaching the resolved half-open time bounds
    /// (either may be `None`).
    fn build(self, from: Option<jiff::Timestamp>, to: Option<jiff::Timestamp>) -> ActivityFilter {
        ActivityFilter {
            types: self.types.unwrap_or_default(),
            actor: self.actor,
            from,
            to,
        }
    }
}

/// Query parameters for the activity feed: the type/actor filter, an optional date
/// window (narrows only when given — the feed is otherwise all-time), and cursor
/// pagination.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityListQuery {
    /// Keep only these activity types (e.g. `file.created`). Repeat the `type`
    /// parameter for several; omit for no type constraint. The field is `types`
    /// since `type` is a reserved word.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub types: Option<Vec<ActivityType>>,
    /// Keep only activities performed by this account. Omit for any actor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<Uuid>,
    /// Optional `from`/`to` day range; each bound narrows the feed only if given.
    #[serde(flatten)]
    pub window: DateWindow,
    /// Cursor pagination.
    #[serde(flatten)]
    pub pagination: CursorPagination,
}

impl ActivityListQuery {
    /// Resolves the repository filter, validating the optional window bounds.
    pub fn to_filter(&self) -> Result<ActivityFilter> {
        let (from, to) = self.window.resolve_optional_bounds()?;
        let fields = ActivityFilterFields {
            types: self.types.clone(),
            actor: self.actor,
        };
        Ok(fields.build(from, to))
    }
}

/// Query parameters for the activity export: the type/actor filter, a bounded date
/// window (defaulted and capped, since the export materializes rows), and the
/// output `format`. See [`DateWindow`] for the range defaults and bounds.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityExportQuery {
    /// Keep only these activity types (e.g. `file.created`). Repeat the `type`
    /// parameter for several; omit for no type constraint. The field is `types`
    /// since `type` is a reserved word.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub types: Option<Vec<ActivityType>>,
    /// Keep only activities performed by this account. Omit for any actor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<Uuid>,
    /// The `from`/`to` day range.
    #[serde(flatten)]
    pub window: DateWindow,
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
    pub fn resolve(&self) -> Result<ResolvedExport> {
        let window = self.window.resolve()?;
        let fields = ActivityFilterFields {
            types: self.types.clone(),
            actor: self.actor,
        };
        let filter = fields.build(Some(window.from_timestamp()?), Some(window.to_timestamp()?));
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
}
