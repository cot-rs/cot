use cot::request::extractors::RequestForm;
use utoipa::ToSchema;
use utoipa::openapi::path::{Operation, Parameter, ParameterIn};
use utoipa::openapi::{RefOr, Schema};

use crate::auth::Auth;
use crate::form::Form;
use crate::request::Request;
use crate::request::extractors::{Json, Path, RequestDb, UrlQuery};
use crate::router::Urls;
use crate::session::Session;

trait ModifyOperation {
    #[allow(unused)]
    fn modify(&self, operation: &mut Operation) {}
}

impl<D: ToSchema> ModifyOperation for Request {}
impl<D: ToSchema> ModifyOperation for Urls {}
impl<D: ToSchema> ModifyOperation for Session {}
impl<D: ToSchema> ModifyOperation for Auth {}
#[cfg(feature = "db")]
impl<D: ToSchema> ModifyOperation for RequestDb {}

impl<D: ToSchema> ModifyOperation for Json<D> {
    fn modify(&self, operation: &mut Operation) {
        operation.request_body = Some(self.to_schema());
    }
}

impl<D: ToSchema> ModifyOperation for Path<D> {
    fn modify(&self, operation: &mut Operation) {
        operation.parameters.push(self.to_schema());
    }
}

impl<D: ToSchema> ModifyOperation for UrlQuery<D> {
    fn modify(&self, operation: &mut Operation) {
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
                            .schema(Some(property))
                            .build(),
                    );
                }
            }
            Schema::OneOf(_) => {}
            Schema::AllOf(_) => {}
            Schema::AnyOf(_) => {}
            _ => {}
        }
        operation.parameters.push(self.to_schema());
    }
}

impl<F: Form + ToSchema> ModifyOperation for RequestForm<F> {
    fn modify(&self, operation: &mut Operation) {}
}
