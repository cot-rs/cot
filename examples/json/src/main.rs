use std::error::Error;
use std::sync::Arc;

use aide::openapi::{PathItem, ReferenceOr, Responses};
use aide::swagger::Swagger;
use cot::cli::CliMetadata;
use cot::config::ProjectConfig;
use cot::openapi::{AsOpenapiOperation, RouteInfo};
use cot::project::RegisterAppsContext;
use cot::request::Request;
use cot::request::extractors::{Json, Path, UrlQuery};
use cot::response::{Response, ResponseExt};
use cot::router::{Route, Router};
use cot::{App, AppBuilder, Body, Project, StatusCode};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
struct AddRequest {
    a: i32,
    b: i32,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
struct AddRequestQuery {
    a: i32,
    b: Option<i32>,
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
struct AddResponse {
    result: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
struct AddPath {
    gowno: i32,
    wdupie: String,
}

async fn add(
    Json(add_request): Json<AddRequest>,
    path: Path<AddPath>,
    query: UrlQuery<AddRequestQuery>,
) -> cot::Result<Response> {
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
    let mut route_info = RouteInfo::new();
    route_info.param_names = &["gowno", "wdupie"];
    let openapi = aide::openapi::OpenApi {
        paths: Some(aide::openapi::Paths {
            paths: IndexMap::from([(
                "/add/{gowno}/{wdupie}/".to_string(),
                ReferenceOr::Item({
                    let mut item = PathItem::default();

                    let mut operation = add.as_operation(&route_info);
                    operation.responses = Some(Responses {
                        ..Default::default()
                    });
                    item.post = Some(operation);

                    item
                }),
            )]),
            ..Default::default()
        }),
        ..Default::default()
    };

    Response::new_json(StatusCode::OK, &openapi)
}

struct AddApp;

impl App for AddApp {
    fn name(&self) -> &'static str {
        env!("CARGO_PKG_NAME")
    }

    fn router(&self) -> Router {
        Router::with_urls([
            Route::with_handler("/add/{gowno}/{wdupie}/", add),
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
