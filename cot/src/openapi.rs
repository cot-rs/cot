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
//! 2. Use [`ApiMethodRouter`](cot_core::router::method::method::ApiMethodRouter)
//!    to set up your API routes and register them with a router (possibly using
//!    convenience functions, such as
//!    [`api_get`](cot_core::router::method::method::api_get) or
//!    [`api_post`](cot_core::router::method::method::api_post)).
//! 3. Register your
//!    [`ApiMethodRouter`](cot_core::router::method::method::ApiMethodRouter)s
//!    with a [`Router`](cot_core::router::Router) using
//!    [`Route::with_api_handler`](cot_core::router::Route::with_api_handler) or
//!    [`Route::with_api_handler_and_name`](cot_core::router::Route::with_api_handler_and_name).
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
//! use cot::json::Json;
//! use cot::openapi::swagger_ui::SwaggerUi;
//! use cot::project::{MiddlewareContext, RegisterAppsContext, RootHandler, RootHandlerBuilder};
//! use cot::response::{Response, ResponseExt};
//! use cot::static_files::StaticFilesMiddleware;
//! use cot::{App, AppBuilder, Project, StatusCode};
//! use cot_core::router::method::method::api_post;
//! use cot_core::router::{Route, Router};
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
//! async fn add(Json(add_request): Json<AddRequest>) -> Json<AddResponse> {
//!     Json(AddResponse {
//!         result: add_request.a + add_request.b,
//!     })
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
//!     ) -> RootHandler {
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

use aide::openapi::{
    MediaType, Operation, Parameter, ParameterData, ParameterSchemaOrContent, PathStyle,
    QueryStyle, ReferenceOr, RequestBody, StatusCode,
};
use cot::router::Urls;
use cot_core::handle_all_parameters;
use cot_core::handler::BoxRequestHandler;
use cot_core::impl_as_openapi_operation;
use cot_core::openapi::add_query_param;
#[doc(inline)]
pub use cot_core::openapi::{RouteContext, AsApiRoute, BoxApiEndpointRequestHandler, AsApiOperation, into_box_api_endpoint_request_handler, method, ApiOperationPart, ApiOperationResponse};
use cot_core::request::extractors::{Path, UrlQuery};
use cot_core::response::{Response, WithExtension};
/// Derive macro for the [`ApiOperationResponse`] trait.
///
/// This macro can be applied to enums to automatically implement the
/// [`ApiOperationResponse`] trait for OpenAPI documentation generation.
/// The enum must consist of tuple variants with exactly one field each,
/// where each field type implements [`ApiOperationResponse`].
///
/// **Note**: This macro only implements [`ApiOperationResponse`]. If you also
/// need [`IntoResponse`], you must derive it separately or implement it
/// manually.
///
/// # Requirements
///
/// - **Only enums are supported**: This macro will produce a compile error if
///   applied to structs or unions.
/// - **Tuple variants with one field**: Each enum variant must be a tuple
///   variant with exactly one field (e.g., `Variant(Type)`).
/// - **Field types must implement `ApiOperationResponse`**: Each field type
///   must implement the [`ApiOperationResponse`] trait.
///
/// # Generated Implementation
///
/// The macro generates an implementation that aggregates OpenAPI responses
/// from all the wrapped types:
///
/// ```compile_fail
/// impl ApiOperationResponse for MyEnum {
///     fn api_operation_responses(
///         operation: &mut Operation,
///         route_context: &RouteContext<'_>,
///         schema_generator: &mut SchemaGenerator,
///     ) -> Vec<(Option<StatusCode>, Response)> {
///         let mut responses = Vec::new();
///         responses.extend(Type1::api_operation_responses(operation, route_context, schema_generator));
///         responses.extend(Type2::api_operation_responses(operation, route_context, schema_generator));
///         // ... for each variant type
///         responses
///     }
/// }
/// ```
///
/// # Examples
///
/// Basic usage (you'll also need to implement or derive [`IntoResponse`]):
///
/// ```
/// use cot::json::Json;
/// use cot::openapi::ApiOperationResponse;
/// use cot::response::IntoResponse;
///
/// #[derive(IntoResponse, ApiOperationResponse)]
/// enum MyResponse {
///     Success(Json<String>),
///     Error(Json<ErrorResponse>),
/// }
///
/// #[derive(serde::Serialize, schemars::JsonSchema)]
/// struct ErrorResponse {
///     message: String,
/// }
/// ```
///
/// # Relationship with [`IntoResponse`]
///
/// This derive macro **only** implements [`ApiOperationResponse`]. If you need
/// both traits (which is common for response enums), you should derive both (or
/// implement [`IntoResponse`] manually).
///
/// ```
/// use cot::json::Json;
/// use cot::openapi::ApiOperationResponse;
/// use cot::response::IntoResponse;
///
/// #[derive(IntoResponse, ApiOperationResponse)]
/// enum MyResponse {
///     Success(Json<String>),
///     Error(Json<ErrorResponse>),
/// }
///
/// # #[derive(serde::Serialize, schemars::JsonSchema)]
/// # struct ErrorResponse {
/// #     message: String,
/// # }
/// ```
///
/// [`ApiOperationResponse`]: crate::openapi::ApiOperationResponse
/// [`IntoResponse`]: crate::response::IntoResponse
pub use cot_macros::ApiOperationResponse;
use indexmap::IndexMap;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde_json::Value;

use crate::auth::Auth;
use crate::form::Form;
use crate::json::Json;
use crate::request::extractors::{FromRequest, FromRequestHead, RequestForm};
use crate::request::{Request, RequestHead};
use crate::session::Session;
use crate::{Body, Method, RequestHandler};

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
/// use cot::request::RequestHead;
/// use cot::request::extractors::FromRequestHead;
/// use cot::response::Response;
/// use cot_core::router::Route;
/// use cot_core::router::method::method::api_get;
///
/// struct MyExtractor;
/// impl FromRequestHead for MyExtractor {
///     async fn from_request_head(head: &RequestHead) -> cot::Result<Self> {
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
/// let router = cot_core::router::Router::with_urls([Route::with_api_handler(
///     "/with_api",
///     api_get(handler),
/// )]);
/// ```
///
/// ```
/// use cot::openapi::NoApi;
/// use cot::response::Response;
/// use cot_core::router::Route;
/// use cot_core::router::method::method::api_get;
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
/// let router = cot_core::router::Router::with_urls([Route::with_api_handler(
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
    async fn from_request(head: &RequestHead, body: Body) -> cot::Result<Self> {
        T::from_request(head, body).await.map(Self)
    }
}

impl<T: FromRequestHead> FromRequestHead for NoApi<T> {
    async fn from_request_head(head: &RequestHead) -> cot::Result<Self> {
        T::from_request_head(head).await.map(Self)
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


impl ApiOperationPart for Urls {}
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


impl<F: Form + JsonSchema> ApiOperationPart for RequestForm<F> {
    fn modify_api_operation(
        operation: &mut Operation,
        route_context: &RouteContext<'_>,
        schema_generator: &mut SchemaGenerator,
    ) {
        if route_context.method == Some(Method::GET) || route_context.method == Some(Method::HEAD) {
            let schema = F::json_schema(schema_generator);

            if let Some(Value::Object(properties)) = schema.get("properties") {
                for (key, item) in properties {
                    let object_item = Schema::try_from(item.clone())
                        .expect("schema.properties must contain valid schemas");

                    add_query_param(operation, object_item, key.clone());
                }
            }
        } else {
            operation.request_body = Some(ReferenceOr::Item(RequestBody {
                content: IndexMap::from([(
                    crate::headers::URLENCODED_FORM_CONTENT_TYPE.to_string(),
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


impl<S: JsonSchema> ApiOperationResponse for Json<S> {
    fn api_operation_responses(
        _operation: &mut Operation,
        _route_context: &RouteContext<'_>,
        schema_generator: &mut SchemaGenerator,
    ) -> Vec<(Option<StatusCode>, aide::openapi::Response)> {
        vec![(
            Some(StatusCode::Code(http::StatusCode::OK.as_u16())),
            aide::openapi::Response {
                description: "OK".to_string(),
                content: IndexMap::from([(
                    crate::headers::JSON_CONTENT_TYPE.to_string(),
                    MediaType {
                        schema: Some(aide::openapi::SchemaObject {
                            json_schema: S::json_schema(schema_generator),
                            external_docs: None,
                            example: None,
                        }),
                        ..Default::default()
                    },
                )]),
                ..Default::default()
            },
        )]
    }
}

#[cfg(test)]
mod tests {
    use aide::openapi::{Operation, Parameter};
    use cot_core::html::Html;
    use schemars::SchemaGenerator;
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::json::Json;
    use crate::openapi::AsApiOperation;

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

    async fn test_handler() -> Html {
        Html::new("test")
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

            if let Some(obj) = schema_obj.as_object() {
                if let Some(Value::Object(properties)) = obj.get("properties") {
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

            if let Some(obj) = schema_obj.as_object() {
                if let Some(Value::Object(properties)) = &obj.get("properties") {
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
    fn api_operation_part_for_query() {
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

    #[test]
    fn api_operation_response_for_json() {
        let mut operation = Operation::default();
        let route_context = RouteContext::new();
        let mut schema_generator = SchemaGenerator::default();

        let responses = Json::<TestRequest>::api_operation_responses(
            &mut operation,
            &route_context,
            &mut schema_generator,
        );

        assert_eq!(responses.len(), 1);
        let (status_code, response) = &responses[0];

        assert_eq!(status_code, &Some(StatusCode::Code(200)));
        assert_eq!(response.description, "OK");
        assert!(response.content.contains_key("application/json"));

        let content = response.content.get("application/json").unwrap();
        assert!(content.schema.is_some());

        let schema = &content.schema.as_ref().unwrap().json_schema;
        if let Some(obj) = schema.as_object() {
            if let Some(Value::Object(properties)) = &obj.get("properties") {
                assert!(properties.contains_key("field1"));
                assert!(properties.contains_key("field2"));
                assert!(properties.contains_key("optional_field"));
            } else {
                panic!("Expected properties in schema");
            }
        } else {
            panic!("Expected schema object");
        }
    }

    #[test]
    fn api_operation_response_for_with_extension() {
        let mut operation = Operation::default();
        let route_context = RouteContext::new();
        let mut schema_generator = SchemaGenerator::default();

        // WithExtension should delegate to the wrapped type's implementation
        let responses = WithExtension::<Json<TestRequest>, ()>::api_operation_responses(
            &mut operation,
            &route_context,
            &mut schema_generator,
        );

        assert_eq!(responses.len(), 1);
        let (status_code, _) = &responses[0];
        assert_eq!(status_code, &Some(StatusCode::Code(200)));
    }

    #[test]
    fn api_operation_response_for_result() {
        let mut operation = Operation::default();
        let route_context = RouteContext::new();
        let mut schema_generator = SchemaGenerator::default();

        let responses = <crate::Result<Response>>::api_operation_responses(
            &mut operation,
            &route_context,
            &mut schema_generator,
        );

        assert_eq!(responses.len(), 1);
        let (status_code, response) = &responses[0];

        assert_eq!(status_code, &None); // Default response
        assert_eq!(response.description, "*&lt;unspecified&gt;*");
        assert!(response.content.is_empty());
    }

    #[test]
    fn api_operation_response_for_result_with_json_success() {
        let mut operation = Operation::default();
        let route_context = RouteContext::new();
        let mut schema_generator = SchemaGenerator::default();

        let responses = <Result<Json<TestRequest>, ()>>::api_operation_responses(
            &mut operation,
            &route_context,
            &mut schema_generator,
        );

        assert_eq!(responses.len(), 1);
        let (status_code, response) = &responses[0];

        assert_eq!(status_code, &Some(StatusCode::Code(200)));
        assert_eq!(response.description, "OK");
        assert!(response.content.contains_key("application/json"));

        let content = response.content.get("application/json").unwrap();
        assert!(content.schema.is_some());
    }

    #[test]
    fn api_operation_response_for_result_with_multiple_responses() {
        #[derive(schemars::JsonSchema)]
        struct MultiResponse;

        impl ApiOperationResponse for MultiResponse {
            fn api_operation_responses(
                _operation: &mut Operation,
                _route_context: &RouteContext<'_>,
                _schema_generator: &mut SchemaGenerator,
            ) -> Vec<(Option<StatusCode>, aide::openapi::Response)> {
                vec![
                    (
                        Some(StatusCode::Code(200)),
                        aide::openapi::Response {
                            description: "Success".to_string(),
                            ..Default::default()
                        },
                    ),
                    (
                        Some(StatusCode::Code(400)),
                        aide::openapi::Response {
                            description: "Bad Request".to_string(),
                            ..Default::default()
                        },
                    ),
                ]
            }
        }

        let mut operation = Operation::default();
        let route_context = RouteContext::new();
        let mut schema_generator = SchemaGenerator::default();

        let responses = <Result<MultiResponse, ()>>::api_operation_responses(
            &mut operation,
            &route_context,
            &mut schema_generator,
        );

        assert_eq!(responses.len(), 2);

        let (status_code_1, response_1) = &responses[0];
        assert_eq!(status_code_1, &Some(StatusCode::Code(200)));
        assert_eq!(response_1.description, "Success");

        let (status_code_2, response_2) = &responses[1];
        assert_eq!(status_code_2, &Some(StatusCode::Code(400)));
        assert_eq!(response_2.description, "Bad Request");
    }
}
