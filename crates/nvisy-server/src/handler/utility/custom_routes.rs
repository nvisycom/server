//! Custom routes utilities for extending the API router.

use std::collections::HashSet;

use aide::axum::ApiRouter;

use crate::service::ServiceState;

/// A built-in handler module whose routes can be excluded from the router.
///
/// A wrapping binary (e.g. the hosted edition) that needs to replace a
/// built-in endpoint excludes its module via [`CustomRoutes::exclude`], then
/// mounts its own routes with [`CustomRoutes::add_private_routes`] /
/// [`CustomRoutes::add_public_routes`]. This is required because merging a
/// second route for the same method and path panics; excluding the built-in
/// one first leaves the path free.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinModule {
    /// Account API tokens.
    Tokens,
    /// Account notifications.
    Notifications,
    /// Workspace invitations.
    Invites,
    /// Webhooks.
    Webhooks,
    /// Authentication (`/auth/*`, public).
    Authentication,
}

/// Type alias for a function that maps/transforms an ApiRouter.
///
/// This is used for applying transformations to routers before or after middlewares.
pub type RouterMapFn = fn(ApiRouter<ServiceState>) -> ApiRouter<ServiceState>;

/// Configuration for custom routes that can be merged into the main API router.
///
/// This struct allows you to extend the API with custom private and public routes
/// while maintaining the same authentication and middleware structure.
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
/// The type parameter `S` is the application state the custom routes are typed
/// to — [`ServiceState`] for the first-party binary (the default), or a
/// downstream state that embeds it. The built-in routes are always assembled
/// against [`ServiceState`] and merged with these by
/// [`routes`](crate::handler::routes)
/// after erasure, so the map-middleware hooks operate on `ServiceState` (the
/// built-in router), not on `S`.
#[derive(Clone)]
pub struct CustomRoutes<S = ServiceState> {
    /// Custom private routes that require authentication.
    pub private_routes: Option<ApiRouter<S>>,
    /// Custom public routes that don't require authentication.
    pub public_routes: Option<ApiRouter<S>>,
    /// Function to map the built-in private routes before middlewares are applied.
    pub private_before_middleware: Option<RouterMapFn>,
    /// Function to map the built-in private routes after middlewares are applied.
    pub private_after_middleware: Option<RouterMapFn>,
    /// Function to map the built-in public routes before middlewares are applied.
    pub public_before_middleware: Option<RouterMapFn>,
    /// Function to map the built-in public routes after middlewares are applied.
    pub public_after_middleware: Option<RouterMapFn>,
    /// Flag to disable authentication routes.
    pub disable_authentication: bool,
    /// Built-in modules to exclude from the router so their routes can be
    /// replaced by custom ones.
    pub excluded_modules: HashSet<BuiltinModule>,
}

impl<S> Default for CustomRoutes<S> {
    /// An empty configuration — no custom routes, hooks, or exclusions. Does not
    /// require `S: Default` (unlike a derived impl), so it works for any state.
    fn default() -> Self {
        Self {
            private_routes: None,
            public_routes: None,
            private_before_middleware: None,
            private_after_middleware: None,
            public_before_middleware: None,
            public_after_middleware: None,
            disable_authentication: false,
            excluded_modules: HashSet::new(),
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

    /// Sets the private routes.
    ///
    /// Private routes will be protected by authentication middleware.
    pub fn with_private_routes(mut self, routes: ApiRouter<S>) -> Self {
        self.private_routes = Some(routes);
        self
    }

    /// Sets the public routes.
    ///
    /// Public routes will be accessible without authentication.
    pub fn with_public_routes(mut self, routes: ApiRouter<S>) -> Self {
        self.public_routes = Some(routes);
        self
    }

    /// Adds custom private routes, merging with existing private routes if any.
    pub fn add_private_routes(mut self, routes: ApiRouter<S>) -> Self {
        self.private_routes = match self.private_routes {
            Some(existing) => Some(existing.merge(routes)),
            None => Some(routes),
        };
        self
    }

    /// Adds custom public routes, merging with existing public routes if any.
    pub fn add_public_routes(mut self, routes: ApiRouter<S>) -> Self {
        match self.public_routes {
            Some(existing) => self.public_routes = Some(existing.merge(routes)),
            None => self.public_routes = Some(routes),
        }
        self
    }

    /// Sets the disable authentication flag.
    ///
    /// When enabled, authentication routes will not be included in the public router.
    pub fn with_disable_authentication(mut self, disable: bool) -> Self {
        self.disable_authentication = disable;
        self
    }

    /// Excludes a built-in module's routes from the router.
    ///
    /// Use this to replace a built-in endpoint: exclude its module, then mount
    /// a custom router with [`Self::add_private_routes`] /
    /// [`Self::add_public_routes`]. Merging a replacement without excluding the
    /// original first panics on the route collision.
    pub fn exclude(mut self, module: BuiltinModule) -> Self {
        self.excluded_modules.insert(module);
        self
    }

    /// Returns true if the given built-in module has been excluded.
    pub fn is_excluded(&self, module: BuiltinModule) -> bool {
        self.excluded_modules.contains(&module)
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

    /// Takes the private routes, leaving `None` in their place.
    pub fn take_private_routes(&mut self) -> Option<ApiRouter<S>> {
        self.private_routes.take()
    }

    /// Takes the public routes, leaving `None` in their place.
    pub fn take_public_routes(&mut self) -> Option<ApiRouter<S>> {
        self.public_routes.take()
    }

    /// Sets a hook to transform the built-in private router before the auth
    /// layers are applied.
    ///
    /// The hook runs on the built-in `ServiceState` router during assembly;
    /// authentication and the merge of any custom private routes happen after it.
    pub fn with_private_before_middleware(mut self, f: RouterMapFn) -> Self {
        self.private_before_middleware = Some(f);
        self
    }

    /// Sets a hook to transform the built-in private router after the before-hook
    /// but still before the auth layers and the custom-route merge.
    ///
    /// The hook runs on the built-in `ServiceState` router while it is still
    /// typed to `ServiceState`, so it cannot see custom `S` routes or the auth
    /// layers.
    pub fn with_private_after_middleware(mut self, f: RouterMapFn) -> Self {
        self.private_after_middleware = Some(f);
        self
    }

    /// Sets a hook to transform the built-in public router (first of the two).
    ///
    /// Runs on the built-in `ServiceState` router during assembly, before custom
    /// public routes are merged.
    pub fn with_public_before_middleware(mut self, f: RouterMapFn) -> Self {
        self.public_before_middleware = Some(f);
        self
    }

    /// Sets a hook to transform the built-in public router (second of the two).
    ///
    /// Runs on the built-in `ServiceState` router during assembly, before custom
    /// public routes are merged.
    pub fn with_public_after_middleware(mut self, f: RouterMapFn) -> Self {
        self.public_after_middleware = Some(f);
        self
    }

    /// Applies the before-middleware function to the built-in private routes if
    /// it exists. Downstream custom routes are merged afterwards and untouched.
    pub(crate) fn map_private_before_middleware(
        &self,
        routes: ApiRouter<ServiceState>,
    ) -> ApiRouter<ServiceState> {
        if let Some(f) = self.private_before_middleware {
            f(routes)
        } else {
            routes
        }
    }

    /// Applies the after-middleware function to the built-in private routes if
    /// it exists. Downstream custom routes are merged afterwards and untouched.
    pub(crate) fn map_private_after_middleware(
        &self,
        routes: ApiRouter<ServiceState>,
    ) -> ApiRouter<ServiceState> {
        if let Some(f) = self.private_after_middleware {
            f(routes)
        } else {
            routes
        }
    }

    /// Applies the before-middleware function to the built-in public routes if
    /// it exists. Downstream custom routes are merged afterwards and untouched.
    pub(crate) fn map_public_before_middleware(
        &self,
        routes: ApiRouter<ServiceState>,
    ) -> ApiRouter<ServiceState> {
        if let Some(f) = self.public_before_middleware {
            f(routes)
        } else {
            routes
        }
    }

    /// Applies the after-middleware function to the built-in public routes if
    /// it exists. Downstream custom routes are merged afterwards and untouched.
    pub(crate) fn map_public_after_middleware(
        &self,
        routes: ApiRouter<ServiceState>,
    ) -> ApiRouter<ServiceState> {
        if let Some(f) = self.public_after_middleware {
            f(routes)
        } else {
            routes
        }
    }
}
