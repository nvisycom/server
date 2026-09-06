//! Adapter that lets the `oauth2` crate make its token requests through a shared
//! [`reqwest::Client`], instead of pulling its own HTTP client.
//!
//! `oauth2`'s async API accepts any `Fn(HttpRequest) -> Future<Result<HttpResponse, E>>`
//! as its HTTP client. [`reqwest_client`] returns such a closure over a borrowed
//! `reqwest::Client`, translating between the `http` crate's request/response
//! types (which `oauth2` speaks) and `reqwest`.

use oauth2::{HttpRequest, HttpResponse};

/// An error from the OAuth HTTP transport.
#[derive(Debug, thiserror::Error)]
pub enum HttpClientError {
    /// The underlying reqwest call failed.
    #[error("oauth http request failed: {0}")]
    Reqwest(#[from] reqwest::Error),
    /// The response could not be rebuilt as an `http::Response`.
    #[error("oauth http response was invalid: {0}")]
    Response(#[from] http::Error),
}

/// Builds an `oauth2` async HTTP client backed by `client`.
///
/// The client is owned by the returned closure (a `reqwest::Client` is an `Arc`
/// handle, so cloning is cheap) rather than borrowed, so the token-request
/// futures stay `Send` for any lifetime and can cross a task boundary.
pub fn reqwest_client(
    client: reqwest::Client,
) -> impl Fn(
    HttpRequest,
) -> std::pin::Pin<Box<dyn Future<Output = Result<HttpResponse, HttpClientError>> + Send>> {
    move |request: HttpRequest| Box::pin(send(client.clone(), request))
}

/// Sends one `oauth2` request through reqwest and returns the reply.
async fn send(
    client: reqwest::Client,
    request: HttpRequest,
) -> Result<HttpResponse, HttpClientError> {
    let (parts, body) = request.into_parts();

    let response = client
        .request(parts.method, parts.uri.to_string())
        .headers(parts.headers)
        .body(body)
        .send()
        .await?;

    let status = response.status();
    let headers = response.headers().clone();
    let body = response.bytes().await?.to_vec();

    let mut builder = http::Response::builder().status(status);
    if let Some(dst) = builder.headers_mut() {
        *dst = headers;
    }
    Ok(builder.body(body)?)
}
