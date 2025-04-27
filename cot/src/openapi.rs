//! OpenAPI integration for Cot.
//!
//! This module provides traits and utilities for generating OpenAPI
//! documentation for Cot applications. The idea is to be able to use Cot's
//! existing request handlers and extractors to generate OpenAPI documentation
//! automatically.
//!
//! # Usage
//!
//! 1. Add [`#[derive(schemars::JsonSchema)]`](schemars::JsonSchema) to the
//!    types used in the extractors and response types.
//! 2. Use [`ApiMethodRouter`](crate::router::method::openapi::ApiMethodRouter)
//!    to set up your API routes and register them with a router (possibly using
//!    convenience functions, such as
//!    [`api_get`](crate::router::method::openapi::api_get) or
//!    [`api_post`](crate::router::method::openapi::api_post)).
//! 3. Register your
//!    [`ApiMethodRouter`](crate::router::method::openapi::ApiMethodRouter)s
//!    with a [`Router`](crate::router::Router) using
//!    [`Route::with_api_handler`](crate::router::Route::with_api_handler) or
//!    [`Route::with_api_handler_and_name`](crate::router::Route::with_api_handler_and_name).
//! 4. Register the [`SwaggerUi`](crate::openapi::swagger_ui::SwaggerUi) app
//!    inside [`Project::register_apps`](crate::project::Project::register_apps)
//!    using [`AppBuilder::register_with_views`](crate::project::AppBuilder::register_with_views).
//!    Remember to enable
//!    [`StaticFilesMiddleware`](crate::static_files::StaticFilesMiddleware) as
//!    well!
//!
//! # Examples
//!
//! ```
//! use cot::config::ProjectConfig;
//! use cot::openapi::swagger_ui::SwaggerUi;
//! use cot::project::{MiddlewareContext, RegisterAppsContext, RootHandlerBuilder};
//! use cot::request::extractors::Json;
//! use cot::response::{Response, ResponseExt};
//! use cot::router::method::openapi::api_post;
//! use cot::router::{Route, Router};
//! use cot::static_files::StaticFilesMiddleware;
//! use cot::{App, AppBuilder, BoxedHandler, Project, StatusCode};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Deserialize, schemars::JsonSchema)]
//! struct AddRequest {
//!     a: i32,
//!     b: i32,
//! }
//!
//! #[derive(Serialize, schemars::JsonSchema)]
//! struct AddResponse {
//!     result: i32,
//! }
//!
//! async fn add(Json(add_request): Json<AddRequest>) -> cot::Result<Response> {
//!     Response::new_json(
//!         StatusCode::OK,
//!         &AddResponse {
//!             result: add_request.a + add_request.b,
//!         },
//!     )
//! }
//!
//! struct AddApp;
//! impl App for AddApp {
//! #     fn name(&self) -> &'static str {
//! #         env!("CARGO_PKG_NAME")
//! #     }
//! #
//!     fn router(&self) -> Router {
//!         Router::with_urls([Route::with_api_handler("/add/", api_post(add))])
//!     }
//! }
//!
//! struct ApiProject;
//! impl Project for ApiProject {
//! #     fn config(&self, _config_name: &str) -> cot::Result<ProjectConfig> {
//! #         Ok(ProjectConfig::dev_default())
//! #     }
//! #
//!     fn middlewares(
//!         &self,
//!         handler: RootHandlerBuilder,
//!         context: &MiddlewareContext,
//!     ) -> BoxedHandler {
//!         handler
//!             // StaticFilesMiddleware is needed for SwaggerUi to serve its
//!             // CSS and JavaScript files
//!             .middleware(StaticFilesMiddleware::from_context(context))
//!             .build()
//!     }
//!
//!     fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
//!         apps.register_with_views(SwaggerUi::new(), "/swagger");
//!         apps.register_with_views(AddApp, "");
//!     }
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> cot::Result<()> {
//! #     let mut client = cot::test::Client::new(ApiProject).await;
//! #
//! #     let response = client.get("/swagger/").await?;
//! #     assert_eq!(response.status(), StatusCode::OK);
//! #
//! #     Ok(())
//! # }
//! ```

#[cfg(feature = "swagger-ui")]
pub mod swagger_ui;

use std::marker::PhantomData;
use std::pin::Pin;

use aide::openapi::{
    MediaType, Operation, Parameter, ParameterData, ParameterSchemaOrContent, PathItem, PathStyle,
    QueryStyle, ReferenceOr, RequestBody,
};
use http::request::Parts;
use indexmap::IndexMap;
use schemars::schema::{InstanceType, Schema, SchemaObject, SingleOrVec};
use schemars::{JsonSchema, SchemaGenerator};

use crate::auth::Auth;
use crate::form::Form;
use crate::handler::BoxRequestHandler;
use crate::request::Request;
use crate::request::extractors::{
    FromRequest, FromRequestParts, Json, Path, RequestForm, UrlQuery,
};
use crate::response::Response;
use crate::router::Urls;
use crate::session::Session;
use crate::{Method, RequestHandler};

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
    /// use cot::openapi::RouteContext;
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
/// to use [`ApiMethodRouter`](crate::router::method::openapi::ApiMethodRouter).
/// You might want to implement this if you want to create a wrapper that
/// modifies the OpenAPI spec or want to create it manually.
///
/// An object implementing [`AsApiRoute`] can be used passed to
/// [`Route::with_api_handler`](crate::router::Route::with_api_handler) to
/// generate the OpenAPI specs.
///
/// # Examples
///
/// ```
/// use aide::openapi::PathItem;
/// use cot::aide::openapi::Operation;
/// use cot::openapi::{AsApiOperation, AsApiRoute, RouteContext};
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
///
/// # assert_eq!(
/// #     RouteWrapper(cot::router::method::openapi::ApiMethodRouter::new())
/// #         .as_api_route(&RouteContext::new(), &mut SchemaGenerator::default())
/// #         .summary,
/// #     Some("This route was wrapped with RouteWrapper".to_owned())
/// # );
/// ```
pub trait AsApiRoute {
    /// Returns the OpenAPI path item for the route.
    ///
    /// # Examples
    ///
    /// ```
    /// use aide::openapi::PathItem;
    /// use cot::aide::openapi::Operation;
    /// use cot::openapi::{AsApiOperation, AsApiRoute, RouteContext};
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
    ///
    /// # assert_eq!(
    /// #     RouteWrapper(cot::router::method::openapi::ApiMethodRouter::new())
    /// #         .as_api_route(&RouteContext::new(), &mut SchemaGenerator::default())
    /// #         .summary,
    /// #     Some("This route was wrapped with RouteWrapper".to_owned())
    /// # );
    /// ```
    fn as_api_route(
        &self,
        route_context: &RouteContext<'_>,
        schema_generator: &mut SchemaGenerator,
    ) -> PathItem;
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
        ) -> Pin<Box<dyn Future<Output = cot::Result<Response>> + Send + '_>> {
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

pub(crate) trait BoxApiEndpointRequestHandler: BoxRequestHandler + AsApiRoute {
    // TODO: consider removing this when Rust trait_upcasting is stabilized and we
    // bump the MSRV (lands in Rust 1.86)
    fn as_box_request_handler(&self) -> &(dyn BoxRequestHandler + Send + Sync);
}

pub(crate) fn into_box_api_endpoint_request_handler<HandlerParams, H>(
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
        ) -> Pin<Box<dyn Future<Output = cot::Result<Response>> + Send + '_>> {
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

/// A wrapper type that allows using non-OpenAPI handlers and request parameters
/// in OpenAPI routes.
///
/// If you need an extractor or a handler that does not implement
/// [`AsApiOperation`]/[`ApiOperationPart`], you can wrap it in a `NoApi` to
/// indicate that it should just be ignored during OpenAPI generation.
///
/// # Examples
///
/// ```
/// use cot::openapi::NoApi;
/// use cot::request::extractors::FromRequestParts;
/// use cot::response::Response;
/// use cot::router::Route;
/// use cot::router::method::openapi::api_get;
/// use http::request::Parts;
///
/// struct MyExtractor;
/// impl FromRequestParts for MyExtractor {
///     async fn from_request_parts(parts: &mut Parts) -> cot::Result<Self> {
///         // ...
/// #         unimplemented!()
///     }
/// }
///
/// async fn handler(NoApi(extractor): NoApi<MyExtractor>) -> cot::Result<Response> {
///     // MyExtractor doesn't have to implement ApiOperationPart and
///     // doesn't show up in the OpenAPI spec
/// #     unimplemented!()
/// }
///
/// let router =
///     cot::router::Router::with_urls([Route::with_api_handler("/with_api", api_get(handler))]);
/// ```
///
/// ```
/// use cot::openapi::NoApi;
/// use cot::response::Response;
/// use cot::router::Route;
/// use cot::router::method::openapi::api_get;
///
/// async fn handler_with_openapi() -> cot::Result<Response> {
///     // ...
/// #     unimplemented!()
/// }
/// async fn handler_without_openapi() -> cot::Result<Response> {
///     // ...
/// #     unimplemented!()
/// }
///
/// let router = cot::router::Router::with_urls([Route::with_api_handler(
///     "/with_api",
///     // POST will be ignored in OpenAPI spec
///     api_get(handler_with_openapi).post(NoApi(handler_without_openapi)),
/// )]);
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NoApi<T>(pub T);

impl<HandlerParams, H> RequestHandler<HandlerParams> for NoApi<H>
where
    H: RequestHandler<HandlerParams>,
{
    fn handle(&self, request: Request) -> impl Future<Output = cot::Result<Response>> + Send {
        self.0.handle(request)
    }
}

impl<T: FromRequest> FromRequest for NoApi<T> {
    async fn from_request(request: Request) -> cot::Result<Self> {
        T::from_request(request).await.map(Self)
    }
}

impl<T: FromRequestParts> FromRequestParts for NoApi<T> {
    async fn from_request_parts(parts: &mut Parts) -> cot::Result<Self> {
        T::from_request_parts(parts).await.map(Self)
    }
}

impl<T> ApiOperationPart for NoApi<T> {}

impl<T> AsApiOperation for NoApi<T> {
    fn as_api_operation(
        &self,
        _route_context: &RouteContext<'_>,
        _schema_generator: &mut SchemaGenerator,
    ) -> Option<Operation> {
        None
    }
}

macro_rules! impl_as_openapi_operation {
    ($($ty:ident),*) => {
        impl<T, $($ty,)* R, Response> AsApiOperation<($($ty,)*)> for T
        where
            T: Fn($($ty,)*) -> R + Clone + Send + Sync + 'static,
            $($ty: ApiOperationPart,)*
            R: for<'a> Future<Output = Response> + Send,
            Response: ApiOperationPart,
        {
            #[allow(non_snake_case)]
            fn as_api_operation(
                &self,
                route_context: &RouteContext<'_>,
                schema_generator: &mut SchemaGenerator,
            ) -> Option<Operation> {
                let mut operation = Operation::default();

                $(
                    $ty::modify_api_operation(
                        &mut operation,
                        &route_context,
                        schema_generator
                    );
                )*
                Response::modify_api_operation(
                    &mut operation,
                    &route_context,
                    schema_generator
                );

                Some(operation)
            }
        }
    };
}

impl_as_openapi_operation!();
impl_as_openapi_operation!(P1);
impl_as_openapi_operation!(P1, P2);
impl_as_openapi_operation!(P1, P2, P3);
impl_as_openapi_operation!(P1, P2, P3, P4);
impl_as_openapi_operation!(P1, P2, P3, P4, P5);
impl_as_openapi_operation!(P1, P2, P3, P4, P5, P6);
impl_as_openapi_operation!(P1, P2, P3, P4, P5, P6, P7);
impl_as_openapi_operation!(P1, P2, P3, P4, P5, P6, P7, P8);
impl_as_openapi_operation!(P1, P2, P3, P4, P5, P6, P7, P8, P9);
impl_as_openapi_operation!(P1, P2, P3, P4, P5, P6, P7, P8, P9, P10);

/// A trait that can be implemented for types that should be taken into
/// account when generating OpenAPI paths.
///
/// When implementing this trait for a type, you can modify the `Operation`
/// object to add information about the type to the OpenAPI spec. The
/// default implementation of [`ApiOperationPart::modify_api_operation`]
/// does nothing to indicate that the type has no effect on the OpenAPI spec.
///
/// # Example
///
/// ```
/// use cot::aide::openapi::{Operation, MediaType, ReferenceOr, RequestBody};
/// use cot::openapi::{ApiOperationPart, RouteContext};
/// use cot::request::Request;
/// use cot::request::extractors::FromRequest;
/// use indexmap::IndexMap;
/// use cot::schemars::SchemaGenerator;
/// use serde::de::DeserializeOwned;
///
/// pub struct Json<D>(pub D);
///
/// impl<D: DeserializeOwned> FromRequest for Json<D> {
///     async fn from_request(mut request: Request) -> cot::Result<Self> {
///         // parse the request body as JSON
/// #       unimplemented!()
///     }
/// }
///
/// impl<D: schemars::JsonSchema> ApiOperationPart for Json<D> {
///     fn modify_api_operation(
///         operation: &mut Operation,
///         _route_context: &RouteContext<'_>,
///         schema_generator: &mut SchemaGenerator,
///     ) {
///         operation.request_body = Some(ReferenceOr::Item(RequestBody {
///             content: IndexMap::from([(
///                 "application/json".to_owned(),
///                 MediaType {
///                     schema: Some(aide::openapi::SchemaObject {
///                         json_schema: D::json_schema(schema_generator),
///                         external_docs: None,
///                         example: None,
///                     }),
///                     ..Default::default()
///                 },
///             )]),
///             ..Default::default()
///         }));
///     }
/// }
///
/// # let mut operation = Operation::default();
/// # let route_context = RouteContext::new();
/// # let mut schema_generator = SchemaGenerator::default();
/// # Json::<String>::modify_api_operation(&mut operation, &route_context, &mut schema_generator);
/// # assert!(operation.request_body.is_some());
/// ```
pub trait ApiOperationPart {
    /// Modify the OpenAPI operation object.
    ///
    /// This function is called by the framework when generating the OpenAPI
    /// spec for a route. You can use this function to add custom information
    /// to the operation object.
    ///
    /// The default implementation does nothing.
    ///
    /// # Examples
    ///
    /// ```
    /// use aide::openapi::Operation;
    /// use cot::openapi::{ApiOperationPart, RouteContext};
    /// use schemars::SchemaGenerator;
    ///
    /// struct MyExtractor<T>(T);
    ///
    /// impl<T> ApiOperationPart for MyExtractor<T> {
    ///     fn modify_api_operation(
    ///         operation: &mut Operation,
    ///         _route_context: &RouteContext<'_>,
    ///         _schema_generator: &mut SchemaGenerator,
    ///     ) {
    ///         // Add custom OpenAPI information to the operation
    ///     }
    /// }
    /// ```
    #[allow(unused)]
    fn modify_api_operation(
        operation: &mut Operation,
        route_context: &RouteContext<'_>,
        schema_generator: &mut SchemaGenerator,
    ) {
    }
}

impl ApiOperationPart for Request {}
impl ApiOperationPart for Urls {}
impl ApiOperationPart for Method {}
impl ApiOperationPart for Session {}
impl ApiOperationPart for Auth {}
#[cfg(feature = "db")]
impl ApiOperationPart for crate::request::extractors::RequestDb {}

impl<D: JsonSchema> ApiOperationPart for Json<D> {
    fn modify_api_operation(
        operation: &mut Operation,
        _route_context: &RouteContext<'_>,
        schema_generator: &mut SchemaGenerator,
    ) {
        operation.request_body = Some(ReferenceOr::Item(RequestBody {
            content: IndexMap::from([(
                crate::headers::JSON_CONTENT_TYPE.to_string(),
                MediaType {
                    schema: Some(aide::openapi::SchemaObject {
                        json_schema: D::json_schema(schema_generator),
                        external_docs: None,
                        example: None,
                    }),
                    ..Default::default()
                },
            )]),
            required: true,
            ..Default::default()
        }));
    }
}

impl<D: JsonSchema> ApiOperationPart for Path<D> {
    #[track_caller]
    fn modify_api_operation(
        operation: &mut Operation,
        route_context: &RouteContext<'_>,
        schema_generator: &mut SchemaGenerator,
    ) {
        let schema = D::json_schema(schema_generator).into_object();

        if let Some(array) = schema.array {
            // a tuple of path params, e.g. Path<(i32, String)>

            if let Some(items) = array.items {
                match items {
                    SingleOrVec::Single(_) => {}
                    SingleOrVec::Vec(item_list) => {
                        assert_eq!(
                            route_context.param_names.len(),
                            item_list.len(),
                            "the number of path parameters in the route URL must match \
                            the number of params in the Path type (found path params: {:?})",
                            route_context.param_names,
                        );

                        for (&param_name, item) in
                            route_context.param_names.iter().zip(item_list.into_iter())
                        {
                            let schema = item.into_object();

                            add_path_param(operation, schema, param_name.to_owned());
                        }
                    }
                }
            }
        } else if let Some(object) = schema.object {
            // a struct of path params, e.g. Path<MyStruct>

            let mut route_context_sorted = route_context.param_names.to_vec();
            route_context_sorted.sort_unstable();
            let mut object_props_sorted = object.properties.keys().collect::<Vec<_>>();
            object_props_sorted.sort();

            assert_eq!(
                route_context_sorted, object_props_sorted,
                "Path parameters in the route info must exactly match parameters \
                in the Path type. Make sure that the type you pass to Path contains \
                all the parameters for the route, and that the names match exactly."
            );

            for (key, item) in object.properties {
                let object_item = item.into_object();

                add_path_param(operation, object_item, key);
            }
        } else if schema.instance_type.is_some() {
            // single path param, e.g. Path<i32>

            assert_eq!(
                route_context.param_names.len(),
                1,
                "the number of path parameters in the route URL must equal \
                to 1 if a single parameter was passed to the Path type (found path params: {:?})",
                route_context.param_names,
            );

            add_path_param(operation, schema, route_context.param_names[0].to_owned());
        }
    }
}

impl<D: JsonSchema> ApiOperationPart for UrlQuery<D> {
    fn modify_api_operation(
        operation: &mut Operation,
        _route_context: &RouteContext<'_>,
        schema_generator: &mut SchemaGenerator,
    ) {
        let schema = D::json_schema(schema_generator).into_object();

        if let Some(object) = schema.object {
            for (key, item) in object.properties {
                let object_item = item.into_object();

                add_query_param(operation, object_item, key);
            }
        }
    }
}

impl<F: Form + JsonSchema> ApiOperationPart for RequestForm<F> {
    fn modify_api_operation(
        operation: &mut Operation,
        route_context: &RouteContext<'_>,
        schema_generator: &mut SchemaGenerator,
    ) {
        if route_context.method == Some(Method::GET) || route_context.method == Some(Method::HEAD) {
            let schema = F::json_schema(schema_generator).into_object();

            if let Some(object) = schema.object {
                for (key, item) in object.properties {
                    let object_item = item.into_object();

                    add_query_param(operation, object_item, key);
                }
            }
        } else {
            operation.request_body = Some(ReferenceOr::Item(RequestBody {
                content: IndexMap::from([(
                    crate::headers::FORM_CONTENT_TYPE.to_string(),
                    MediaType {
                        schema: Some(aide::openapi::SchemaObject {
                            json_schema: F::json_schema(schema_generator),
                            external_docs: None,
                            example: None,
                        }),
                        ..Default::default()
                    },
                )]),
                required: true,
                ..Default::default()
            }));
        }
    }
}

fn add_path_param(operation: &mut Operation, mut schema: SchemaObject, param_name: String) {
    let required = extract_is_required(&mut schema);

    operation
        .parameters
        .push(ReferenceOr::Item(Parameter::Path {
            parameter_data: param_with_name(param_name, schema, required),
            style: PathStyle::default(),
        }));
}

fn add_query_param(operation: &mut Operation, mut schema: SchemaObject, param_name: String) {
    let required = extract_is_required(&mut schema);

    operation
        .parameters
        .push(ReferenceOr::Item(Parameter::Query {
            parameter_data: param_with_name(param_name, schema, required),
            allow_reserved: false,
            style: QueryStyle::default(),
            allow_empty_value: None,
        }));
}

fn extract_is_required(object_item: &mut SchemaObject) -> bool {
    match &mut object_item.instance_type {
        Some(SingleOrVec::Vec(type_list)) => {
            let nullable = type_list.contains(&InstanceType::Null);
            type_list.retain(|&element| element != InstanceType::Null);
            !nullable
        }
        Some(SingleOrVec::Single(_)) | None => true,
    }
}

fn param_with_name(
    param_name: String,
    schema_object: SchemaObject,
    required: bool,
) -> ParameterData {
    ParameterData {
        name: param_name,
        description: None,
        required,
        deprecated: None,
        format: ParameterSchemaOrContent::Schema(aide::openapi::SchemaObject {
            json_schema: Schema::Object(schema_object),
            external_docs: None,
            example: None,
        }),
        example: None,
        examples: IndexMap::default(),
        explode: None,
        extensions: IndexMap::default(),
    }
}

impl ApiOperationPart for crate::Result<Response> {
    fn modify_api_operation(
        operation: &mut Operation,
        _route_context: &RouteContext<'_>,
        _schema_generator: &mut SchemaGenerator,
    ) {
        let responses = operation.responses.get_or_insert_default();
        responses.default = Some(ReferenceOr::Item(aide::openapi::Response {
            description: "*&lt;unspecified&gt;*".to_string(),
            ..Default::default()
        }));
    }
}

#[cfg(test)]
mod tests {
    use aide::openapi::{Operation, Parameter};
    use schemars::SchemaGenerator;
    use schemars::schema::Schema;
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::openapi::AsApiOperation;
    use crate::request::extractors::{Json, Path, UrlQuery};
    use crate::response::{Response, ResponseExt};
    use crate::{Body, StatusCode};

    #[derive(Deserialize, Serialize, schemars::JsonSchema)]
    struct TestRequest {
        field1: String,
        field2: i32,
        optional_field: Option<bool>,
    }

    #[derive(Form, schemars::JsonSchema)]
    struct TestForm {
        field1: String,
        field2: i32,
        optional_field: Option<bool>,
    }

    #[derive(schemars::JsonSchema)]
    #[expect(dead_code)] // fields are never actually read
    struct TestPath {
        field1: String,
        field2: i32,
    }

    async fn test_handler() -> cot::Result<Response> {
        Ok(Response::new_html(StatusCode::OK, Body::fixed("test")))
    }

    #[test]
    fn route_context() {
        let context = RouteContext::default();
        assert!(context.method.is_none());
        assert!(context.param_names.is_empty());

        let context = RouteContext::new();
        assert!(context.method.is_none());
        assert!(context.param_names.is_empty());
    }

    #[test]
    fn no_api_handler() {
        let handler = NoApi(test_handler);
        let route_context = RouteContext::new();
        let mut schema_generator = SchemaGenerator::default();

        let operation = handler.as_api_operation(&route_context, &mut schema_generator);
        assert!(operation.is_none());
    }

    #[test]
    fn no_api_param() {
        let mut operation = Operation::default();
        let route_context = RouteContext::new();
        let mut schema_generator = SchemaGenerator::default();

        NoApi::<()>::modify_api_operation(&mut operation, &route_context, &mut schema_generator);
        assert_eq!(operation, Operation::default());
    }

    #[test]
    fn api_operation_part_for_json() {
        let mut operation = Operation::default();
        let route_context = RouteContext::new();
        let mut schema_generator = SchemaGenerator::default();

        Json::<TestRequest>::modify_api_operation(
            &mut operation,
            &route_context,
            &mut schema_generator,
        );

        if let Some(ReferenceOr::Item(request_body)) = &operation.request_body {
            let content = &request_body.content;
            assert!(content.contains_key("application/json"));
            let content_json = content.get("application/json").unwrap();
            let schema_obj = &content_json.schema.clone().unwrap().json_schema;

            if let Schema::Object(obj) = schema_obj {
                if let Some(properties) = &obj.object.as_ref().map(|o| &o.properties) {
                    assert!(properties.contains_key("field1"));
                    assert!(properties.contains_key("field2"));
                    assert!(properties.contains_key("optional_field"));
                } else {
                    panic!("Expected properties in schema");
                }
            } else {
                panic!("Expected object schema");
            }
        } else {
            panic!("Expected request body: {:?}", &operation.request_body);
        }
    }

    #[test]
    fn api_operation_part_for_form_get() {
        let mut operation = Operation::default();
        let mut route_context = RouteContext::new();
        route_context.method = Some(Method::GET);
        let mut schema_generator = SchemaGenerator::default();

        RequestForm::<TestForm>::modify_api_operation(
            &mut operation,
            &route_context,
            &mut schema_generator,
        );

        assert_eq!(operation.parameters.len(), 3); // field1, field2, optional_field

        for param in &operation.parameters {
            match param {
                ReferenceOr::Item(Parameter::Query { parameter_data, .. }) => {
                    assert!(
                        parameter_data.name == "field1"
                            || parameter_data.name == "field2"
                            || parameter_data.name == "optional_field"
                    );

                    if parameter_data.name == "optional_field" {
                        assert!(!parameter_data.required);
                    } else {
                        assert!(parameter_data.required);
                    }
                }
                _ => panic!("Expected query parameter"),
            }
        }
    }

    #[test]
    fn api_operation_part_for_form_post() {
        let mut operation = Operation::default();
        let mut route_context = RouteContext::new();
        route_context.method = Some(Method::POST);
        let mut schema_generator = SchemaGenerator::default();

        RequestForm::<TestForm>::modify_api_operation(
            &mut operation,
            &route_context,
            &mut schema_generator,
        );

        if let Some(ReferenceOr::Item(request_body)) = &operation.request_body {
            let content = &request_body.content;
            assert!(content.contains_key("application/x-www-form-urlencoded"));
            let content_json = content.get("application/x-www-form-urlencoded").unwrap();
            let schema_obj = &content_json.schema.clone().unwrap().json_schema;

            if let Schema::Object(obj) = schema_obj {
                if let Some(properties) = &obj.object.as_ref().map(|o| &o.properties) {
                    assert!(properties.contains_key("field1"));
                    assert!(properties.contains_key("field2"));
                    assert!(properties.contains_key("optional_field"));
                } else {
                    panic!("Expected properties in schema");
                }
            } else {
                panic!("Expected object schema");
            }
        } else {
            panic!("Expected request body: {:?}", &operation.request_body);
        }
    }

    #[test]
    fn api_operation_part_for_path_single() {
        let mut operation = Operation::default();
        let mut route_context = RouteContext::new();
        route_context.param_names = &["id"];
        let mut schema_generator = SchemaGenerator::default();

        Path::<i32>::modify_api_operation(&mut operation, &route_context, &mut schema_generator);

        assert_eq!(operation.parameters.len(), 1);
        if let ReferenceOr::Item(Parameter::Path { parameter_data, .. }) = &operation.parameters[0]
        {
            assert_eq!(parameter_data.name, "id");
            assert!(parameter_data.required);
        } else {
            panic!("Expected path parameter");
        }
    }

    #[test]
    fn api_operation_part_for_path_tuple() {
        let mut operation = Operation::default();
        let mut route_context = RouteContext::new();
        route_context.param_names = &["id", "id2"];
        let mut schema_generator = SchemaGenerator::default();

        Path::<(i32, i32)>::modify_api_operation(
            &mut operation,
            &route_context,
            &mut schema_generator,
        );

        assert_eq!(operation.parameters.len(), 2);

        if let ReferenceOr::Item(Parameter::Path { parameter_data, .. }) = &operation.parameters[0]
        {
            assert_eq!(parameter_data.name, "id");
            assert!(parameter_data.required);
        } else {
            panic!("Expected path parameter");
        }

        if let ReferenceOr::Item(Parameter::Path { parameter_data, .. }) = &operation.parameters[1]
        {
            assert_eq!(parameter_data.name, "id2");
            assert!(parameter_data.required);
        } else {
            panic!("Expected path parameter");
        }
    }

    #[test]
    fn api_operation_part_for_path_object() {
        let mut operation = Operation::default();
        let mut route_context = RouteContext::new();
        route_context.param_names = &["field1", "field2"];
        let mut schema_generator = SchemaGenerator::default();

        Path::<TestPath>::modify_api_operation(
            &mut operation,
            &route_context,
            &mut schema_generator,
        );

        assert_eq!(operation.parameters.len(), 2);

        if let ReferenceOr::Item(Parameter::Path { parameter_data, .. }) = &operation.parameters[0]
        {
            assert_eq!(parameter_data.name, "field1");
            assert!(parameter_data.required);
        } else {
            panic!("Expected path parameter");
        }

        if let ReferenceOr::Item(Parameter::Path { parameter_data, .. }) = &operation.parameters[1]
        {
            assert_eq!(parameter_data.name, "field2");
            assert!(parameter_data.required);
        } else {
            panic!("Expected path parameter");
        }
    }

    #[test]
    #[should_panic(
        expected = "Path parameters in the route info must exactly match parameters in the Path"
    )]
    fn api_operation_part_for_path_object_invalid_route_info() {
        let mut operation = Operation::default();
        let route_context = RouteContext::new();
        let mut schema_generator = SchemaGenerator::default();

        Path::<TestPath>::modify_api_operation(
            &mut operation,
            &route_context,
            &mut schema_generator,
        );
    }

    #[test]
    fn test_api_operation_part_for_query() {
        let mut operation = Operation::default();
        let route_context = RouteContext::new();
        let mut schema_generator = SchemaGenerator::default();

        UrlQuery::<TestRequest>::modify_api_operation(
            &mut operation,
            &route_context,
            &mut schema_generator,
        );

        assert_eq!(operation.parameters.len(), 3); // field1, field2, optional_field

        for param in &operation.parameters {
            match param {
                ReferenceOr::Item(Parameter::Query { parameter_data, .. }) => {
                    assert!(
                        parameter_data.name == "field1"
                            || parameter_data.name == "field2"
                            || parameter_data.name == "optional_field"
                    );

                    if parameter_data.name == "optional_field" {
                        assert!(!parameter_data.required);
                    } else {
                        assert!(parameter_data.required);
                    }
                }
                _ => panic!("Expected query parameter"),
            }
        }
    }
}
