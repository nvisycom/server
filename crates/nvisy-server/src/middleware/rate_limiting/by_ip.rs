//! IP-based rate limiting middleware.

use axum::extract::{Request, State};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum_client_ip::ClientIp;

use crate::service::{RateLimitKey, RateLimiter};

/// Rate limits requests by IP address
///
/// This middleware should be applied to authentication endpoints to prevent:
/// - Brute force attacks
/// - Credential stuffing
/// - Account enumeration
///
/// # Example
///
/// ```rust,no_run
/// use axum::middleware::from_fn_with_state;
/// use nvisy_server::middleware::rate_limit_by_ip;
/// use nvisy_server::service::ServiceState;
///
/// let state = ServiceState::from_config(&config).await?;
/// let middleware = from_fn_with_state(state.clone(), rate_limit_by_ip);
/// ```
pub async fn rate_limit_by_ip(
    ClientIp(ip_address): ClientIp,
    State(rate_limiter): State<RateLimiter>,
    request: Request,
    next: Next,
) -> Response {
    let key = RateLimitKey::from_ip(ip_address);

    match rate_limiter.check(key).await {
        Ok(()) => next.run(request).await,
        Err(error) => error.into_response(),
    }
}

/// Rate limits requests by IP address with strict limits
///
/// Use this for sensitive endpoints like password reset or account creation
pub async fn rate_limit_strict(
    ClientIp(ip_address): ClientIp,
    State(rate_limiter): State<RateLimiter>,
    request: Request,
    next: Next,
) -> Response {
    let key = RateLimitKey::from_ip(ip_address);

    // Use cost of 4 tokens for strict limiting (5 requests = 20 tokens with moderate config)
    match rate_limiter.check_with_cost(key, 4).await {
        Ok(()) => next.run(request).await,
        Err(error) => error.into_response(),
    }
}
