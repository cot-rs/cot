#[cfg(feature = "swagger-ui")]
pub mod swagger_ui;

use std::marker::PhantomData;
use std::pin::Pin;

use aide::openapi::{
    MediaType, Operation, Parameter, ParameterData, ParameterSchemaOrContent, PathItem,
    ReferenceOr, RequestBody,
};
use cot::RequestHandler;
use cot::handler::BoxRequestHandler;
use cot::response::Response;
use indexmap::IndexMap;
use schemars::schema::{InstanceType, Schema, SchemaObject, SingleOrVec};
use schemars::{JsonSchema, SchemaGenerator};

use crate::Method;
use crate::auth::Auth;
use crate::form::Form;
use crate::request::Request;
use crate::request::extractors::{Json, Path, RequestDb, RequestForm, UrlQuery};
use crate::router::Urls;
use crate::session::Session;

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct RouteContext<'a> {
    pub param_names: &'a [&'a str],
}

impl RouteContext<'_> {
    #[must_use]
    pub fn new() -> Self {
        Self { param_names: &[] }
    }
}

impl Default for RouteContext<'_> {
    fn default() -> Self {
        Self::new()
    }
}

pub trait AsPathItem {
    fn as_path_item(&self, route_context: &RouteContext<'_>) -> PathItem;
}

pub trait AsOpenapiOperation<T = ()> {
    fn as_operation(&self, route_context: &RouteContext<'_>) -> Option<Operation>;
}

pub(crate) trait BoxApiRequestHandler: BoxRequestHandler + AsOpenapiOperation {}

pub(crate) fn into_box_api_request_handler<HandlerParams, ApiParams, H>(
    handler: H,
) -> impl BoxApiRequestHandler
where
    H: RequestHandler<HandlerParams> + AsOpenapiOperation<ApiParams> + Send + Sync,
{
    struct Inner<HandlerParams, ApiParams, H>(
        H,
        PhantomData<fn() -> HandlerParams>,
        PhantomData<fn() -> ApiParams>,
    );

    impl<HandlerParams, ApiParams, H> BoxRequestHandler for Inner<HandlerParams, ApiParams, H>
    where
        H: RequestHandler<HandlerParams> + AsOpenapiOperation<ApiParams> + Send + Sync,
    {
        fn handle(
            &self,
            request: Request,
        ) -> Pin<Box<dyn Future<Output = cot::Result<Response>> + Send + '_>> {
            Box::pin(self.0.handle(request))
        }
    }

    impl<HandlerParams, ApiParams, H> AsOpenapiOperation for Inner<HandlerParams, ApiParams, H>
    where
        H: RequestHandler<HandlerParams> + AsOpenapiOperation<ApiParams> + Send + Sync,
    {
        fn as_operation(&self, route_context: &RouteContext<'_>) -> Option<Operation> {
            self.0.as_operation(route_context)
        }
    }

    impl<HandlerParams, ApiParams, H> BoxApiRequestHandler for Inner<HandlerParams, ApiParams, H> where
        H: RequestHandler<HandlerParams> + AsOpenapiOperation<ApiParams> + Send + Sync
    {
    }

    Inner(handler, PhantomData, PhantomData)
}

pub(crate) trait BoxApiEndpointRequestHandler: BoxRequestHandler + AsPathItem {}

pub(crate) fn into_box_api_endpoint_request_handler<HandlerParams, H>(
    handler: H,
) -> impl BoxApiEndpointRequestHandler
where
    H: RequestHandler<HandlerParams> + AsPathItem + Send + Sync,
{
    struct Inner<HandlerParams, H>(H, PhantomData<fn() -> HandlerParams>);

    impl<HandlerParams, H> BoxRequestHandler for Inner<HandlerParams, H>
    where
        H: RequestHandler<HandlerParams> + AsPathItem + Send + Sync,
    {
        fn handle(
            &self,
            request: Request,
        ) -> Pin<Box<dyn Future<Output = cot::Result<Response>> + Send + '_>> {
            Box::pin(self.0.handle(request))
        }
    }

    impl<HandlerParams, H> AsPathItem for Inner<HandlerParams, H>
    where
        H: RequestHandler<HandlerParams> + AsPathItem + Send + Sync,
    {
        fn as_path_item(&self, route_context: &RouteContext<'_>) -> PathItem {
            self.0.as_path_item(route_context)
        }
    }

    impl<HandlerParams, H> BoxApiEndpointRequestHandler for Inner<HandlerParams, H> where
        H: RequestHandler<HandlerParams> + AsPathItem + Send + Sync
    {
    }

    Inner(handler, PhantomData)
}

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

impl<T> ModifyOperation for NoApi<T> {}

impl<T> AsOpenapiOperation for NoApi<T> {
    fn as_operation(&self, _route_context: &RouteContext<'_>) -> Option<Operation> {
        None
    }
}

macro_rules! impl_as_openapi_operation {
    ($($ty:ident),*) => {
        impl<T, $($ty,)* R, Response> AsOpenapiOperation<($($ty,)*)> for T
        where
            T: Fn($($ty,)*) -> R + Clone + Send + Sync + 'static,
            $($ty: ModifyOperation,)*
            R: for<'a> Future<Output = Response> + Send,
            Response: ModifyOperation,
        {
            #[allow(non_snake_case)]
            fn as_operation(&self, route_context: &RouteContext<'_>) -> Option<Operation> {
                #[allow(unused_mut)] // for the case where there are no params
                let mut operation = Operation::default();
                #[allow(unused)] // for the case where there are no params
                let mut schema_generator = SchemaGenerator::default();

                $(
                    $ty::modify(&mut operation, &route_context, &mut schema_generator);
                )*
                Response::modify(&mut operation, &route_context, &mut schema_generator);

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

pub trait ModifyOperation {
    #[allow(unused)]
    fn modify(
        operation: &mut Operation,
        route_context: &RouteContext<'_>,
        schema_generator: &mut SchemaGenerator,
    ) {
    }
}

impl ModifyOperation for Request {}
impl ModifyOperation for Urls {}
impl ModifyOperation for Method {}
impl ModifyOperation for Session {}
impl ModifyOperation for Auth {}
#[cfg(feature = "db")]
impl ModifyOperation for RequestDb {}

impl<D: JsonSchema> ModifyOperation for Json<D> {
    fn modify(
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

impl<D: JsonSchema> ModifyOperation for Path<D> {
    fn modify(
        operation: &mut Operation,
        route_context: &RouteContext<'_>,
        schema_generator: &mut SchemaGenerator,
    ) {
        let schema = D::json_schema(schema_generator).into_object();

        if schema.instance_type.is_some() {
            // single path param, e.g. Path<i32>

            assert_eq!(
                route_context.param_names.len(),
                1,
                "the number of path parameters in the route URL must equal \
                to 1 if a single parameter was passed to the Path type (found path params: {:?})",
                route_context.param_names,
            );

            add_path_param(operation, schema, route_context.param_names[0].to_owned());
        } else if let Some(array) = schema.array {
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
        }
    }
}

impl<D: JsonSchema> ModifyOperation for UrlQuery<D> {
    fn modify(
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

fn add_path_param(operation: &mut Operation, mut schema: SchemaObject, param_name: String) {
    let required = extract_is_required(&mut schema);

    operation
        .parameters
        .push(ReferenceOr::Item(Parameter::Path {
            parameter_data: param_with_name(param_name, schema, required),
            style: Default::default(),
        }));
}

fn add_query_param(operation: &mut Operation, mut schema: SchemaObject, param_name: String) {
    let required = extract_is_required(&mut schema);

    operation
        .parameters
        .push(ReferenceOr::Item(Parameter::Query {
            parameter_data: param_with_name(param_name, schema, required),
            allow_reserved: false,
            style: Default::default(),
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
        Some(SingleOrVec::Single(_)) => true,
        None => true,
    }
}

fn param_with_name(
    param_name: String,
    schema_object: SchemaObject,
    required: bool,
) -> ParameterData {
    ParameterData {
        name: param_name.clone(),
        description: None,
        required,
        deprecated: None,
        format: ParameterSchemaOrContent::Schema(aide::openapi::SchemaObject {
            json_schema: Schema::Object(schema_object),
            external_docs: None,
            example: None,
        }),
        example: None,
        examples: Default::default(),
        explode: None,
        extensions: Default::default(),
    }
}

impl<F: Form + JsonSchema> ModifyOperation for RequestForm<F> {
    fn modify(
        operation: &mut Operation,
        _route_context: &RouteContext<'_>,
        schema_generator: &mut SchemaGenerator,
    ) {
        todo!()
    }
}

impl ModifyOperation for crate::Result<Response> {
    fn modify(
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
