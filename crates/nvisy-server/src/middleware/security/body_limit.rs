//! Request body size limiting middleware.

use tower_http::limit::RequestBodyLimitLayer;

/// Default maximum request body size: 16MB
#[allow(dead_code)]
pub const DEFAULT_MAX_BODY_SIZE: usize = 16 * 1024 * 1024;

/// Creates a request body size limit layer with the default size (16MB).
#[allow(dead_code)]
pub fn create_default_body_limit_layer() -> RequestBodyLimitLayer {
    RequestBodyLimitLayer::new(DEFAULT_MAX_BODY_SIZE)
}

/// Creates a request body size limit layer with a custom size.
///
/// # Arguments
///
/// * `max_size` - Maximum allowed request body size in bytes
///
/// # Examples
///
/// ```rust
/// use nvisy_server::middleware::security::create_body_limit_layer;
///
/// // Allow up to 32MB
/// let layer = create_body_limit_layer(32 * 1024 * 1024);
/// ```
pub fn create_body_limit_layer(max_size: usize) -> RequestBodyLimitLayer {
    RequestBodyLimitLayer::new(max_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_max_body_size() {
        assert_eq!(DEFAULT_MAX_BODY_SIZE, 16 * 1024 * 1024);
    }

    #[test]
    fn test_create_default_body_limit_layer() {
        let _layer = create_default_body_limit_layer();
        // Layer creation should not panic
    }

    #[test]
    fn test_create_body_limit_layer() {
        let _layer = create_body_limit_layer(1024 * 1024); // 1MB
        // Layer creation should not panic
    }
}
