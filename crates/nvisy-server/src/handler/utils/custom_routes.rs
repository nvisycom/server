//! Custom routes utilities for extending the API router.

use utoipa_axum::router::OpenApiRouter;

use crate::service::ServiceState;

/// Configuration for custom routes that can be merged into the main API router.
///
/// This struct allows you to extend the API with custom private and public routes
/// while maintaining the same authentication and middleware structure.
///
/// # Examples
///
/// ```rust
/// use nvisy_server::handler::utils::CustomRoutes;
/// use utoipa_axum::router::OpenApiRouter;
///
/// let custom = CustomRoutes::new()
///     .with_private_routes(some_private_router)
///     .with_public_routes(some_public_router);
/// ```
#[derive(Default, Clone)]
pub struct CustomRoutes {
    /// Custom private routes that require authentication.
    pub private_routes: Option<OpenApiRouter<ServiceState>>,
    /// Custom public routes that don't require authentication.
    pub public_routes: Option<OpenApiRouter<ServiceState>>,
}

impl CustomRoutes {
    /// Creates a new empty `CustomRoutes` instance.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the private routes.
    ///
    /// Private routes will be protected by authentication middleware.
    pub fn with_private_routes(mut self, routes: OpenApiRouter<ServiceState>) -> Self {
        self.private_routes = Some(routes);
        self
    }

    /// Sets the public routes.
    ///
    /// Public routes will be accessible without authentication.
    pub fn with_public_routes(mut self, routes: OpenApiRouter<ServiceState>) -> Self {
        self.public_routes = Some(routes);
        self
    }

    /// Adds custom private routes, merging with existing private routes if any.
    pub fn add_private_routes(mut self, routes: OpenApiRouter<ServiceState>) -> Self {
        match self.private_routes {
            Some(existing) => self.private_routes = Some(existing.merge(routes)),
            None => self.private_routes = Some(routes),
        }
        self
    }

    /// Adds custom public routes, merging with existing public routes if any.
    pub fn add_public_routes(mut self, routes: OpenApiRouter<ServiceState>) -> Self {
        match self.public_routes {
            Some(existing) => self.public_routes = Some(existing.merge(routes)),
            None => self.public_routes = Some(routes),
        }
        self
    }

    /// Returns true if there are any private routes configured.
    pub fn has_private_routes(&self) -> bool {
        self.private_routes.is_some()
    }

    /// Returns true if there are any public routes configured.
    pub fn has_public_routes(&self) -> bool {
        self.public_routes.is_some()
    }

    /// Returns true if no custom routes are configured.
    pub fn is_empty(&self) -> bool {
        !self.has_private_routes() && !self.has_public_routes()
    }

    /// Merges this `CustomRoutes` with another, combining all routes.
    pub fn merge(mut self, other: CustomRoutes) -> Self {
        if let Some(other_private) = other.private_routes {
            self = self.add_private_routes(other_private);
        }
        if let Some(other_public) = other.public_routes {
            self = self.add_public_routes(other_public);
        }
        self
    }

    /// Takes the private routes, leaving `None` in their place.
    pub fn take_private_routes(&mut self) -> Option<OpenApiRouter<ServiceState>> {
        self.private_routes.take()
    }

    /// Takes the public routes, leaving `None` in their place.
    pub fn take_public_routes(&mut self) -> Option<OpenApiRouter<ServiceState>> {
        self.public_routes.take()
    }
}

#[cfg(test)]
mod tests {
    use utoipa_axum::router::OpenApiRouter;

    use super::*;

    #[test]
    fn test_custom_routes_new() {
        let routes = CustomRoutes::new();
        assert!(routes.is_empty());
        assert!(!routes.has_private_routes());
        assert!(!routes.has_public_routes());
    }

    #[test]
    fn test_custom_routes_builder() {
        let private_router = OpenApiRouter::new();
        let public_router = OpenApiRouter::new();

        let routes = CustomRoutes::new()
            .with_private_routes(private_router)
            .with_public_routes(public_router);

        assert!(routes.has_private_routes());
        assert!(routes.has_public_routes());
        assert!(!routes.is_empty());
    }

    #[test]
    fn test_custom_routes_merge() {
        let routes1 = CustomRoutes::new().with_private_routes(OpenApiRouter::new());

        let routes2 = CustomRoutes::new().with_public_routes(OpenApiRouter::new());

        let merged = routes1.merge(routes2);
        assert!(merged.has_private_routes());
        assert!(merged.has_public_routes());
    }
}
