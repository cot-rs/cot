use std::error::Error;

use cot::cli::CliMetadata;
use cot::config::ProjectConfig;
use cot::error::handler::{DynErrorPageHandler, ErrorPageHandler};
use cot::error::not_found::NotFound;
use cot::html::Html;
use cot::project::RegisterAppsContext;
use cot::response::{IntoResponse, Response};
use cot::router::{Route, Router};
use cot::{App, AppBuilder, Project, StatusCode};

async fn return_hello() -> cot::Result<Response> {
    panic!()
}

struct HelloApp;

impl App for HelloApp {
    fn name(&self) -> &'static str {
        env!("CARGO_PKG_NAME")
    }

    fn router(&self) -> Router {
        Router::with_urls([Route::with_handler("/", return_hello)])
    }
}

struct HelloProject;

impl Project for HelloProject {
    fn cli_metadata(&self) -> CliMetadata {
        cot::cli::metadata!()
    }

    fn config(&self, _config_name: &str) -> cot::Result<ProjectConfig> {
        let mut config = ProjectConfig::dev_default();
        config.debug = false; // make sure we can see our custom error pages
        config.register_panic_hook = true;
        Ok(config)
    }

    fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
        apps.register_with_views(HelloApp, "");
    }

    fn server_error_handler(&self) -> DynErrorPageHandler {
        DynErrorPageHandler::new(error_page_handler)
    }
}

async fn error_page_handler(error: cot::Error) -> impl IntoResponse {
    if let Some(inner) = error.source() {
        if inner.is::<NotFound>() {
            return Html::new(include_str!("404.html"))
                .with_status(StatusCode::INTERNAL_SERVER_ERROR)
                .into_response();
        }
    }

    Html::new(include_str!("500.html"))
        .with_status(StatusCode::INTERNAL_SERVER_ERROR)
        .into_response()
}

#[cot::main]
fn main() -> impl Project {
    HelloProject
}
