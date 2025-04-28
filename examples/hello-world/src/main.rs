use cot::cli::CliMetadata;
use cot::config::ProjectConfig;
use cot::project::RegisterAppsContext;
use cot::router::{Route, Router};
use cot::{App, AppBuilder, Project};

async fn return_hello() -> &'static str {
    "Hello Cot!"
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
        Ok(ProjectConfig::dev_default())
    }

    fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
        apps.register_with_views(HelloApp, "");
    }
}

#[cot::main]
fn main() -> impl Project {
    HelloProject
}
