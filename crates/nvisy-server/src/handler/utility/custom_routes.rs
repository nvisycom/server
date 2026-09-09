//! Custom routes utilities for extending the API router.

use aide::axum::ApiRouter;

use crate::service::ServiceState;

/// Custom routes a wrapping binary contributes alongside the built-in ones.
///
/// A host embeds this crate's [`ServiceState`] in its own state `S` and adds its
/// own private (authenticated) and public routes; [`routes`](crate::handler::routes)
/// merges them with the built-ins under one final state type.
///
/// # Examples
///
/// ```rust
/// use nvisy_server::handler::CustomRoutes;
/// use nvisy_server::service::ServiceState;
///
/// let custom = CustomRoutes::<ServiceState>::new();
/// assert!(custom.is_empty());
/// ```
///
/// The type parameter `S` is the application state the custom routes are typed
/// to — [`ServiceState`] for the first-party binary (the default), or a
/// downstream state that embeds it.
#[derive(Clone)]
pub struct CustomRoutes<S = ServiceState> {
    /// Custom private routes that require authentication.
    pub private_routes: Option<ApiRouter<S>>,
    /// Custom public routes that don't require authentication.
    pub public_routes: Option<ApiRouter<S>>,
}

impl<S> Default for CustomRoutes<S> {
    /// An empty configuration — no custom routes. Does not require `S: Default`
    /// (unlike a derived impl), so it works for any state.
    fn default() -> Self {
        Self {
            private_routes: None,
            public_routes: None,
        }
    }
}

impl<S> CustomRoutes<S>
where
    S: Clone + Send + Sync + 'static,
{
    /// Creates a new empty `CustomRoutes` instance.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds custom private routes (authenticated), merging with any already set.
    pub fn add_private_routes(mut self, routes: ApiRouter<S>) -> Self {
        self.private_routes = Some(match self.private_routes {
            Some(existing) => existing.merge(routes),
            None => routes,
        });
        self
    }

    /// Adds custom public routes (unauthenticated), merging with any already set.
    pub fn add_public_routes(mut self, routes: ApiRouter<S>) -> Self {
        self.public_routes = Some(match self.public_routes {
            Some(existing) => existing.merge(routes),
            None => routes,
        });
        self
    }

    /// Returns true if no custom routes are configured.
    pub fn is_empty(&self) -> bool {
        self.private_routes.is_none() && self.public_routes.is_none()
    }
}
