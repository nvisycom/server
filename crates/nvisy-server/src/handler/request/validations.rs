//! Request validation utilities.

use validator::ValidationError;

pub fn validation_error(code: &'static str, message: &str) -> ValidationError {
    let mut error = ValidationError::new(code);
    error.message = Some(message.to_string().into());
    error
}

pub fn is_alphanumeric(tags: &[String]) -> Result<(), ValidationError> {
    for (index, tag) in tags.iter().enumerate() {
        if !tag
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        {
            let mut error = ValidationError::new("alphanumeric");
            error.message =
                Some(format!("Tag at index {} contains invalid characters", index).into());
            return Err(error);
        }
    }

    Ok(())
}

// Private helper functions for text sanitization
fn normalize_string(input: &str) -> String {
    input.trim().to_lowercase()
}

/// Trait for normalizing required/non-optional request data
pub trait Normalized {
    /// Normalize a string field (trim whitespace)
    fn normalized_string(&self) -> String;
}

/// Trait for normalizing optional request data
pub trait OptionNormalized {
    /// Normalize an optional string field
    fn normalized_option(&self) -> Option<String>;
}

impl Normalized for String {
    fn normalized_string(&self) -> String {
        normalize_string(self)
    }
}

impl OptionNormalized for Option<String> {
    fn normalized_option(&self) -> Option<String> {
        self.as_ref().map(|s| normalize_string(s))
    }
}
