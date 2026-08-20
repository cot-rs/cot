//! Router for passing requests to their respective views.
//!
//! # Examples
//!
//! ```
//! use cot::request::Request;
//! use cot::response::Response;
//! use cot::router::{Route, Router};
//!
//! async fn home(request: Request) -> cot::Result<Response> {
//!     Ok(cot::reverse_redirect!(request, "get_page", page = 123)?)
//! }
//!
//! async fn get_page(request: Request) -> cot::Result<Response> {
//!     unimplemented!()
//! }
//!
//! let router = Router::with_urls([Route::with_handler_and_name(
//!     "/{page}", get_page, "get_page",
//! )]);
//! ```

use std::collections::HashMap;
use std::fmt::Formatter;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use cot::router::path::AbsolutePath;
use cot_core::error::impl_into_cot_error;
use cot_core::handler::{BoxRequestHandler, RequestHandler, into_box_request_handler};
use cot_core::request::{AppName, RouteName};
use derive_more::with_trait::Debug;
use tracing::debug;

use crate::error::NotFound;
use crate::request::{PathParams, Request, RequestExt, RequestHead};
use crate::response::Response;
use crate::router::path::{PathMatcher, ReverseParamMap};
use crate::router::tree::{Entry, RouteTrie};
use crate::{Error, ProjectContext, Result};

pub mod method;
pub mod path;
mod tree;

/// A router that can be used to route requests to their respective views.
///
/// This struct is responsible for routing requests to their respective views.
/// It can be created directly by calling the [`Router::with_urls`] method, and
/// that's what is typically done in [`cot::App::router`] implementations.
///
/// # Examples
///
/// ```
/// use cot::request::Request;
/// use cot::response::Response;
/// use cot::router::{Route, Router};
///
/// async fn home(request: Request) -> cot::Result<Response> {
///     unimplemented!()
/// }
///
/// let router = Router::with_urls([Route::with_handler_and_name("/", home, "home")]);
/// ```
#[derive(Clone, Debug)]
pub struct Router {
    app_name: Option<AppName>,
    urls: Vec<Route>,
    names: HashMap<RouteName, Arc<PathMatcher>>,
    route_tree: RouteTrie,
}

impl Router {
    /// Create an empty router.
    ///
    /// This router will not route any requests.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::router::Router;
    ///
    /// let router = Router::empty();
    /// ```
    #[must_use]
    pub fn empty() -> Self {
        Self::with_urls(&[])
    }

    /// Create a router with the given routes.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::Request;
    /// use cot::response::Response;
    /// use cot::router::{Route, Router};
    ///
    /// async fn home(request: Request) -> cot::Result<Response> {
    ///     unimplemented!()
    /// }
    ///
    /// let router = Router::with_urls([Route::with_handler_and_name("/", home, "home")]);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when a url string could not be parsed into a [`Route`]
    #[must_use]
    pub fn with_urls<T: Into<Vec<Route>>>(urls: T) -> Self {
        match Self::try_with_urls(urls) {
            Ok(router) => router,
            Err(err) => panic!("{err}"),
        }
    }

    /// Create a router with the given routes. This is a fallible version
    /// of [`Self::with_urls`]
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::Request;
    /// use cot::response::Response;
    /// use cot::router::{Route, Router};
    ///
    /// async fn home(request: Request) -> cot::Result<Response> {
    ///     unimplemented!()
    /// }
    ///
    /// let router = Router::try_with_urls([Route::with_handler_and_name("/", home, "home")]).unwrap();
    /// ```
    ///
    /// # Errors
    ///
    /// This method fails when the underlying trie fails to build.
    pub fn try_with_urls<T: Into<Vec<Route>>>(urls: T) -> Result<Self> {
        let urls = urls.into();
        let mut names = HashMap::new();

        for url in &urls {
            if let Some(name) = &url.name {
                names.insert(name.clone(), url.url.clone());
            }
        }
        let route_tree = RouteTrie::build(&urls)?;
        Ok(Self {
            app_name: None,
            urls,
            names,
            route_tree,
        })
    }

    pub(crate) fn set_app_name(&mut self, app_name: AppName) {
        self.app_name = Some(app_name);
    }

    async fn route(&self, mut request: Request, request_path: &str) -> Result<Response> {
        debug!("Routing request to {}", request_path);

        if let Some(result) = self.get_handler(request_path) {
            let mut path_params = PathParams::new();
            for (key, value) in result.params.iter().rev() {
                path_params.insert(key.clone(), value.clone());
            }
            request.extensions_mut().insert(path_params);
            if let Some(app_name) = result.app_name {
                request.extensions_mut().insert(app_name);
            }
            if let Some(name) = result.name {
                request.extensions_mut().insert(name);
            }
            result.handler.handle(request).await
        } else {
            debug!("Not found: {}", request_path);
            Err(Error::from(NotFound::router()))
        }
    }

    fn get_handler(&self, request_path: &str) -> Option<HandlerFound<'_>> {
        let m = self.route_tree.at(request_path)?;

        let (route_index, remaining_path) = match m.value {
            Entry::Handler(idx) => (*idx, String::new()),
            Entry::Router(idx) => {
                let remaining = match m.params.get(tree::NESTED_ROUTER_PARAM) {
                    Some(rest) => AbsolutePath::new(rest),
                    None => AbsolutePath::root(),
                };
                (*idx, remaining.into())
            }
        };

        let params: Vec<(String, String)> = m
            .params
            .iter()
            .filter(|(key, _)| *key != tree::NESTED_ROUTER_PARAM)
            .map(|(k, v)| (k.to_owned(), v.to_owned()))
            .collect();

        Self::route_to_handler(self, route_index, &remaining_path, &params)
    }

    fn route_to_handler<'a>(
        router: &'a Router,
        route_index: usize,
        remaining_path: &str,
        params: &[(String, String)],
    ) -> Option<HandlerFound<'a>> {
        let route = &router.urls[route_index];

        match &route.view {
            RouteInner::Handler(handler) => Some(HandlerFound {
                handler: &**handler,
                app_name: router.app_name.clone(),
                name: route.name.clone(),
                params: params.to_vec(),
            }),
            RouteInner::Router(nested_router) => {
                nested_router.get_handler(remaining_path).map(|mut found| {
                    found.app_name = found.app_name.or_else(|| router.app_name.clone());
                    found.params.extend(params.iter().cloned());
                    found
                })
            }
            #[cfg(feature = "openapi")]
            RouteInner::ApiHandler(handler) => {
                let handler: &(dyn BoxRequestHandler + Send + Sync) = &**handler;
                Some(HandlerFound {
                    handler,
                    app_name: router.app_name.clone(),
                    name: route.name.clone(),
                    params: params.to_vec(),
                })
            }
        }
    }

    pub(crate) fn has_route(&self, request_path: &str) -> bool {
        self.get_handler(request_path).is_some()
    }

    /// Handle a request.
    ///
    /// This method is called by the [`CotApp`](crate::App) to handle
    /// a request.
    ///
    /// # Errors
    ///
    /// This method re-throws any errors that occur in the request handler.
    pub async fn handle(&self, request: Request) -> Result<Response> {
        let path = request.uri().path().to_owned();
        self.route(request, &path).await
    }

    /// Generates a URL for a view using its name.
    ///
    /// Instead of using this method directly, consider using the
    /// [`reverse!`](crate::reverse) macro which provides much more ergonomic
    /// way to call this.
    ///
    /// The `app_name` parameter specifies the name of the app that the view
    /// should be found in. If it is `None`, the view is searched for across all
    /// registered apps.
    ///
    /// # Errors
    ///
    /// This method returns an error if the view name is not found.
    ///
    /// This method returns an error if the URL cannot be generated because of
    /// missing parameters.
    pub fn reverse(
        &self,
        app_name: Option<&str>,
        name: &str,
        params: &ReverseParamMap,
    ) -> Result<String> {
        Ok(self
            .reverse_option(app_name, name, params)?
            .ok_or_else(|| NoViewToReverse {
                app_name: app_name.map(ToOwned::to_owned),
                view_name: name.to_owned(),
            })?)
    }

    /// Generates a URL for a view using its name.
    ///
    /// The `app_name` parameter specifies the name of the app that the view
    /// should be found in. If it is `None`, the view is searched for across all
    /// registered apps.
    ///
    /// It returns [`None`] if the view name is not found.
    ///
    /// # Errors
    ///
    /// This method returns an error if the URL cannot be generated because of
    /// missing parameters.
    pub fn reverse_option(
        &self,
        app_name: Option<&str>,
        name: &str,
        params: &ReverseParamMap,
    ) -> Result<Option<String>> {
        if app_name.is_none()
            || self.app_name.is_none()
            || app_name == self.app_name.as_ref().map(|name| name.0.as_str())
        {
            self.reverse_option_impl(app_name, name, params)
        } else {
            Ok(None)
        }
    }

    fn reverse_option_impl(
        &self,
        app_name: Option<&str>,
        name: &str,
        params: &ReverseParamMap,
    ) -> Result<Option<String>> {
        let url = self
            .names
            .get(&RouteName(String::from(name)))
            .map(|matcher| matcher.reverse(params));
        if let Some(url) = url {
            return Ok(Some(url?));
        }

        for route in &self.urls {
            if let RouteInner::Router(router) = &route.view
                && let Some(url) = router.reverse_option(app_name, name, params)?
            {
                let prefix = AbsolutePath::new(route.url.reverse(params)?);
                let suffix = AbsolutePath::new(url);
                return Ok(Some(prefix.join(&suffix).into()));
            }
        }
        Ok(None)
    }

    /// Get the routes in this router.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::Request;
    /// use cot::response::Response;
    /// use cot::router::{Route, Router};
    ///
    /// async fn home(request: Request) -> cot::Result<Response> {
    ///     unimplemented!()
    /// }
    ///
    /// let router = Router::with_urls([Route::with_handler_and_name("/", home, "home")]);
    /// assert_eq!(router.routes().len(), 1);
    /// ```
    #[must_use]
    pub fn routes(&self) -> &[Route] {
        &self.urls
    }

    /// Check if this router is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::Request;
    /// use cot::response::Response;
    /// use cot::router::{Route, Router};
    ///
    /// async fn home(request: Request) -> cot::Result<Response> {
    ///     unimplemented!()
    /// }
    ///
    /// let router = Router::empty();
    /// assert!(router.is_empty());
    ///
    /// let router = Router::with_urls([Route::with_handler_and_name("/", home, "home")]);
    /// assert!(!router.is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.urls.is_empty()
    }

    /// Returns the OpenAPI paths for the router.
    ///
    /// This might be useful if you want to manually serve the generated OpenAPI
    /// specs.
    ///
    /// # Panics
    ///
    /// Panics if invalid schemas are generated. This should not happen in
    /// normal operation, but if it does, it indicates a bug in the
    /// [`schemars`](https://docs.rs/schemars/latest/schemars/) library
    /// or in the way the OpenAPI specs are generated.
    #[cfg(feature = "openapi")]
    #[must_use]
    pub fn as_api(&self) -> aide::openapi::OpenApi {
        let mut paths = aide::openapi::Paths::default();
        let mut schema_generator =
            schemars::SchemaGenerator::new(schemars::generate::SchemaSettings::openapi3());

        self.as_openapi_impl(
            &AbsolutePath::root(),
            &[],
            &mut paths,
            &mut schema_generator,
        );

        let component_schemas = schema_generator
            .take_definitions(true)
            .into_iter()
            .map(|(name, json_schema)| {
                (
                    name,
                    aide::openapi::SchemaObject {
                        json_schema: schemars::Schema::try_from(json_schema).expect(
                            "SchemaGenerator::take_definitions should return valid schemas",
                        ),
                        example: None,
                        external_docs: None,
                    },
                )
            })
            .collect();
        aide::openapi::OpenApi {
            paths: Some(paths),
            components: Some(aide::openapi::Components {
                schemas: component_schemas,
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[cfg(feature = "openapi")]
    fn as_openapi_impl(
        &self,
        url: &AbsolutePath,
        param_names: &[&str],
        paths: &mut aide::openapi::Paths,
        schema_generator: &mut schemars::SchemaGenerator,
    ) {
        for route in &self.urls {
            Self::route_as_openapi(route, param_names, paths, schema_generator, url);
        }
    }

    #[cfg(feature = "openapi")]
    fn route_as_openapi(
        route: &Route,
        param_names: &[&str],
        paths: &mut aide::openapi::Paths,
        schema_generator: &mut schemars::SchemaGenerator,
        url: &AbsolutePath,
    ) {
        match &route.view {
            RouteInner::Router(router) => {
                let mut params = Vec::from(param_names);
                params.extend(route.url.param_names());

                let url = url.join(&AbsolutePath::new(route.url()));

                router.as_openapi_impl(&url, &params, paths, schema_generator);
            }
            RouteInner::ApiHandler(handler) => {
                let mut params = Vec::from(param_names);
                params.extend(route.url.param_names());

                let url = url.join(&AbsolutePath::new(route.url()));

                let mut route_context = crate::openapi::RouteContext::new();
                route_context.param_names = &params;

                paths.paths.insert(
                    url.into(),
                    aide::openapi::ReferenceOr::Item(
                        handler.as_api_route(&route_context, schema_generator),
                    ),
                );
            }
            RouteInner::Handler(_) => {}
        }
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Debug, thiserror::Error)]
#[error("failed to reverse route `{view_name}` due to view not existing")]
struct NoViewToReverse {
    app_name: Option<String>,
    view_name: String,
}
impl_into_cot_error!(NoViewToReverse);

const ERROR_PREFIX: &str = "route conflict error:";
#[derive(Debug, thiserror::Error)]
enum RouteConflictError {
    #[error(
        "{ERROR_PREFIX} duplicate route: `{new}` conflicts with an already registered handler route `{existing}` \
         (both fully match the same path)"
    )]
    DuplicateHandler { existing: String, new: String },

    #[error(
        "{ERROR_PREFIX} duplicate nested router: `{new}` conflicts with an already registered \
         nested router mounted at `{existing}`"
    )]
    DuplicateRouter { existing: String, new: String },

    #[error(
        "{ERROR_PREFIX} conflicting route parameters: `{existing}` uses `{{{existing_name}}}` but `{new}` uses \
         `{{{new_name}}}` at the same position in the path; both routes must bind the same \
         parameter name there, since only one value can be captured at that position"
    )]
    ConflictingParamName {
        existing: String,
        existing_name: String,
        new: String,
        new_name: String,
    },

    #[error(
        "{ERROR_PREFIX} conflicting wildcard parameters: `{existing}` uses `{{*{existing_name}}}` but `{new}` \
         uses `{{*{new_name}}}` at the same position in the path"
    )]
    ConflictingWildcardName {
        existing: String,
        existing_name: String,
        new: String,
        new_name: String,
    },

    #[error(
        "{ERROR_PREFIX} duplicate wildcard route: `{new}` conflicts with an already-registered \
         wildcard route `{existing}`"
    )]
    DuplicateWildcard { existing: String, new: String },
    #[error("{ERROR_PREFIX} error while inserting route")]
    RouteInsert(#[from] matchit::InsertError),
}
impl_into_cot_error!(RouteConflictError);

#[derive(Debug)]
struct HandlerFound<'a> {
    #[debug("handler(...)")]
    handler: &'a (dyn BoxRequestHandler + Send + Sync),
    app_name: Option<AppName>,
    name: Option<RouteName>,
    params: Vec<(String, String)>,
}

/// A service that routes requests to their respective views.
///
/// This is mostly an internal service used by the [`CotApp`](crate::App) to
/// route requests to their respective views with an interface that is
/// compatible with the [`tower::Service`] trait.
#[derive(Debug, Clone)]
pub struct RouterService {
    router: Arc<Router>,
}

impl RouterService {
    /// Create a new router service.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    ///
    /// use cot::request::Request;
    /// use cot::response::Response;
    /// use cot::router::{Route, Router, RouterService};
    ///
    /// async fn home(request: Request) -> cot::Result<Response> {
    ///     unimplemented!()
    /// }
    ///
    /// let router = Router::with_urls([Route::with_handler_and_name("/", home, "home")]);
    /// let service = RouterService::new(Arc::new(router));
    /// ```
    #[must_use]
    pub fn new(router: Arc<Router>) -> Self {
        Self { router }
    }
}

impl tower::Service<Request> for RouterService {
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response>> + Send>>;
    type Response = Response;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<std::result::Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let router = self.router.clone();
        Box::pin(async move { router.handle(req).await })
    }
}

// used in the reverse! macro; not part of public API
#[doc(hidden)]
#[must_use]
pub fn split_view_name(view_name: &str) -> (Option<&str>, &str) {
    let colon_pos = view_name.find(':');
    if let Some(colon_pos) = colon_pos {
        let app_name = &view_name[..colon_pos];
        let view_name = &view_name[colon_pos + 1..];
        (Some(app_name), view_name)
    } else {
        (None, view_name)
    }
}

/// A route that can be used to route requests to their respective views.
///
/// Non-empty route paths may omit the leading slash. Cot normalizes them by
/// prepending `/`, so `"home"` and `"/home"` define the same route.
///
/// # Examples
///
/// ```
/// use cot::request::Request;
/// use cot::response::Response;
/// use cot::router::{Route, Router};
///
/// async fn home(request: Request) -> cot::Result<Response> {
///     unimplemented!()
/// }
///
/// let router = Router::with_urls([Route::with_handler_and_name("/", home, "home")]);
/// ```
#[derive(Debug, Clone)]
pub struct Route {
    url: Arc<PathMatcher>,
    view: RouteInner,
    name: Option<RouteName>,
}

impl Route {
    /// Create a new route with the given handler.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::Request;
    /// use cot::response::Response;
    /// use cot::router::{Route, Router};
    ///
    /// async fn home(request: Request) -> cot::Result<Response> {
    ///     // ...
    /// #     unimplemented!()
    /// }
    ///
    /// let route = Route::with_handler("home", home);
    /// assert_eq!(route.url(), "/home");
    /// ```
    #[must_use]
    pub fn with_handler<HandlerParams, H>(url: &str, handler: H) -> Self
    where
        HandlerParams: 'static,
        H: RequestHandler<HandlerParams> + Send + Sync + 'static,
    {
        Self {
            url: Arc::new(PathMatcher::new(url)),
            view: RouteInner::Handler(Arc::new(into_box_request_handler(handler))),
            name: None,
        }
    }

    /// Create a new route with the given handler for inclusion in the OpenAPI
    /// specifications.
    ///
    /// See [`crate::openapi`] module documentation for more details on how to
    /// generate OpenAPI specifications automatically.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::Request;
    /// use cot::response::Response;
    /// use cot::router::method::openapi::api_get;
    /// use cot::router::{Route, Router};
    ///
    /// async fn home(request: Request) -> cot::Result<Response> {
    ///     // ...
    /// #     unimplemented!()
    /// }
    ///
    /// let route = Route::with_api_handler("/", api_get(home));
    /// ```
    #[must_use]
    #[cfg(feature = "openapi")]
    pub fn with_api_handler<HandlerParams, H>(url: &str, handler: H) -> Self
    where
        HandlerParams: 'static,
        H: RequestHandler<HandlerParams> + crate::openapi::AsApiRoute + Send + Sync + 'static,
    {
        Self {
            url: Arc::new(PathMatcher::new(url)),
            view: RouteInner::ApiHandler(Arc::new(
                crate::openapi::into_box_api_endpoint_request_handler(handler),
            )),
            name: None,
        }
    }

    /// Create a new route with the given handler and name.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::Request;
    /// use cot::response::Response;
    /// use cot::router::method::openapi::api_get;
    /// use cot::router::{Route, Router};
    ///
    /// async fn home(request: Request) -> cot::Result<Response> {
    ///     // ...
    /// #     unimplemented!()
    /// }
    ///
    /// let route = Route::with_handler_and_name("/", api_get(home), "home");
    /// ```
    #[must_use]
    pub fn with_handler_and_name<N, HandlerParams, H>(url: &str, handler: H, name: N) -> Self
    where
        N: Into<String>,
        HandlerParams: 'static,
        H: RequestHandler<HandlerParams> + Send + Sync + 'static,
    {
        Self {
            url: Arc::new(PathMatcher::new(url)),
            view: RouteInner::Handler(Arc::new(into_box_request_handler(handler))),
            name: Some(RouteName(name.into())),
        }
    }

    /// Create a new route with the given handler and name for inclusion in the
    /// OpenAPI specs.
    ///
    /// See [`crate::openapi`] module documentation for more details on how to
    /// generate OpenAPI specs automatically.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::Request;
    /// use cot::response::Response;
    /// use cot::router::method::openapi::api_post;
    /// use cot::router::{Route, Router};
    ///
    /// async fn home(request: Request) -> cot::Result<Response> {
    ///     // ...
    /// #     unimplemented!()
    /// }
    ///
    /// let route = Route::with_api_handler_and_name("/", api_post(home), "home");
    /// ```
    #[must_use]
    #[cfg(feature = "openapi")]
    pub fn with_api_handler_and_name<N, HandlerParams, H>(url: &str, handler: H, name: N) -> Self
    where
        N: Into<String>,
        HandlerParams: 'static,
        H: RequestHandler<HandlerParams> + crate::openapi::AsApiRoute + Send + Sync + 'static,
    {
        Self {
            url: Arc::new(PathMatcher::new(url)),
            view: RouteInner::ApiHandler(Arc::new(
                crate::openapi::into_box_api_endpoint_request_handler(handler),
            )),
            name: Some(RouteName(name.into())),
        }
    }

    /// Create a new route with the given router.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::Request;
    /// use cot::response::Response;
    /// use cot::router::{Route, Router};
    ///
    /// async fn home(request: Request) -> cot::Result<Response> {
    ///     unimplemented!()
    /// }
    ///
    /// let router = Router::with_urls([Route::with_handler_and_name("/", home, "home")]);
    /// let route = Route::with_router("/", router);
    /// ```
    #[must_use]
    pub fn with_router(url: &str, router: Router) -> Self {
        Self {
            url: Arc::new(PathMatcher::new(url)),
            view: RouteInner::Router(Arc::new(router)),
            name: None,
        }
    }

    /// Get the URL for this route.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::Request;
    /// use cot::response::Response;
    /// use cot::router::{Route, Router};
    ///
    /// async fn home(request: Request) -> cot::Result<Response> {
    ///     unimplemented!()
    /// }
    ///
    /// let route = Route::with_handler("/test", home);
    /// assert_eq!(route.url(), "/test");
    /// ```
    #[must_use]
    pub fn url(&self) -> String {
        self.url.to_string()
    }

    /// Get the name of this route, if it was created with the
    /// [`Self::with_handler_and_name`] function.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::Request;
    /// use cot::response::Response;
    /// use cot::router::{Route, Router};
    ///
    /// async fn home(request: Request) -> cot::Result<Response> {
    ///     unimplemented!()
    /// }
    ///
    /// let route = Route::with_handler_and_name("/", home, "home");
    /// assert_eq!(route.name(), Some("home"));
    /// ```
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_ref().map(|name| name.0.as_str())
    }

    #[must_use]
    pub(crate) fn kind(&self) -> RouteKind {
        match &self.view {
            RouteInner::Handler(_) => RouteKind::Handler,
            RouteInner::Router(_) => RouteKind::Router,
            #[cfg(feature = "openapi")]
            RouteInner::ApiHandler(_) => RouteKind::Handler,
        }
    }

    #[must_use]
    pub(crate) fn router(&self) -> Option<Arc<Router>> {
        match &self.view {
            RouteInner::Router(router) => Some(router.clone()),
            RouteInner::Handler(_) => None,
            #[cfg(feature = "openapi")]
            RouteInner::ApiHandler(_) => None,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum RouteKind {
    Handler,
    Router,
}

#[derive(Clone)]
enum RouteInner {
    Handler(Arc<dyn BoxRequestHandler + Send + Sync>),
    Router(Arc<Router>),
    #[cfg(feature = "openapi")]
    ApiHandler(Arc<dyn crate::openapi::BoxApiEndpointRequestHandler + Send + Sync>),
}

/// Get a URL for a view by its registered name and given params.
///
/// If the view name has two parts separated by a colon, the first part is
/// considered the app name. If the app name is not provided, the app name of
/// the request is used. This means that if you don't specify the `app_name`,
/// this macro will only return URLs for views in the same app as the current
/// request handler.
///
/// # Return value
///
/// Returns a [`cot::Result<String>`] that contains the URL for the view. You
/// will typically want to append `?` to the macro call to get the URL.
///
/// # Examples
///
/// ```
/// use cot::html::Html;
/// use cot::project::RegisterAppsContext;
/// use cot::request::Request;
/// use cot::router::{Route, Router};
/// use cot::{App, AppBuilder, Project, StatusCode, reverse};
///
/// async fn home(request: Request) -> cot::Result<Html> {
///     // any of below two lines returns the same:
///     let url = reverse!(request, "home")?;
///     let url = reverse!(request, "my_custom_app:home")?;
///
///     Ok(Html::new(format!(
///         "Hello! The URL for this view is: {}",
///         url
///     )))
/// }
///
/// let router = Router::with_urls([Route::with_handler_and_name("/", home, "home")]);
///
/// struct MyApp;
///
/// impl App for MyApp {
///     fn name(&self) -> &'static str {
///         "my_custom_app"
///     }
///
///     fn router(&self) -> Router {
///         Router::with_urls([Route::with_handler_and_name("/", home, "home")])
///     }
/// }
///
/// struct MyProject;
///
/// impl Project for MyProject {
///     fn register_apps(&self, apps: &mut AppBuilder, context: &RegisterAppsContext) {
///         apps.register_with_views(MyApp, "");
///     }
/// }
/// ```
#[macro_export]
macro_rules! reverse {
    ($request:expr, $view_name:literal $(, $($key:ident = $value:expr),*)?) => {{
        #[allow(
            clippy::allow_attributes,
            unused_imports,
            reason = "allow using either `Request` or `Urls` objects"
        )]
        use $crate::request::RequestExt;
        let (app_name, view_name) = $crate::router::split_view_name($view_name);
        let app_name = app_name.or_else(|| $request.app_name());
        $request
            .router()
            .reverse(app_name, view_name, &$crate::reverse_param_map!($( $($key = $value),* )?))
    }};
}

/// A helper structure to allow reversing URLs from a request handler.
///
/// This is mainly useful as an extractor to allow reversing URLs without
/// access to a full [`Request`] object.
///
/// # Examples
///
/// ```
/// use cot::html::Html;
/// use cot::router::{Route, Router, Urls};
/// use cot::test::TestRequestBuilder;
/// use cot::{RequestHandler, reverse};
///
/// async fn my_handler(urls: Urls) -> cot::Result<Html> {
///     let url = reverse!(urls, "home")?;
///     Ok(Html::new(format!("{url}")))
/// }
///
/// # #[tokio::main]
/// # async fn main() -> cot::Result<()> {
/// let router = Router::with_urls([Route::with_handler_and_name("/", my_handler, "home")]);
/// let request = TestRequestBuilder::get("/").router(router).build();
///
/// assert_eq!(
///     my_handler
///         .handle(request)
///         .await?
///         .into_body()
///         .into_bytes()
///         .await?,
///     "/"
/// );
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct Urls {
    app_name: Option<String>,
    router: Arc<Router>,
}

impl Urls {
    /// Create a new `Urls` object from a [`Request`] object.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::html::Html;
    /// use cot::request::Request;
    /// use cot::response::{Response, ResponseExt};
    /// use cot::router::Urls;
    /// use cot::{Body, StatusCode, reverse};
    ///
    /// async fn my_handler(request: Request) -> cot::Result<Html> {
    ///     let urls = Urls::from_request(&request);
    ///     let url = reverse!(urls, "home")?;
    ///     Ok(Html::new(format!(
    ///         "Hello! The URL for this view is: {}",
    ///         url
    ///     )))
    /// }
    /// ```
    pub fn from_request(request: &Request) -> Self {
        Self {
            app_name: request.app_name().map(ToOwned::to_owned),
            router: Arc::clone(request.router()),
        }
    }

    pub(crate) fn from_parts(request_head: &RequestHead) -> Self {
        Self {
            app_name: request_head.app_name().map(ToOwned::to_owned),
            router: Arc::clone(request_head.router()),
        }
    }

    /// Get the app name the current route belongs to, or [`None`] if the
    /// request is not routed.
    ///
    /// This is mainly useful for providing context to reverse redirects, where
    /// you want to redirect to a route in the same app.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::Response;
    /// use cot::router::Urls;
    ///
    /// async fn my_handler(urls: Urls) -> cot::Result<Response> {
    ///     let app_name = urls.app_name();
    ///     // ... do something with the app name
    ///     # unimplemented!()
    /// }
    /// ```
    #[must_use]
    pub fn app_name(&self) -> Option<&str> {
        self.app_name.as_deref()
    }

    /// Get the router.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::Response;
    /// use cot::router::Urls;
    ///
    /// async fn my_handler(urls: Urls) -> cot::Result<Response> {
    ///     let router = urls.router();
    ///     // ... do something with the router
    ///     # unimplemented!()
    /// }
    /// ```
    #[must_use]
    pub fn router(&self) -> &Router {
        &self.router
    }
}

impl Debug for RouteInner {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            RouteInner::Handler(_) => f.debug_tuple("Handler").field(&"handler(...)").finish(),
            RouteInner::Router(router) => f.debug_tuple("Router").field(router).finish(),
            #[cfg(feature = "openapi")]
            RouteInner::ApiHandler(_) => {
                f.debug_tuple("ApiHandler").field(&"handler(...)").finish()
            }
        }
    }
}

impl From<&ProjectContext> for Urls {
    fn from(ctx: &ProjectContext) -> Self {
        Self {
            app_name: None,
            router: Arc::clone(ctx.router()),
        }
    }
}

impl From<&mut ProjectContext> for Urls {
    fn from(ctx: &mut ProjectContext) -> Self {
        Self::from(&*ctx)
    }
}

/// Get a URL for a view by its registered name and given params and return a
/// response with a redirect.
///
/// This macro is a shorthand for creating a response with a redirect to a URL
/// generated by the [`reverse!`] macro.
///
/// # Return value
///
/// Returns a [`cot::Result<Response>`] that contains the URL for
/// the view. You will typically want to append `?` to the macro call to get the
/// [`Response`] object.
///
/// # Examples
///
/// ```
/// use cot::request::Request;
/// use cot::response::Response;
/// use cot::reverse_redirect;
/// use cot::router::{Route, Router};
///
/// async fn infinite_loop(request: Request) -> cot::Result<Response> {
///     Ok(reverse_redirect!(request, "home")?)
/// }
///
/// let router = Router::with_urls([Route::with_handler_and_name("/", infinite_loop, "home")]);
/// ```
#[macro_export]
macro_rules! reverse_redirect {
    ($request:expr, $view_name:literal $(, $($key:ident = $value:expr),*)?) => {
        $crate::reverse!(
            $request,
            $view_name,
            $( $($key = $value),* )?
        ).map(|url|
            $crate::response::IntoResponse::into_response($crate::response::Redirect::new(url))
                .expect("Failed to build response")
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StatusCode;
    use crate::html::Html;
    use crate::request::Request;
    use crate::response::{IntoResponse, Response};
    use crate::test::TestRequestBuilder;

    struct MockHandler;

    impl RequestHandler for MockHandler {
        async fn handle(&self, _request: Request) -> Result<Response> {
            Html::new("OK").into_response()
        }
    }

    #[cfg(feature = "openapi")]
    impl crate::openapi::AsApiRoute for MockHandler {
        fn as_api_route(
            &self,
            _route_context: &cot::openapi::RouteContext<'_>,
            _schema_generator: &mut schemars::SchemaGenerator,
        ) -> aide::openapi::PathItem {
            aide::openapi::PathItem::default()
        }
    }

    fn assert_params(mut actual: Vec<(String, String)>, expected: &[(&str, &str)]) {
        let mut expected = expected
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect::<Vec<_>>();
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected);
    }

    #[test]
    fn route_inner_debug() {
        let route = Route::with_handler("/test", MockHandler);
        assert!(format!("{route:?}").contains("Handler(\"handler(...)\")"));

        let route = Route::with_router("/test", Router::empty());
        assert!(format!("{route:?}").contains("Router(Router {"));

        #[cfg(feature = "openapi")]
        {
            let route = Route::with_api_handler("/test", MockHandler);
            assert!(format!("{route:?}").contains("ApiHandler(\"handler(...)\")"));
        }
    }

    #[test]
    fn route_kind() {
        let handler_route = Route::with_handler("/test", MockHandler);
        assert_eq!(handler_route.kind(), RouteKind::Handler);

        let router_route = Route::with_router("/test", Router::empty());
        assert_eq!(router_route.kind(), RouteKind::Router);

        #[cfg(feature = "openapi")]
        {
            let api_route = Route::with_api_handler("/test", MockHandler);
            assert_eq!(api_route.kind(), RouteKind::Handler);
        }
    }

    #[test]
    fn route_router() {
        let router = Router::empty();
        let route = Route::with_router("/test", router.clone());
        assert!(route.router().is_some());

        let route = Route::with_handler("/test", MockHandler);
        assert!(route.router().is_none());

        #[cfg(feature = "openapi")]
        {
            let route = Route::with_api_handler("/test", MockHandler);
            assert!(route.router().is_none());
        }
    }

    #[test]
    fn router_with_urls() {
        let route = Route::with_handler("/test", MockHandler);
        let router = Router::with_urls(vec![route.clone()]);
        assert_eq!(router.routes().len(), 1);
    }

    #[cot::test]
    async fn router_route() {
        let route = Route::with_handler("/test", MockHandler);
        let router = Router::with_urls(vec![route.clone()]);
        let response = router.route(test_request(), "/test").await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[cot::test]
    async fn router_route_without_leading_slash() {
        let route = Route::with_handler_and_name("test", MockHandler, "test");
        assert_eq!(route.url(), "/test");

        let router = Router::with_urls(vec![route]);
        let response = router.route(test_request(), "/test").await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let url = router
            .reverse(None, "test", &ReverseParamMap::new())
            .unwrap();
        assert_eq!(url, "/test");
    }

    #[cot::test]
    async fn router_handle() {
        let route = Route::with_handler("/test", MockHandler);
        let router = Router::with_urls(vec![route.clone()]);
        let response = router.handle(test_request()).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[cot::test]
    async fn sub_router_handle() {
        let route_1 = Route::with_handler("/test", MockHandler);
        let sub_router_1 = Router::with_urls(vec![route_1.clone()]);
        let route_2 = Route::with_handler("/test", MockHandler);
        let sub_router_2 = Router::with_urls(vec![route_2.clone()]);

        let router = Router::with_urls(vec![
            Route::with_router("/", sub_router_1),
            Route::with_router("/sub", sub_router_2),
        ]);
        let response = router
            .handle(TestRequestBuilder::get("/sub/test").build())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn router_reverse() {
        let route = Route::with_handler_and_name("/test", MockHandler, "test");
        let router = Router::with_urls(vec![route.clone()]);
        let params = ReverseParamMap::new();
        let url = router.reverse(None, "test", &params).unwrap();
        assert_eq!(url, "/test");
    }

    #[test]
    fn router_reverse_with_param() {
        let route = Route::with_handler_and_name("/test/{id}", MockHandler, "test");
        let router = Router::with_urls(vec![route.clone()]);
        let mut params = ReverseParamMap::new();
        params.insert("id", "123");
        let url = router.reverse(None, "test", &params).unwrap();
        assert_eq!(url, "/test/123");
    }

    #[test]
    fn router_no_param_route_matches_exact_path() {
        let router = Router::with_urls(vec![Route::with_handler_and_name(
            "/users",
            MockHandler,
            "users",
        )]);

        let found = router.get_handler("/users").unwrap();

        assert_eq!(found.name, Some(RouteName("users".to_string())));
        assert!(found.params.is_empty());
    }

    #[test]
    fn router_no_param_route_rejects_different_path() {
        let router = Router::with_urls(vec![Route::with_handler_and_name(
            "/users",
            MockHandler,
            "users",
        )]);

        assert!(router.get_handler("/test").is_none());
    }

    #[test]
    fn router_param_route_captures_single_segment() {
        let router = Router::with_urls(vec![Route::with_handler_and_name(
            "/users/{id}",
            MockHandler,
            "user_detail",
        )]);

        let found = router.get_handler("/users/123").unwrap();

        assert_eq!(found.name, Some(RouteName("user_detail".to_string())));
        assert_params(found.params, &[("id", "123")]);
    }

    #[test]
    fn router_param_route_rejects_empty_segment() {
        let router = Router::with_urls(vec![Route::with_handler_and_name(
            "/users/{id}",
            MockHandler,
            "user_detail",
        )]);

        assert!(router.get_handler("/users/").is_none());
    }

    #[test]
    fn router_param_route_rejects_extra_path_for_handler() {
        let router = Router::with_urls(vec![Route::with_handler_and_name(
            "/users/{id}",
            MockHandler,
            "user_detail",
        )]);

        assert!(router.get_handler("/users/123/abc").is_none());
    }

    #[test]
    fn router_multiple_param_route_captures_all_params() {
        let router = Router::with_urls(vec![Route::with_handler_and_name(
            "/users/{id}/posts/{post_id}",
            MockHandler,
            "post_detail",
        )]);

        let found = router.get_handler("/users/123/posts/456").unwrap();

        assert_eq!(found.name, Some(RouteName("post_detail".to_string())));
        assert_params(found.params, &[("id", "123"), ("post_id", "456")]);
    }

    #[test]
    fn router_routes_with_common_static_prefixes_match_independently() {
        let router = Router::with_urls(vec![
            Route::with_handler_and_name("/car", MockHandler, "car"),
            Route::with_handler_and_name("/cart", MockHandler, "cart"),
            Route::with_handler_and_name("/catalog", MockHandler, "catalog"),
        ]);

        assert_eq!(
            router.get_handler("/car").unwrap().name,
            Some(RouteName("car".to_string()))
        );
        assert_eq!(
            router.get_handler("/cart").unwrap().name,
            Some(RouteName("cart".to_string()))
        );
        assert_eq!(
            router.get_handler("/catalog").unwrap().name,
            Some(RouteName("catalog".to_string()))
        );
        assert!(router.get_handler("/cartographer").is_none());
    }

    #[test]
    fn router_static_route_takes_priority_over_dynamic_route() {
        let router = Router::with_urls(vec![
            Route::with_handler_and_name("/users/{id}", MockHandler, "dynamic"),
            Route::with_handler_and_name("/users/new", MockHandler, "static"),
        ]);

        let found = router.get_handler("/users/new").unwrap();

        assert_eq!(found.name, Some(RouteName("static".to_string())));
    }

    #[test]
    fn router_wildcard_root() {
        let router = Router::with_urls(vec![Route::with_handler_and_name(
            "/{*path}",
            MockHandler,
            "users",
        )]);

        let found = router.get_handler("/foo/bar").unwrap();

        assert_eq!(found.name, Some(RouteName("users".to_string())));
        assert_eq!(
            found.params,
            vec![("path".to_string(), "foo/bar".to_string())]
        );
    }
    #[test]
    fn router_wildcard_single_segment() {
        let router = Router::with_urls(vec![Route::with_handler_and_name(
            "/users/rand/{*path}",
            MockHandler,
            "users",
        )]);

        let found = router.get_handler("/users/rand/foo").unwrap();

        assert_eq!(found.name, Some(RouteName("users".to_string())));
        assert_eq!(found.params, vec![("path".to_string(), "foo".to_string())]);
    }
    #[test]
    fn router_wildcard_multi_segment() {
        let router = Router::with_urls(vec![Route::with_handler_and_name(
            "/users/rand/{*path}",
            MockHandler,
            "users",
        )]);

        let found = router.get_handler("/users/rand/foo/bar").unwrap();

        assert_eq!(found.name, Some(RouteName("users".to_string())));
        assert_eq!(
            found.params,
            vec![("path".to_string(), "foo/bar".to_string())]
        );
    }

    #[test]
    fn router_wildcard_empty_not_allowed() {
        let router = Router::with_urls(vec![Route::with_handler_and_name(
            "/users/rand/{*path}",
            MockHandler,
            "users",
        )]);

        assert!(router.get_handler("/users/rand").is_none());
    }

    #[test]
    fn router_wildcard_route_is_lower_priority_than_static_route() {
        let router = Router::with_urls(vec![
            Route::with_handler_and_name("/static/{*path}", MockHandler, "wildcard"),
            Route::with_handler_and_name("/static/index.html", MockHandler, "static"),
        ]);

        let found = router.get_handler("/static/index.html").unwrap();

        assert_eq!(found.name, Some(RouteName("static".to_string())));
    }

    #[test]
    fn router_nested_router_consumes_remaining_path() {
        let sub_router = Router::with_urls(vec![Route::with_handler_and_name(
            "/posts/{post_id}",
            MockHandler,
            "post_detail",
        )]);
        let router = Router::with_urls(vec![Route::with_router("/users/{id}", sub_router)]);

        let found = router.get_handler("/users/123/posts/456").unwrap();

        assert_eq!(found.name, Some(RouteName("post_detail".to_string())));
        assert_params(found.params, &[("id", "123"), ("post_id", "456")]);
    }

    #[test]
    fn router_handler_takes_priority_over_nested_router_at_same_path() {
        let sub_router = Router::with_urls(vec![Route::with_handler_and_name(
            "/",
            MockHandler,
            "nested",
        )]);
        let router = Router::with_urls(vec![
            Route::with_router("/users", sub_router),
            Route::with_handler_and_name("/users", MockHandler, "handler"),
        ]);

        let found = router.get_handler("/users").unwrap();

        assert_eq!(found.name, Some(RouteName("handler".to_string())));
    }

    #[test]
    #[should_panic(
        expected = "route conflict error: duplicate route: `/users` conflicts with an already registered handler route `/users` (both fully match the same path)"
    )]
    fn router_duplicate_handler_routes_panic() {
        let _ = Router::with_urls(vec![
            Route::with_handler("/users", MockHandler),
            Route::with_handler("/users", MockHandler),
        ]);
    }

    #[test]
    #[should_panic(
        expected = "route conflict error: duplicate nested router: `/users` conflicts with an already registered nested router mounted at `/users`"
    )]
    fn router_duplicate_nested_router_routes_panic() {
        let _ = Router::with_urls(vec![
            Route::with_router("/users", Router::empty()),
            Route::with_router("/users", Router::empty()),
        ]);
    }

    #[test]
    #[should_panic(
        expected = "route conflict error: conflicting route parameters: `/foo/{bar}` uses `{bar}` but `/foo/{baz}` uses `{baz}` at the same position in the path; both routes must bind the same parameter name there, since only one value can be captured at that position"
    )]
    fn router_conflicting_param_names_panic() {
        let _ = Router::with_urls(vec![
            Route::with_handler("/foo/{bar}", MockHandler),
            Route::with_handler("/foo/{baz}", MockHandler),
        ]);
    }

    #[test]
    fn router_same_path_with_trailing_lash_diff() {
        // this should not fail
        let _ = Router::with_urls(vec![
            Route::with_handler("/foo/{bar}/", MockHandler),
            Route::with_handler("/foo/{baz}", MockHandler),
        ]);
    }

    #[test]
    #[should_panic(
        expected = "route conflict error: duplicate route: `/static/{*path}` conflicts with an already registered handler route `/static/{*path}` (both fully match the same path)"
    )]
    fn router_duplicate_wildcard_routes_panic() {
        let _ = Router::with_urls(vec![
            Route::with_handler("/static/{*path}", MockHandler),
            Route::with_handler("/static/{*path}", MockHandler),
        ]);
    }

    #[test]
    #[should_panic(
        expected = "route conflict error: conflicting wildcard parameters: `/static/{*path}` uses `{*path}` but `/static/{*file_path}` uses `{*file_path}` at the same position in the path"
    )]
    fn router_conflicting_wildcard_names_panic() {
        let _ = Router::with_urls(vec![
            Route::with_handler("/static/{*path}", MockHandler),
            Route::with_handler("/static/{*file_path}", MockHandler),
        ]);
    }

    #[test]
    #[should_panic(
        expected = "route conflict error: duplicate route: `/static/{*file_path}` conflicts with an already registered handler route `/static/{path}` (both fully match the same path)"
    )]
    fn router_wildcard_and_param_at_same_segment_conflict() {
        let _ = Router::with_urls(vec![
            Route::with_handler("/static/{path}", MockHandler),
            Route::with_handler("/static/{*file_path}", MockHandler),
        ]);
    }

    #[test]
    fn router_empty_returns_no_handler() {
        let router = Router::empty();
        assert!(router.get_handler("/").is_none());
    }

    #[test]
    fn router_root_mounted_nested_router() {
        let sub_router = Router::with_urls(vec![Route::with_handler_and_name(
            "/inner",
            MockHandler,
            "inner",
        )]);
        let router = Router::with_urls(vec![Route::with_router("/", sub_router)]);

        let found = router.get_handler("/inner").unwrap();
        assert_eq!(found.name, Some(RouteName("inner".to_string())));
    }

    #[test]
    fn router_nested_router_trailing_slash_prefix() {
        let sub_router = Router::with_urls(vec![Route::with_handler_and_name(
            "/inner",
            MockHandler,
            "inner",
        )]);
        let router = Router::with_urls(vec![Route::with_router("/api/", sub_router)]);

        let found = router.get_handler("/api/inner").unwrap();
        assert_eq!(found.name, Some(RouteName("inner".to_string())));
    }

    #[test]
    fn router_reverse_option_wrong_app_name_returns_none() {
        let route = Route::with_handler_and_name("/test", MockHandler, "test");
        let mut router = Router::with_urls(vec![route]);
        router.set_app_name(AppName("app_1".to_string()));

        let result = router
            .reverse_option(Some("app_2"), "test", &ReverseParamMap::new())
            .unwrap();

        assert!(result.is_none());
    }

    #[test]
    fn router_reverse_missing_view_returns_error() {
        let router = Router::empty();
        let result = router.reverse(None, "missing", &ReverseParamMap::new());
        assert!(result.is_err());
    }

    #[test]
    fn router_root_mount_matches_root_path() {
        let sub_router = Router::with_urls(vec![Route::with_handler_and_name(
            "/",
            MockHandler,
            "index",
        )]);
        let router = Router::with_urls(vec![Route::with_router("", sub_router)]);

        let found = router.get_handler("/").unwrap();

        assert_eq!(found.name, Some(RouteName("index".to_string())));
    }

    #[test]
    fn router_exact_mount_match_routes_to_nested_root_not_empty_path() {
        let sub_router = Router::with_urls(vec![Route::with_handler_and_name(
            "/",
            MockHandler,
            "sub_index",
        )]);
        let router = Router::with_urls(vec![Route::with_router("/api", sub_router)]);

        let found = router.get_handler("/api").unwrap();

        assert_eq!(found.name, Some(RouteName("sub_index".to_string())));
    }

    #[test]
    fn router_reverse_root_mount_no_double_slash() {
        let route = Route::with_handler_and_name("/", MockHandler, "index");
        let sub_router = Router::with_urls(vec![route]);
        let router = Router::with_urls(vec![Route::with_router("/", sub_router)]);

        let url = router
            .reverse(None, "index", &ReverseParamMap::new())
            .unwrap();

        assert_eq!(url, "/");
    }

    #[test]
    fn router_reverse_nested_under_root_mount_no_double_slash() {
        let route = Route::with_handler_and_name("/inner", MockHandler, "inner");
        let sub_router = Router::with_urls(vec![route]);
        let router = Router::with_urls(vec![Route::with_router("/", sub_router)]);

        let url = router
            .reverse(None, "inner", &ReverseParamMap::new())
            .unwrap();

        assert_eq!(url, "/inner");
    }

    #[test]
    fn router_reverse_deeply_nested_root_mounts_no_double_slash() {
        let route = Route::with_handler_and_name("/leaf", MockHandler, "leaf");
        let inner_router = Router::with_urls(vec![route]);
        let mid_router = Router::with_urls(vec![Route::with_router("/", inner_router)]);
        let router = Router::with_urls(vec![Route::with_router("/", mid_router)]);

        let url = router
            .reverse(None, "leaf", &ReverseParamMap::new())
            .unwrap();

        assert_eq!(url, "/leaf");
    }

    #[test]
    fn router_reverse_app_name() {
        let route = Route::with_handler_and_name("/test", MockHandler, "test");
        let mut router_1 = Router::with_urls(vec![route.clone()]);
        router_1.set_app_name(AppName("app_1".to_string()));
        let mut router_2 = Router::with_urls(vec![route.clone()]);
        router_2.set_app_name(AppName("app_2".to_string()));
        let root_router = Router::with_urls(vec![
            Route::with_router("/", router_1),
            Route::with_router("/sub", router_2),
        ]);

        let params = ReverseParamMap::new();
        let url = root_router.reverse(Some("app_2"), "test", &params).unwrap();

        assert_eq!(url, "/sub/test");
    }

    #[test]
    fn router_reverse_app_name_nested() {
        let route = Route::with_handler_and_name("/test", MockHandler, "test");
        let router = Router::with_urls(vec![route.clone()]);
        let sub_router = Router::with_urls(vec![Route::with_router("/sub", router)]);
        let mut root_router = Router::with_urls(vec![Route::with_router("/subsub", sub_router)]);
        root_router.set_app_name(AppName("app_root".to_string()));

        let params = ReverseParamMap::new();
        let url = root_router
            .reverse(Some("app_root"), "test", &params)
            .unwrap();

        assert_eq!(url, "/subsub/sub/test");
    }

    #[test]
    fn router_reverse_option() {
        let route = Route::with_handler_and_name("/test", MockHandler, "test");
        let router = Router::with_urls(vec![route.clone()]);
        let params = ReverseParamMap::new();
        let url = router
            .reverse_option(None, "test", &params)
            .unwrap()
            .unwrap();
        assert_eq!(url, "/test");
    }

    #[test]
    fn router_routes() {
        let route = Route::with_handler("/test", MockHandler);
        let router = Router::with_urls(vec![route.clone()]);
        assert_eq!(router.routes().len(), 1);
    }

    #[test]
    fn router_is_empty() {
        let router = Router::with_urls(vec![]);
        assert!(router.is_empty());
    }

    #[test]
    fn route_with_handler() {
        let route = Route::with_handler("/test", MockHandler);
        assert_eq!(route.url.to_string(), "/test");
    }

    #[test]
    fn route_with_handler_and_params() {
        let route = Route::with_handler("/test/{id}", MockHandler);
        assert_eq!(route.url.to_string(), "/test/{id}");
    }

    #[test]
    fn route_with_handler_and_name() {
        let route = Route::with_handler_and_name("/test", MockHandler, "test");
        assert_eq!(route.url.to_string(), "/test");
        assert_eq!(route.name, Some(RouteName("test".to_string())));
    }

    #[test]
    fn route_with_router() {
        let sub_route = Route::with_handler("/sub", MockHandler);
        let sub_router = Router::with_urls(vec![sub_route]);
        let route = Route::with_router("/test", sub_router);
        assert_eq!(route.url.to_string(), "/test");
    }

    #[test]
    fn test_reverse_macro() {
        let route = Route::with_handler_and_name("/test/{id}", MockHandler, "test");
        let router = Router::with_urls(vec![route]);

        let request = TestRequestBuilder::get("/").router(router).build();
        let url = reverse!(request, "test", id = 123).unwrap();

        assert_eq!(url, "/test/123");
    }

    #[test]
    fn test_reverse_redirect_macro() {
        let route = Route::with_handler_and_name("/test/{id}", MockHandler, "test");
        let router = Router::with_urls(vec![route]);

        let request = TestRequestBuilder::get("/").router(router).build();
        let response = cot::reverse_redirect!(request, "test", id = 123).unwrap();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(response.headers().get("location").unwrap(), "/test/123");
    }

    fn test_request() -> Request {
        TestRequestBuilder::get("/test").build()
    }
}
