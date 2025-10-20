use axum::extract::rejection::FormRejection;
use axum::extract::{Form as AxumForm, FromRequest, OptionalFromRequest, Request};
use derive_more::{Deref, DerefMut, From};
use serde::de::DeserializeOwned;

use crate::handler::{Error, ErrorKind};

/// Enhanced form data extractor with improved error handling and type safety.
///
/// This extractor provides better error messages compared to the
/// default Axum Form extractor. It includes:
/// - Detailed error messages for different form parsing failures
/// - Type-safe deserialization with proper error context
/// - Clear indication of which fields failed validation
/// - Content-Type validation with helpful suggestions
///
/// # Content-Type Requirements
///
/// This extractor expects `application/x-www-form-urlencoded` data.
/// If a different content type is provided, a helpful error message
/// will suggest the correct content type.
///
/// # Examples
///
/// ```rust,no_run
/// use nvisy_server::extract::Form;
/// use serde::Deserialize;
/// use uuid::Uuid;
///
/// #[derive(Deserialize)]
/// struct LoginForm {
///     email: String,
///     password: String,
///     remember_me: Option<bool>,
/// }
///
/// async fn login(Form(form): Form<LoginForm>) -> Result<(), ()> {
///     println!("Login attempt for: {}", form.email);
///     if form.remember_me.unwrap_or(false) {
///         println!("Remember me enabled");
///     }
///     Ok(())
/// }
/// ```
///
/// # Usage with HTML Forms
///
/// ```html
/// <form method="POST" action="/login">
///   <input type="email" name="email" required>
///   <input type="password" name="password" required>
///   <input type="checkbox" name="remember_me" value="true">
///   <button type="submit">Login</button>
/// </form>
/// ```
///
/// # Error Handling
///
/// Unlike the default Axum Form extractor, this provides detailed error
/// messages when form parsing fails:
///
/// - Missing required fields with field names
/// - Type conversion failures with context
/// - Invalid Content-Type with suggestions
/// - Malformed form data with helpful guidance
///
/// All errors are automatically converted to appropriate HTTP responses
/// with detailed error messages for better API debugging and user experience.
#[must_use]
#[derive(Debug, Clone, Copy, Default, Deref, DerefMut, From)]
pub struct Form<T>(pub T);

impl<T> Form<T> {
    /// Creates a new [`Form`] wrapper around the provided form data.
    #[inline]
    pub fn new(inner: T) -> Self {
        Self(inner)
    }

    /// Consumes the wrapper and returns the inner form data.
    #[inline]
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T, S> FromRequest<S> for Form<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Error<'static>;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match AxumForm::<T>::from_request(req, state).await {
            Ok(AxumForm(form)) => Ok(Form(form)),
            Err(rejection) => Err(enhance_form_error(rejection)),
        }
    }
}

impl<T, S> OptionalFromRequest<S> for Form<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Error<'static>;

    async fn from_request(req: Request, state: &S) -> Result<Option<Self>, Self::Rejection> {
        match AxumForm::<T>::from_request(req, state).await {
            Ok(AxumForm(form)) => Ok(Some(Form(form))),
            Err(_) => Ok(None),
        }
    }
}

/// Enhances form parsing errors with detailed context and user-friendly messages.
///
/// This function takes the raw Axum form rejection and converts it into a more
/// informative error that helps developers understand what went wrong with the
/// form data parsing.
fn enhance_form_error(rejection: FormRejection) -> Error<'static> {
    tracing::debug!(
        target: "nvisy::extract::form",
        error = %rejection,
        "Form data parsing failed"
    );

    match rejection {
        FormRejection::FailedToDeserializeForm(err) => {
            // Extract the inner serde_urlencoded error for more specific handling
            let error_message = err.to_string();

            if error_message.contains("missing field") {
                let field_name = extract_field_name_from_error(&error_message);
                ErrorKind::BadRequest
                    .with_message("Missing required form field")
                    .with_context(format!(
                        "The form field '{}' is required but was not provided",
                        field_name.unwrap_or("unknown")
                    ))
            } else if error_message.contains("invalid type")
                || error_message.contains("invalid value")
            {
                ErrorKind::BadRequest
                    .with_message("Invalid form field value")
                    .with_context(format!(
                        "Failed to parse form field: {}. Please check the field format and try again",
                        error_message
                    ))
            } else if error_message.contains("duplicate field") {
                let field_name = extract_field_name_from_error(&error_message);
                ErrorKind::BadRequest
                    .with_message("Duplicate form field")
                    .with_context(format!(
                        "The form field '{}' was provided multiple times. Please provide it only once",
                        field_name.unwrap_or("unknown")
                    ))
            } else {
                ErrorKind::BadRequest
                    .with_message("Invalid form data")
                    .with_context(format!("Failed to parse form data: {}", error_message))
            }
        }
        FormRejection::InvalidFormContentType(err) => ErrorKind::BadRequest
            .with_message("Invalid content type for form data")
            .with_context(format!(
                "Expected 'application/x-www-form-urlencoded' content type, but received: {}. \
                    Please set the correct Content-Type header for form submissions",
                err
            )),
        FormRejection::BytesRejection(err) => ErrorKind::BadRequest
            .with_message("Failed to read form data")
            .with_context(format!(
                "Could not read the request body as form data: {}. \
                    This might indicate a network issue or malformed request",
                err
            )),
        _ => {
            // Fallback for other form rejection types
            ErrorKind::BadRequest
                .with_message("Invalid form submission")
                .with_context(
                    "The form data could not be processed. Please check your form fields and try again"
                )
        }
    }
}

/// Attempts to extract the field name from a serde error message.
///
/// This is a best-effort function that tries to parse field names from
/// error messages to provide more helpful error context.
fn extract_field_name_from_error(error_message: &str) -> Option<&str> {
    // Try to extract field name from common serde error patterns
    if let Some(start) = error_message.find('`')
        && let Some(end) = error_message[start + 1..].find('`')
    {
        return Some(&error_message[start + 1..start + 1 + end]);
    }

    // Try alternative patterns for "missing field X"
    if error_message.contains("missing field ")
        && let Some(start) = error_message.find("missing field ")
    {
        let field_part = &error_message[start + 14..]; // "missing field " is 14 chars
        if let Some(end) = field_part.find(' ') {
            return Some(&field_part[..end]);
        } else {
            // Field name might be at the end of the message
            return Some(field_part.trim());
        }
    }

    // Try pattern for "duplicate field X"
    if error_message.contains("duplicate field ")
        && let Some(start) = error_message.find("duplicate field ")
    {
        let field_part = &error_message[start + 16..]; // "duplicate field " is 16 chars
        if let Some(end) = field_part.find(' ') {
            return Some(&field_part[..end]);
        } else {
            return Some(field_part.trim());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_field_name_from_error() {
        // Test backtick pattern
        assert_eq!(
            extract_field_name_from_error("missing field `username`"),
            Some("username")
        );

        // Test missing field pattern
        assert_eq!(
            extract_field_name_from_error("missing field email at line 1"),
            Some("email")
        );

        // Test duplicate field pattern
        assert_eq!(
            extract_field_name_from_error("duplicate field password at line 1"),
            Some("password")
        );

        // Test field at end of message
        assert_eq!(
            extract_field_name_from_error("missing field remember_me"),
            Some("remember_me")
        );

        // Test no match
        assert_eq!(extract_field_name_from_error("some other error"), None);
    }

    #[test]
    fn test_form_creation() {
        let form = Form::new("test".to_string());
        assert_eq!(form.into_inner(), "test");
    }

    #[test]
    fn test_form_deref() {
        let form = Form::new("test".to_string());
        assert_eq!(*form, "test");
    }
}
