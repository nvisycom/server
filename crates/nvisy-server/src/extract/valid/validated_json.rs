//! Validated JSON extractor with automatic validation.
//!
//! This module provides [`ValidateJson`], a JSON extractor that deserializes a
//! body (via [`Json`]) and then runs `garde::Validate` on it, turning any
//! [`Report`] into a structured [`Error`].
//!
//! garde is used rather than `validator` specifically because its error type
//! carries only the field path and a message — never the rejected value — so a
//! failed validation cannot leak submitted request contents into the response
//! or the logs. Nested (`#[garde(dive)]`) failures are reported under their
//! dotted path (`files[2].name`) by garde itself.

use aide::OperationInput;
use aide::generate::GenContext;
use aide::openapi::{Operation, Response};
use axum::extract::{FromRequest, OptionalFromRequest, Request};
use derive_more::{Deref, DerefMut, From};
use garde::{Report, Validate};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use crate::extract::Json;
use crate::response::{Error, ErrorKind};

/// JSON extractor that deserializes and then validates the request body.
///
/// Works with any type that implements both [`serde::Deserialize`] and
/// [`garde::Validate`] with a `()` context. Deserialization is delegated to
/// [`Json`], so JSON syntax and content-type errors carry that extractor's
/// messages; a body that parses but fails validation is rejected with a
/// field-by-field message built from the garde [`Report`].
#[must_use]
#[derive(Debug, Clone, Copy, Default, Deref, DerefMut, From)]
pub struct ValidateJson<T>(pub T);

impl<T, S> FromRequest<S> for ValidateJson<T>
where
    T: DeserializeOwned + Validate<Context = ()> + 'static,
    S: Send + Sync,
{
    type Rejection = Error<'static>;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(data) = <Json<T> as FromRequest<S>>::from_request(req, state).await?;
        data.validate()?;
        Ok(Self(data))
    }
}

impl<T, S> OptionalFromRequest<S> for ValidateJson<T>
where
    T: DeserializeOwned + Validate<Context = ()> + 'static,
    S: Send + Sync,
{
    type Rejection = Error<'static>;

    /// Extracts and validates a body only when one is present. A genuinely absent
    /// body yields `None`; a present-but-broken body (malformed JSON, wrong
    /// content-type) or one that fails validation is propagated as an error rather
    /// than silently treated as absent, so an optional-body handler cannot mistake
    /// an invalid payload for "no payload".
    async fn from_request(req: Request, state: &S) -> Result<Option<Self>, Self::Rejection> {
        let Some(Json(data)) =
            <Json<T> as OptionalFromRequest<S>>::from_request(req, state).await?
        else {
            return Ok(None);
        };
        data.validate()?;
        Ok(Some(Self(data)))
    }
}

impl From<Report> for Error<'static> {
    fn from(report: Report) -> Self {
        let messages: Vec<String> = report
            .iter()
            .map(|(path, error)| describe_error(path, error))
            .collect();

        // The report holds only paths and messages, never the submitted value,
        // so logging it in full cannot leak request contents.
        tracing::warn!(errors = %report, "Request validation failed");

        let user_message = match messages.as_slice() {
            [] => "Validation failed".to_string(),
            [single] => single.clone(),
            many => many.join(". "),
        };

        ErrorKind::BadRequest
            .with_message(user_message)
            .with_resource("request")
    }
}

/// Renders one report entry into a user-facing message.
///
/// A top-level error (empty path) is surfaced as-is; a field error is prefixed
/// with its dotted path so the client can see which field failed.
fn describe_error(path: &garde::Path, error: &garde::Error) -> String {
    if path.is_empty() {
        error.message().to_string()
    } else {
        format!("Field '{}' {}", path, error.message())
    }
}

impl<T> OperationInput for ValidateJson<T>
where
    T: JsonSchema,
{
    fn operation_input(ctx: &mut GenContext, operation: &mut Operation) {
        Json::<T>::operation_input(ctx, operation);
    }

    fn inferred_early_responses(
        ctx: &mut GenContext,
        operation: &mut Operation,
    ) -> Vec<(Option<aide::openapi::StatusCode>, Response)> {
        Json::<T>::inferred_early_responses(ctx, operation)
    }
}

#[cfg(test)]
mod tests {
    use garde::Validate;

    use super::describe_error;

    #[derive(Validate)]
    struct Sample {
        #[garde(length(chars, min = 2, max = 4))]
        name: String,
        #[garde(email)]
        email: String,
        #[garde(dive)]
        nested: Vec<Inner>,
    }

    #[derive(Validate)]
    struct Inner {
        #[garde(range(min = 1, max = 10))]
        count: u32,
    }

    /// Collects the report of a failed validation as `path -> message` pairs.
    fn report_of(sample: &Sample) -> Vec<(String, String)> {
        let report = sample.validate().expect_err("sample should be invalid");
        report
            .iter()
            .map(|(path, error)| (path.to_string(), error.message().to_string()))
            .collect()
    }

    #[test]
    fn a_field_error_is_prefixed_with_its_path() {
        let out = describe_error(
            &garde::Path::new("name"),
            &garde::Error::new("length is lower than 2"),
        );
        assert_eq!(out, "Field 'name' length is lower than 2");
    }

    #[test]
    fn a_top_level_error_has_no_prefix() {
        let out = describe_error(&garde::Path::empty(), &garde::Error::new("is invalid"));
        assert_eq!(out, "is invalid");
    }

    #[test]
    fn length_counts_characters_not_bytes() {
        // Four multi-byte characters (12 bytes) must pass a max=4 *character*
        // bound; a byte-counting rule would wrongly reject this.
        let sample = Sample {
            name: "\u{00e9}\u{00e9}\u{00e9}\u{00e9}".to_string(),
            email: "user@example.com".to_string(),
            nested: vec![Inner { count: 5 }],
        };
        assert!(sample.validate().is_ok());
    }

    #[test]
    fn nested_dive_errors_are_reported_under_an_indexed_path() {
        let sample = Sample {
            name: "ok".to_string(),
            email: "user@example.com".to_string(),
            nested: vec![Inner { count: 0 }],
        };
        let report = report_of(&sample);
        assert!(
            report.iter().any(|(path, _)| path == "nested[0].count"),
            "expected an indexed nested path, got {report:?}"
        );
    }

    #[test]
    fn an_invalid_email_is_reported_on_its_field() {
        let sample = Sample {
            name: "ok".to_string(),
            email: "not-an-email".to_string(),
            nested: vec![Inner { count: 5 }],
        };
        let report = report_of(&sample);
        assert!(report.iter().any(|(path, _)| path == "email"));
    }
}
