//! OpenAPI integration for Cot Core.
//!
//! This module provides core traits and utilities for OpenAPI integration.
//! It contains the minimal types needed by the router to support OpenAPI.
//! Higher-level OpenAPI functionality is implemented in the main `cot` crate.

use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;

use aide::openapi::{Operation, PathItem};
use schemars::SchemaGenerator;

use crate::Method;
use crate::handler::{BoxRequestHandler, RequestHandler};
use crate::request::Request;
use crate::response::Response;

/// Context for API route generation.
///
/// `RouteContext` is used to generate OpenAPI paths from routes. It provides
/// information about the route, such as the HTTP method and route parameter
/// names.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct RouteContext<'a> {
    /// The HTTP method of the route.
    pub method: Option<Method>,
    /// The names of the route parameters.
    pub param_names: &'a [&'a str],
}

impl RouteContext<'_> {
    /// Creates a new `RouteContext` with no information about the route.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot_core::openapi::RouteContext;
    ///
    /// let context = RouteContext::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            method: None,
            param_names: &[],
        }
    }
}

impl Default for RouteContext<'_> {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the OpenAPI path item for the route - a collection of different
/// HTTP operations (GET, POST, etc.) at a given URL.
///
/// You usually shouldn't need to implement this directly. Instead, it's easiest
/// to use [`ApiMethodRouter`](crate::router::method::method::ApiMethodRouter).
/// You might want to implement this if you want to create a wrapper that
/// modifies the OpenAPI spec or want to create it manually.
///
/// An object implementing [`AsApiRoute`] can be passed to
/// [`Route::with_api_handler`](crate::router::Route::with_api_handler) to
/// generate the OpenAPI specs.
///
/// # Examples
///
/// ```
/// use aide::openapi::PathItem;
/// use cot_core::openapi::{AsApiRoute, RouteContext};
/// use schemars::SchemaGenerator;
///
/// struct RouteWrapper<T>(T);
///
/// impl<T: AsApiRoute> AsApiRoute for RouteWrapper<T> {
///     fn as_api_route(
///         &self,
///         route_context: &RouteContext<'_>,
///         schema_generator: &mut SchemaGenerator,
///     ) -> PathItem {
///         let mut spec = self.0.as_api_route(route_context, schema_generator);
///         spec.summary = Some("This route was wrapped with RouteWrapper".to_owned());
///         spec
///     }
/// }
/// ```
pub trait AsApiRoute {
    /// Returns the OpenAPI path item for the route.
    ///
    /// # Examples
    ///
    /// ```
    /// use aide::openapi::PathItem;
    /// use cot_core::openapi::{AsApiRoute, RouteContext};
    /// use schemars::SchemaGenerator;
    ///
    /// struct RouteWrapper<T>(T);
    ///
    /// impl<T: AsApiRoute> AsApiRoute for RouteWrapper<T> {
    ///     fn as_api_route(
    ///         &self,
    ///         route_context: &RouteContext<'_>,
    ///         schema_generator: &mut SchemaGenerator,
    ///     ) -> PathItem {
    ///         let mut spec = self.0.as_api_route(route_context, schema_generator);
    ///         spec.summary = Some("This route was wrapped with RouteWrapper".to_owned());
    ///         spec
    ///     }
    /// }
    /// ```
    fn as_api_route(
        &self,
        route_context: &RouteContext<'_>,
        schema_generator: &mut SchemaGenerator,
    ) -> PathItem;
}

/// Trait for handlers that can be used in API routes with OpenAPI
/// documentation.
///
/// This trait combines [`BoxRequestHandler`] and [`AsApiRoute`] to allow
/// handlers to both process requests and provide OpenAPI documentation.
pub trait BoxApiEndpointRequestHandler: BoxRequestHandler + AsApiRoute {
    // TODO: consider removing this when Rust trait_upcasting is stabilized and we
    // bump the MSRV (lands in Rust 1.86)
    fn as_box_request_handler(&self) -> &(dyn BoxRequestHandler + Send + Sync);
}

/// Wraps a handler into a type-erased [`BoxApiEndpointRequestHandler`].
///
/// This function is used internally by the router to convert handlers into
/// trait objects that can be stored and invoked dynamically.
pub fn into_box_api_endpoint_request_handler<HandlerParams, H>(
    handler: H,
) -> impl BoxApiEndpointRequestHandler
where
    H: RequestHandler<HandlerParams> + AsApiRoute + Send + Sync,
{
    struct Inner<HandlerParams, H>(H, PhantomData<fn() -> HandlerParams>);

    impl<HandlerParams, H> BoxRequestHandler for Inner<HandlerParams, H>
    where
        H: RequestHandler<HandlerParams> + AsApiRoute + Send + Sync,
    {
        fn handle(
            &self,
            request: Request,
        ) -> Pin<Box<dyn Future<Output = crate::Result<Response>> + Send + '_>> {
            Box::pin(self.0.handle(request))
        }
    }

    impl<HandlerParams, H> AsApiRoute for Inner<HandlerParams, H>
    where
        H: RequestHandler<HandlerParams> + AsApiRoute + Send + Sync,
    {
        fn as_api_route(
            &self,
            route_context: &RouteContext<'_>,
            schema_generator: &mut SchemaGenerator,
        ) -> PathItem {
            self.0.as_api_route(route_context, schema_generator)
        }
    }

    impl<HandlerParams, H> BoxApiEndpointRequestHandler for Inner<HandlerParams, H>
    where
        H: RequestHandler<HandlerParams> + AsApiRoute + Send + Sync,
    {
        fn as_box_request_handler(&self) -> &(dyn BoxRequestHandler + Send + Sync) {
            self
        }
    }

    Inner(handler, PhantomData)
}

/// Returns the OpenAPI operation for the route - a specific HTTP operation
/// (GET, POST, etc.) at a given URL.
///
/// You shouldn't typically need to implement this trait yourself. It is
/// implemented automatically for all functions that can be used as request
/// handlers, as long as all the parameters and the return type implement the
/// [`ApiOperationPart`] trait. You might need to implement it yourself if you
/// are creating a wrapper over a [`RequestHandler`] that adds some extra
/// functionality, or you want to modify the OpenAPI specs or create them
/// manually.
///
/// # Examples
///
/// ```
/// use cot::aide::openapi::Operation;
/// use cot::openapi::{AsApiOperation, RouteContext};
/// use schemars::SchemaGenerator;
///
/// struct HandlerWrapper<T>(T);
///
/// impl<T> AsApiOperation for HandlerWrapper<T> {
///     fn as_api_operation(
///         &self,
///         route_context: &RouteContext<'_>,
///         schema_generator: &mut SchemaGenerator,
///     ) -> Option<Operation> {
///         // a wrapper that hides the operation from OpenAPI spec
///         None
///     }
/// }
///
/// # assert!(HandlerWrapper::<()>(()).as_api_operation(&RouteContext::new(), &mut SchemaGenerator::default()).is_none());
/// ```
pub trait AsApiOperation<T = ()> {
    /// Returns the OpenAPI operation for the route.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::aide::openapi::Operation;
    /// use cot::openapi::{AsApiOperation, RouteContext};
    /// use schemars::SchemaGenerator;
    ///
    /// struct HandlerWrapper<T>(T);
    ///
    /// impl<T> AsApiOperation for HandlerWrapper<T> {
    ///     fn as_api_operation(
    ///         &self,
    ///         route_context: &RouteContext<'_>,
    ///         schema_generator: &mut SchemaGenerator,
    ///     ) -> Option<Operation> {
    ///         // a wrapper that hides the operation from OpenAPI spec
    ///         None
    ///     }
    /// }
    ///
    /// # assert!(HandlerWrapper::<()>(()).as_api_operation(&RouteContext::new(), &mut SchemaGenerator::default()).is_none());
    /// ```
    fn as_api_operation(
        &self,
        route_context: &RouteContext<'_>,
        schema_generator: &mut SchemaGenerator,
    ) -> Option<Operation>;
}

pub(crate) trait BoxApiRequestHandler: BoxRequestHandler + AsApiOperation {}

pub(crate) fn into_box_api_request_handler<HandlerParams, ApiParams, H>(
    handler: H,
) -> impl BoxApiRequestHandler
where
    H: RequestHandler<HandlerParams> + AsApiOperation<ApiParams> + Send + Sync,
{
    struct Inner<HandlerParams, ApiParams, H>(
        H,
        PhantomData<fn() -> HandlerParams>,
        PhantomData<fn() -> ApiParams>,
    );

    impl<HandlerParams, ApiParams, H> BoxRequestHandler for Inner<HandlerParams, ApiParams, H>
    where
        H: RequestHandler<HandlerParams> + AsApiOperation<ApiParams> + Send + Sync,
    {
        fn handle(
            &self,
            request: Request,
        ) -> Pin<Box<dyn Future<Output = crate::Result<Response>> + Send + '_>> {
            Box::pin(self.0.handle(request))
        }
    }

    impl<HandlerParams, ApiParams, H> AsApiOperation for Inner<HandlerParams, ApiParams, H>
    where
        H: RequestHandler<HandlerParams> + AsApiOperation<ApiParams> + Send + Sync,
    {
        fn as_api_operation(
            &self,
            route_context: &RouteContext<'_>,
            schema_generator: &mut SchemaGenerator,
        ) -> Option<Operation> {
            self.0.as_api_operation(route_context, schema_generator)
        }
    }

    impl<HandlerParams, ApiParams, H> BoxApiRequestHandler for Inner<HandlerParams, ApiParams, H> where
        H: RequestHandler<HandlerParams> + AsApiOperation<ApiParams> + Send + Sync
    {
    }

    Inner(handler, PhantomData, PhantomData)
}
