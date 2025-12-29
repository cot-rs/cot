use askama::Template;
use cot::cli::CliMetadata;
use cot::config::ProjectConfig;
use cot::project::RegisterAppsContext;
use cot::{App, AppBuilder, Project};
use cot_core::error::handler::{DynErrorPageHandler, RequestError};
use cot_core::html::Html;
use cot_core::response::{IntoResponse, Response};
use cot_core::router::{Route, Router};

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

    fn error_handler(&self) -> DynErrorPageHandler {
        DynErrorPageHandler::new(error_page_handler)
    }
}

async fn error_page_handler(error: RequestError) -> cot::Result<impl IntoResponse> {
    #[derive(Debug, Template)]
    #[template(path = "error.html")]
    struct ErrorTemplate {
        error: RequestError,
    }

    let status_code = error.status_code();
    let error_template = ErrorTemplate { error };
    let rendered = error_template.render()?;

    Ok(Html::new(rendered).with_status(status_code))
}

#[cot::main]
fn main() -> impl Project {
    HelloProject
}
