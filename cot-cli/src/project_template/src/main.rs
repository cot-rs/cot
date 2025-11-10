mod migrations;

use askama::Template;
use cot::auth::db::DatabaseUserApp;
use cot::cli::CliMetadata;
use cot::db::migrations::SyncDynMigration;
use cot::html::Html;
use cot::middleware::{AuthMiddleware, LiveReloadMiddleware, SessionMiddleware};
use cot::project::{MiddlewareContext, RegisterAppsContext, RootHandler, RootHandlerBuilder};
use cot::request::extractors::StaticFiles;
use cot::router::{Route, Router};
use cot::static_files::{StaticFile, StaticFilesMiddleware};
use cot::session::db::SessionApp;
use cot::{App, AppBuilder, Project, static_files};

#[derive(Debug, Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    static_files: StaticFiles,
}

async fn index(static_files: StaticFiles) -> cot::Result<Html> {
    let index_template = IndexTemplate { static_files };
    let rendered = index_template.render()?;

    Ok(Html::new(rendered))
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

    fn static_files(&self) -> Vec<StaticFile> {
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
        apps.register(DatabaseUserApp::new());
        apps.register(SessionApp::new());
    }

    fn middlewares(
        &self,
        handler: RootHandlerBuilder,
        context: &MiddlewareContext,
    ) -> RootHandler {
        handler
            .middleware(StaticFilesMiddleware::from_context(context))
            .middleware(AuthMiddleware::new())
            .middleware(SessionMiddleware::from_context(context))
            .middleware(LiveReloadMiddleware::from_context(context))
            .build()
    }
}

#[cot::main]
fn main() -> impl Project {
    {{ project_struct_name }}
}
