use std::error::Error;
use std::sync::Arc;

use cot::cli::CliMetadata;
use cot::config::ProjectConfig;
use cot::project::RegisterAppsContext;
use cot::request::extractors::Json;
use cot::request::Request;
use cot::response::{Response, ResponseExt};
use cot::router::{Route, Router};
use cot::{App, AppBuilder, Body, Project, StatusCode};
use serde::{Deserialize, Serialize};
use utoipa::openapi::path::Operation;
use utoipa::openapi::request_body::RequestBody;
use utoipa::openapi::{Content, HttpMethod, OpenApiVersion, PathItem, Paths};
use utoipa::PartialSchema;
use utoipa_swagger_ui::{Config, SwaggerFile};

#[derive(Debug, Clone, Deserialize, utoipa::ToSchema)]
struct AddRequest {
    a: i32,
    b: i32,
}

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
struct AddResponse {
    result: i32,
}

async fn add(Json(add_request): Json<AddRequest>) -> cot::Result<Response> {
    let response = AddResponse {
        result: add_request.a + add_request.b,
    };

    Response::new_json(StatusCode::OK, &response)
}

async fn swagger_ui(request: Request) -> cot::Result<Response> {
    let config = utoipa_swagger_ui::Config::new(["/api-docs/openapi.json"]);
    let file_path = request.uri().path();
    let file_path = file_path.strip_prefix("/swagger/").unwrap();
    let file = utoipa_swagger_ui::serve(file_path, Arc::new(config));

    match file {
        Ok(Some(file)) => Ok(Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", file.content_type)
            .body(Body::fixed(file.bytes.into_owned()))
            .expect("could not build response")),
        Ok(None) => Err(cot::Error::not_found_message(format!(
            "Swagger file: `{file_path}` not found"
        ))),
        Err(err) => Err(cot::Error::custom(err.to_string())),
    }
}

async fn openapi_json() -> cot::Result<Response> {
    let openapi = utoipa::openapi::OpenApi::builder()
        .paths(
            Paths::builder()
                .path(
                    "/add/",
                    PathItem::builder()
                        .operation(
                            HttpMethod::Post,
                            Operation::builder()
                                .request_body(Some(
                                    RequestBody::builder()
                                        .content(
                                            "application/json",
                                            Content::builder()
                                                .schema(Some(AddRequest::schema()))
                                                .build(),
                                        )
                                        .build(),
                                ))
                                .build(),
                        )
                        .build(),
                )
                .build(),
        )
        .build();

    Response::new_json(StatusCode::OK, &openapi)
}

struct AddApp;

impl App for AddApp {
    fn name(&self) -> &'static str {
        env!("CARGO_PKG_NAME")
    }

    fn router(&self) -> Router {
        Router::with_urls([
            Route::with_handler("/add/", add),
            Route::with_handler("/swagger/", swagger_ui),
            Route::with_handler("/api-docs/openapi.json", openapi_json),
        ])
    }
}

// Test with:
// curl --header "Content-Type: application/json" --request POST --data '{"a": 123, "b": 456}' 'http://127.0.0.1:8000/'

struct JsonProject;

impl Project for JsonProject {
    fn cli_metadata(&self) -> CliMetadata {
        cot::cli::metadata!()
    }

    fn config(&self, _config_name: &str) -> cot::Result<ProjectConfig> {
        Ok(ProjectConfig::dev_default())
    }

    fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
        apps.register_with_views(AddApp, "");
    }
}

#[cot::main]
fn main() -> impl Project {
    JsonProject
}
