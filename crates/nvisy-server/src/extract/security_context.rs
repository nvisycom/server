//! Request security-context extractor: the caller's client IP and user agent.
//!
//! A single extractor so any handler that records who did something (the activity
//! log, audit trails) captures the same fields the same way, rather than poking
//! at connection info and headers ad hoc.

use aide::OperationInput;
use aide::generate::GenContext;
use aide::openapi::Operation;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::TypedHeader;
use axum_extra::headers::UserAgent;
use ipnet::IpNet;

use crate::extract::ClientIp;

/// The caller's client IP and user agent, for stamping onto security-relevant
/// records. Both are best-effort: a missing or unreadable value is `None` rather
/// than a request rejection, since the context annotates an action but never
/// gates it.
#[derive(Debug, Clone, Default)]
#[must_use]
pub struct SecurityContext {
    /// Client IP as a host prefix (`/32` or `/128`), if resolved.
    pub ip_address: Option<IpNet>,
    /// Client `User-Agent` header, if present.
    pub user_agent: Option<String>,
}

impl<S> FromRequestParts<S> for SecurityContext
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let ip_address = ClientIp::from_request_parts(parts, state).await;
        let ip_address = ip_address.ok().map(|ip| IpNet::from(ip.0));

        let user_agent = TypedHeader::<UserAgent>::from_request_parts(parts, state).await;
        let user_agent = user_agent.ok().map(|header| header.0.to_string());

        Ok(Self {
            ip_address,
            user_agent,
        })
    }
}

impl OperationInput for SecurityContext {
    fn operation_input(_ctx: &mut GenContext, _operation: &mut Operation) {
        // Derived from transport metadata (peer IP, User-Agent), not a documented
        // request parameter, so it contributes nothing to the OpenAPI operation.
    }
}
