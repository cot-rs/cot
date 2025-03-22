use cot::cli::CliMetadata;
use cot::config::ProjectConfig;
use cot::project::RegisterAppsContext;
use cot::request::extractors::Json;
use cot::response::{Response, ResponseExt};
use cot::router::{Route, Router};
use cot::{App, AppBuilder, Project, StatusCode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
struct AddRequest {
    a: i32,
    b: i32,
}

#[derive(Debug, Clone, Serialize)]
struct AddResponse {
    result: i32,
}

async fn add(Json(add_request): Json<AddRequest>) -> cot::Result<Response> {
    let response = AddResponse {
        result: add_request.a + add_request.b,
    };

    Response::new_json(StatusCode::OK, &response)
}

struct AddApp;

impl App for AddApp {
    fn name(&self) -> &'static str {
        env!("CARGO_PKG_NAME")
    }

    fn router(&self) -> Router {
        Router::with_urls([Route::with_handler("/", add)])
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
