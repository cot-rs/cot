use cot::request::extractors::RequestForm;
use utoipa::ToSchema;
use utoipa::openapi::path::{Operation, Parameter, ParameterIn};
use utoipa::openapi::request_body::RequestBody;
use utoipa::openapi::{Content, RefOr, Schema};

use crate::auth::Auth;
use crate::form::Form;
use crate::request::Request;
use crate::request::extractors::{Json, Path, RequestDb, UrlQuery};
use crate::router::Urls;
use crate::session::Session;

pub trait AsOpenapiOperation<T = ()> {
    fn as_operation(&self) -> Operation;
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
            fn as_operation(&self) -> Operation {
                #[allow(unused_mut)] // for the case where there are no params
                let mut operation = Operation::new();

                $(
                    $ty::modify(&mut operation);
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
    fn modify(operation: &mut Operation) {}
}

impl ModifyOperation for Request {}
impl ModifyOperation for Urls {}
impl ModifyOperation for Session {}
impl ModifyOperation for Auth {}
#[cfg(feature = "db")]
impl ModifyOperation for RequestDb {}

impl<D: ToSchema> ModifyOperation for Json<D> {
    fn modify(operation: &mut Operation) {
        operation.request_body = Some(
            RequestBody::builder()
                .content(
                    crate::headers::JSON_CONTENT_TYPE,
                    Content::builder().schema(Some(D::schema())).build(),
                )
                .build(),
        );
    }
}

impl<D: ToSchema> ModifyOperation for Path<D> {
    fn modify(operation: &mut Operation) {
        let schema = match D::schema() {
            RefOr::Ref(_) => {
                panic!("ref schema is unsupported")
            }
            RefOr::T(schema) => schema,
        };

        // operation.parameters.push(self.to_schema());
    }
}

impl<D: ToSchema> ModifyOperation for UrlQuery<D> {
    fn modify(operation: &mut Operation) {
        let schema = match D::schema() {
            RefOr::Ref(_) => {
                panic!("ref schema is unsupported")
            }
            RefOr::T(schema) => schema,
        };

        match schema {
            Schema::Array(_) => {}
            Schema::Object(object) => {
                for (name, property) in object.properties.iter() {
                    let parameters = operation.parameters.get_or_insert_default();

                    parameters.push(
                        Parameter::builder()
                            .name(name)
                            .parameter_in(ParameterIn::Query)
                            .schema(Some(property.clone()))
                            .build(),
                    );
                }
            }
            Schema::OneOf(_) => {}
            Schema::AllOf(_) => {}
            Schema::AnyOf(_) => {}
            _ => {}
        }
        // operation.parameters.push(self.to_schema());
    }
}

impl<F: Form + ToSchema> ModifyOperation for RequestForm<F> {
    fn modify(operation: &mut Operation) {}
}
