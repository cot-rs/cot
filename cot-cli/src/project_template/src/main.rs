mod migrations;

use cot::bytes::Bytes;
use cot::cli::CliMetadata;
use cot::db::migrations::SyncDynMigration;
use cot::middleware::{AuthMiddleware, LiveReloadMiddleware, SessionMiddleware};
use cot::project::{MiddlewareContext, RegisterAppsContext, RootHandlerBuilder};
use cot::response::{Response, ResponseExt};
use cot::router::{Route, Router};
use cot::static_files::StaticFilesMiddleware;
use cot::{App, AppBuilder, Body, BoxedHandler, Project, StatusCode, static_files};
use rinja::Template;

#[derive(Debug, Template)]
#[template(path = "index.html")]
struct IndexTemplate {}

async fn index() -> cot::Result<Response> {
    let index_template = IndexTemplate {};
    let rendered = index_template.render()?;

    Ok(Response::new_html(StatusCode::OK, Body::fixed(rendered)))
}

struct {{ app_name }};

impl App for {{ app_name }} {
    fn name(&self) -> &'static str {
        env!("CARGO_CRATE_NAME")
    }

    fn migrations(&self) -> Vec<Box<SyncDynMigration>> {
        cot::db::migrations::wrap_migrations(migrations::MIGRATIONS)
    }

    fn router(&self) -> Router {
        Router::with_urls([Route::with_handler_and_name("/", index, "index")])
    }

    fn static_files(&self) -> Vec<(String, Bytes)> {
        static_files!("css/main.css")
    }
}

struct {{ project_struct_name }};

impl Project for {{ project_struct_name }} {
    fn cli_metadata(&self) -> CliMetadata {
        cot::cli::metadata!()
    }

    fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
        apps.register_with_views({{ app_name }}, "");
    }

    fn middlewares(
        &self,
        handler: RootHandlerBuilder,
        context: &MiddlewareContext,
    ) -> BoxedHandler {
        handler
            .middleware(StaticFilesMiddleware::from_context(context))
            .middleware(AuthMiddleware::new())
            .middleware(SessionMiddleware::new())
            .middleware(LiveReloadMiddleware::from_context(context))
            .build()
    }
}

#[cot::main]
fn main() -> impl Project {
    {{ project_struct_name }}
}
