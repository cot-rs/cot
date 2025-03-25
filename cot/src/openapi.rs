use std::collections::BTreeSet;

use aide::openapi::{
    MediaType, Operation, Parameter, ParameterData, ParameterSchemaOrContent, ReferenceOr,
    RequestBody,
};
use indexmap::IndexMap;
use schemars::schema::{InstanceType, Schema, SchemaObject, SingleOrVec};
use schemars::{JsonSchema, SchemaGenerator};

use crate::Method;
use crate::auth::Auth;
use crate::form::Form;
use crate::request::Request;
use crate::request::extractors::{Json, Path, RequestDb, RequestForm, UrlQuery};
use crate::router::{Route, Urls};
use crate::session::Session;

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct RouteInfo<'a> {
    pub method: Option<Method>,
    pub param_names: &'a [&'a str],
}

impl<'a> RouteInfo<'a> {
    pub fn new() -> Self {
        Self {
            method: None,
            param_names: &[],
        }
    }
}

impl<'a> Default for RouteInfo<'a> {
    fn default() -> Self {
        Self::new()
    }
}

pub trait AsOpenapiOperation<T = ()> {
    fn as_operation(&self, route_info: &RouteInfo<'_>) -> Operation;
}

macro_rules! impl_as_openapi_operation {
    ($($ty:ident),*) => {
        impl<T, $($ty,)* R> AsOpenapiOperation<($($ty,)*)> for T
        where
            T: Fn($($ty,)*) -> R + Clone + Send + Sync + 'static,
            $($ty: ModifyOperation,)*
            R: for<'a> Future<Output = cot::Result<cot::response::Response>> + Send,
        {
            #[allow(non_snake_case)]
            fn as_operation(&self, route_info: &RouteInfo<'_>) -> Operation {
                #[allow(unused_mut)] // for the case where there are no params
                let mut operation = Operation::default();
                #[allow(unused)] // for the case where there are no params
                let mut schema_generator = SchemaGenerator::default();

                $(
                    $ty::modify(&mut operation, &route_info, &mut schema_generator);
                )*

                operation
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
        route_info: &RouteInfo<'_>,
        schema_generator: &mut SchemaGenerator,
    ) {
    }
}

impl ModifyOperation for Request {}
impl ModifyOperation for Urls {}
impl ModifyOperation for Session {}
impl ModifyOperation for Auth {}
#[cfg(feature = "db")]
impl ModifyOperation for RequestDb {}

impl<D: JsonSchema> ModifyOperation for Json<D> {
    fn modify(
        operation: &mut Operation,
        _route_info: &RouteInfo<'_>,
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
        route_info: &RouteInfo<'_>,
        schema_generator: &mut SchemaGenerator,
    ) {
        let schema = D::json_schema(schema_generator).into_object();

        if let Some(array) = schema.array {
            if let Some(items) = array.items {
                match items {
                    SingleOrVec::Single(_) => {}
                    SingleOrVec::Vec(item_list) => {
                        assert_eq!(
                            route_info.param_names.len(),
                            item_list.len(),
                            "the number of path in the route info must match \
                            the number of params in the Path type"
                        );

                        for (&param_name, item) in
                            route_info.param_names.iter().zip(item_list.into_iter())
                        {
                            let schema = item.into_object();

                            operation
                                .parameters
                                .push(ReferenceOr::Item(Parameter::Path {
                                    parameter_data: param_with_name(
                                        param_name.to_owned(),
                                        schema,
                                        true,
                                    ),
                                    style: Default::default(),
                                }));
                        }
                    }
                }
            }
        }

        if let Some(object) = schema.object {
            // todo check that the params are unique in the router
            let route_info_set = BTreeSet::from_iter(route_info.param_names.iter().map(|x| *x));
            let object_set = BTreeSet::from_iter(object.properties.keys().map(|x| x.as_str()));

            assert_eq!(
                route_info_set, object_set,
                "Path parameters in the route info must exactly match parameters \
                in the Path type. Make sure that the type you pass to Path contains \
                all the parameters for the route, and that the names match exactly."
            );

            for (key, item) in object.properties {
                let mut object_item = item.into_object();

                let required = match &mut object_item.instance_type {
                    Some(SingleOrVec::Vec(type_list)) => {
                        let nullable = type_list.contains(&InstanceType::Null);
                        type_list.retain(|&element| element != InstanceType::Null);
                        !nullable
                    }
                    Some(SingleOrVec::Single(_)) => true,
                    None => true,
                };

                operation
                    .parameters
                    .push(ReferenceOr::Item(Parameter::Path {
                        parameter_data: param_with_name(key, object_item, required),
                        style: Default::default(),
                    }));
            }
        }
    }
}

impl<D: JsonSchema> ModifyOperation for UrlQuery<D> {
    fn modify(
        operation: &mut Operation,
        _route_info: &RouteInfo<'_>,
        schema_generator: &mut SchemaGenerator,
    ) {
        let schema = D::json_schema(schema_generator).into_object();

        if let Some(object) = schema.object {
            for (key, item) in object.properties {
                let mut object_item = item.into_object();

                let required = match &mut object_item.instance_type {
                    Some(SingleOrVec::Vec(type_list)) => {
                        let nullable = type_list.contains(&InstanceType::Null);
                        type_list.retain(|&element| element != InstanceType::Null);
                        !nullable
                    }
                    Some(SingleOrVec::Single(_)) => true,
                    None => true,
                };

                operation
                    .parameters
                    .push(ReferenceOr::Item(Parameter::Query {
                        parameter_data: param_with_name(key, object_item, required),
                        allow_reserved: false,
                        style: Default::default(),
                        allow_empty_value: None,
                    }));
            }
        }
    }
}

fn param_with_name(
    param_name: String,
    schema_object: SchemaObject,
    required: bool,
) -> ParameterData {
    ParameterData {
        name: param_name.to_owned(),
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
        _route_info: &RouteInfo<'_>,
        schema_generator: &mut SchemaGenerator,
    ) {
        todo!()
    }
}
