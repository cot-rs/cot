//! OpenAPI integration for Cot Core.
//!
//! This module provides core traits and utilities for OpenAPI integration.
//! It contains the minimal types needed by the router to support OpenAPI.
//! Higher-level OpenAPI functionality is implemented in the main `cot` crate.

pub mod method;

use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;

use aide::openapi::{Operation, PathItem, StatusCode};
use aide::openapi::{
    MediaType, Parameter, ParameterData, ParameterSchemaOrContent, PathStyle,
    QueryStyle, ReferenceOr, RequestBody,
};
use indexmap::IndexMap;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde_json::Value;

use crate::Method;
use crate::handler::{BoxRequestHandler, RequestHandler};
use crate::request::Request;
use crate::request::extractors::{Path, UrlQuery};
use crate::response::{Response, WithExtension};
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
///     async fn from_request(head: &cot::request::RequestHead, body: cot::Body) -> cot::Result<Self> {
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
    #[expect(unused)]
    fn modify_api_operation(
        operation: &mut Operation,
        route_context: &RouteContext<'_>,
        schema_generator: &mut SchemaGenerator,
    ) {
    }
}

/// A trait that generates OpenAPI response objects for handler return types.
///
/// This trait is implemented for types that can be returned from request
/// handlers and need to be documented in the OpenAPI specification. It allows
/// you to specify how a type should be represented in the OpenAPI
/// documentation.
///
/// # Examples
///
/// ```
/// use cot::aide::openapi::{MediaType, Operation, Response, StatusCode};
/// use cot::openapi::{ApiOperationResponse, RouteContext};
/// use indexmap::IndexMap;
/// use schemars::SchemaGenerator;
///
/// // A custom response type
/// struct MyResponse<T>(T);
///
/// impl<T: schemars::JsonSchema> ApiOperationResponse for MyResponse<T> {
///     fn api_operation_responses(
///         _operation: &mut Operation,
///         _route_context: &RouteContext<'_>,
///         schema_generator: &mut SchemaGenerator,
///     ) -> Vec<(Option<StatusCode>, Response)> {
///         vec![(
///             Some(StatusCode::Code(201)),
///             Response {
///                 description: "Created".to_string(),
///                 content: IndexMap::from([(
///                     "application/json".to_string(),
///                     MediaType {
///                         schema: Some(aide::openapi::SchemaObject {
///                             json_schema: T::json_schema(schema_generator),
///                             external_docs: None,
///                             example: None,
///                         }),
///                         ..Default::default()
///                     },
///                 )]),
///                 ..Default::default()
///             },
///         )]
///     }
/// }
/// ```
pub trait ApiOperationResponse {
    /// Returns a list of OpenAPI response objects for this type.
    ///
    /// This method is called by the framework when generating the OpenAPI
    /// specification for a route. It should return a list of responses
    /// that this type can produce, along with their status codes.
    ///
    /// The status code can be `None` to indicate a default response.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::aide::openapi::{MediaType, Operation, Response, StatusCode};
    /// use cot::openapi::{ApiOperationResponse, RouteContext};
    /// use indexmap::IndexMap;
    /// use schemars::SchemaGenerator;
    ///
    /// // A custom response type that always returns 201 Created
    /// struct CreatedResponse<T>(T);
    ///
    /// impl<T: schemars::JsonSchema> ApiOperationResponse for CreatedResponse<T> {
    ///     fn api_operation_responses(
    ///         _operation: &mut Operation,
    ///         _route_context: &RouteContext<'_>,
    ///         schema_generator: &mut SchemaGenerator,
    ///     ) -> Vec<(Option<StatusCode>, Response)> {
    ///         vec![(
    ///             Some(StatusCode::Code(201)),
    ///             Response {
    ///                 description: "Created".to_string(),
    ///                 content: IndexMap::from([(
    ///                     "application/json".to_string(),
    ///                     MediaType {
    ///                         schema: Some(aide::openapi::SchemaObject {
    ///                             json_schema: T::json_schema(schema_generator),
    ///                             external_docs: None,
    ///                             example: None,
    ///                         }),
    ///                         ..Default::default()
    ///                     },
    ///                 )]),
    ///                 ..Default::default()
    ///             },
    ///         )]
    ///     }
    /// }
    /// ```
    #[expect(unused)]
    fn api_operation_responses(
        operation: &mut Operation,
        route_context: &RouteContext<'_>,
        schema_generator: &mut SchemaGenerator,
    ) -> Vec<(Option<StatusCode>, aide::openapi::Response)> {
        Vec::new()
    }
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

#[macro_export]
macro_rules! impl_as_openapi_operation {
    ($($ty:ident),*) => {
        impl<T, $($ty,)* R, Response> AsApiOperation<($($ty,)*)> for T
        where
            T: Fn($($ty,)*) -> R + Clone + Send + Sync + 'static,
            $($ty: ApiOperationPart,)*
            R: for<'a> Future<Output = Response> + Send,
            Response: ApiOperationResponse,
        {
            #[allow(
                clippy::allow_attributes,
                non_snake_case,
                reason = "for the case where there are no FromRequestHead params"
            )]
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
                let responses = Response::api_operation_responses(
                    &mut operation,
                    &route_context,
                    schema_generator
                );
                let operation_responses = operation.responses.get_or_insert_default();
                for (response_code, response) in responses {
                    if let Some(response_code) = response_code {
                        operation_responses.responses.insert(
                            response_code,
                            ReferenceOr::Item(response),
                        );
                    } else {
                        operation_responses.default = Some(ReferenceOr::Item(response));
                    }
                }

                Some(operation)
            }
        }
    };
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

handle_all_parameters!(impl_as_openapi_operation);

impl ApiOperationPart for Request {}
impl ApiOperationPart for Method {}
impl<D: JsonSchema> ApiOperationPart for Path<D> {
    #[track_caller]
    fn modify_api_operation(
        operation: &mut Operation,
        route_context: &RouteContext<'_>,
        schema_generator: &mut SchemaGenerator,
    ) {
        let mut schema = D::json_schema(schema_generator);
        let schema_obj = schema.ensure_object();

        if let Some(items) = schema_obj.get("prefixItems") {
            // a tuple of path params, e.g. Path<(i32, String)>

            if let Value::Array(item_list) = items {
                assert_eq!(
                    route_context.param_names.len(),
                    item_list.len(),
                    "the number of path parameters in the route URL must match \
                    the number of params in the Path type (found path params: {:?})",
                    route_context.param_names,
                );

                for (&param_name, item) in route_context.param_names.iter().zip(item_list.iter()) {
                    let array_item = Schema::try_from(item.clone())
                        .expect("schema.items must contain valid schemas");

                    add_path_param(operation, array_item, param_name.to_owned());
                }
            }
        } else if let Some(properties) = schema_obj.get("properties") {
            // a struct of path params, e.g. Path<MyStruct>

            if let Value::Object(properties) = properties {
                let mut route_context_sorted = route_context.param_names.to_vec();
                route_context_sorted.sort_unstable();
                let mut object_props_sorted = properties.keys().collect::<Vec<_>>();
                object_props_sorted.sort();

                assert_eq!(
                    route_context_sorted, object_props_sorted,
                    "Path parameters in the route info must exactly match parameters \
                    in the Path type. Make sure that the type you pass to Path contains \
                    all the parameters for the route, and that the names match exactly."
                );

                for (key, item) in properties {
                    let object_item = Schema::try_from(item.clone())
                        .expect("schema.properties must contain valid schemas");

                    add_path_param(operation, object_item, key.clone());
                }
            }
        } else if schema_obj.contains_key("type") {
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
        let schema = D::json_schema(schema_generator);

        if let Some(Value::Object(properties)) = schema.get("properties") {
            for (key, item) in properties {
                let object_item = Schema::try_from(item.clone())
                    .expect("schema.properties must contain valid schemas");

                add_query_param(operation, object_item, key.clone());
            }
        }
    }
}
impl<T: ApiOperationResponse, D> ApiOperationResponse for WithExtension<T, D> {
    fn api_operation_responses(
        operation: &mut Operation,
        route_context: &RouteContext<'_>,
        schema_generator: &mut SchemaGenerator,
    ) -> Vec<(Option<StatusCode>, aide::openapi::Response)> {
        T::api_operation_responses(operation, route_context, schema_generator)
    }
}

impl ApiOperationResponse for crate::Result<Response> {
    fn api_operation_responses(
        _operation: &mut Operation,
        _route_context: &RouteContext<'_>,
        _schema_generator: &mut SchemaGenerator,
    ) -> Vec<(Option<StatusCode>, aide::openapi::Response)> {
        vec![(
            None,
            aide::openapi::Response {
                description: "*&lt;unspecified&gt;*".to_string(),
                ..Default::default()
            },
        )]
    }
}

// we don't require `E: ApiOperationResponse` here because a global error
// handler will typically take care of generating OpenAPI responses for errors
//
// we might want to add a version for `E: ApiOperationResponse` when (if ever)
// specialization lands in Rust: https://github.com/rust-lang/rust/issues/31844
impl<T, E> ApiOperationResponse for Result<T, E>
where
    T: ApiOperationResponse,
{
    fn api_operation_responses(
        operation: &mut Operation,
        route_context: &RouteContext<'_>,
        schema_generator: &mut SchemaGenerator,
    ) -> Vec<(Option<StatusCode>, aide::openapi::Response)> {
        let mut responses = Vec::new();

        let ok_response = T::api_operation_responses(operation, route_context, schema_generator);
        for (status_code, response) in ok_response {
            responses.push((status_code, response));
        }

        responses
    }
}

fn add_path_param(operation: &mut Operation, mut schema: Schema, param_name: String) {
    let required = extract_is_required(&mut schema);

    operation
        .parameters
        .push(ReferenceOr::Item(Parameter::Path {
            parameter_data: param_with_name(param_name, schema, required),
            style: PathStyle::default(),
        }));
}

// TODO: remove pub
pub fn add_query_param(operation: &mut Operation, mut schema: Schema, param_name: String) {
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

fn extract_is_required(object_item: &mut Schema) -> bool {
    let object = object_item.ensure_object();
    let obj_type = object.get_mut("type");
    let null_value = Value::String("null".to_string());

    if let Some(Value::Array(types)) = obj_type {
        if types.contains(&null_value) {
            // If the type is nullable, we need to remove "null" from the types
            // and return false, indicating that the parameter is not required.
            types.retain(|t| t != &null_value);
            false
        } else {
            // If "null" is not in the types, we assume it's a required parameter
            true
        }
    } else {
        // If the type is a single string (or some other unknown value), we assume it's
        // a required parameter
        true
    }
}


fn param_with_name(param_name: String, schema: Schema, required: bool) -> ParameterData {
    ParameterData {
        name: param_name,
        description: None,
        required,
        deprecated: None,
        format: ParameterSchemaOrContent::Schema(aide::openapi::SchemaObject {
            json_schema: schema,
            external_docs: None,
            example: None,
        }),
        example: None,
        examples: IndexMap::default(),
        explode: None,
        extensions: IndexMap::default(),
    }
}