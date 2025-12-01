mod migrations;

use std::fmt::{Display, Formatter};
use std::time::Duration;

use askama::Template;
use async_trait::async_trait;
use cot::admin::{AdminApp, AdminModel, AdminModelManager, DefaultAdminModelManager};
use cot::auth::db::{DatabaseUser, DatabaseUserApp};
use cot::cli::CliMetadata;
use cot::config::{
    AuthBackendConfig, DatabaseConfig, MiddlewareConfig, ProjectConfig, SessionMiddlewareConfig,
    StaticFilesConfig, StaticFilesPathRewriteMode,
};
use cot::db::migrations::SyncDynMigration;
use cot::db::{Auto, Model, model};
use cot::form::Form;
use cot::html::Html;
use cot::middleware::{AuthMiddleware, LiveReloadMiddleware, SessionMiddleware};
use cot::project::{MiddlewareContext, RegisterAppsContext, RootHandler};
use cot::request::extractors::RequestDb;
use cot::router::{Route, Router, Urls};
use cot::static_files::StaticFilesMiddleware;
use cot::{App, AppBuilder, Project, ProjectContext};

#[derive(Debug, Clone, Form, AdminModel)]
#[model]
struct TodoItem {
    #[model(primary_key)]
    id: Auto<i32>,
    title: String,
}

impl Display for TodoItem {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.title)
    }
}

#[derive(Debug, Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    urls: &'a Urls,
    todo_items: Vec<TodoItem>,
}

async fn index(urls: Urls, RequestDb(db): RequestDb) -> cot::Result<Html> {
    let todo_items = TodoItem::objects().all(&db).await?;
    let index_template = IndexTemplate {
        urls: &urls,
        todo_items,
    };
    let rendered = index_template.render()?;

    Ok(Html::new(rendered))
}

struct HelloApp;

#[async_trait]
impl App for HelloApp {
    fn name(&self) -> &'static str {
        env!("CARGO_PKG_NAME")
    }

    async fn init(&self, context: &mut ProjectContext) -> cot::Result<()> {
        // TODO use transaction
        let user = DatabaseUser::get_by_username(context.database(), "admin").await?;
        if user.is_none() {
            DatabaseUser::create_user(context.database(), "admin", "admin").await?;
        }

        Ok(())
    }

    fn migrations(&self) -> Vec<Box<SyncDynMigration>> {
        cot::db::migrations::wrap_migrations(migrations::MIGRATIONS)
    }

    fn admin_model_managers(&self) -> Vec<Box<dyn AdminModelManager>> {
        vec![Box::new(DefaultAdminModelManager::<TodoItem>::new())]
    }

    fn router(&self) -> Router {
        Router::with_urls([Route::with_handler("/", index)])
    }
}

struct AdminProject;

impl Project for AdminProject {
    fn cli_metadata(&self) -> CliMetadata {
        cot::cli::metadata!()
    }

    fn config(&self, _config_name: &str) -> cot::Result<ProjectConfig> {
        Ok(ProjectConfig::builder()
            .debug(true)
            .database(
                DatabaseConfig::builder()
                    .url("sqlite://db.sqlite3?mode=rwc")
                    .build(),
            )
            .auth_backend(AuthBackendConfig::Database)
            .middlewares(
                MiddlewareConfig::builder()
                    .session(SessionMiddlewareConfig::builder().secure(false).build())
                    .build(),
            )
            .static_files(
                StaticFilesConfig::builder()
                    .rewrite(StaticFilesPathRewriteMode::QueryParam)
                    .cache_timeout(Duration::from_secs(300))
                    .build(),
            )
            .build())
    }

    fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
        apps.register(DatabaseUserApp::new());
        apps.register_with_views(AdminApp::new(), "/admin");
        apps.register_with_views(HelloApp, "");
    }

    fn middlewares(
        &self,
        handler: cot::project::RootHandlerBuilder,
        context: &MiddlewareContext,
    ) -> RootHandler {
        handler
            .middleware(StaticFilesMiddleware::from_context(context))
            .middleware(AuthMiddleware::new())
            .middleware(SessionMiddleware::from_context(context))
            .middleware(LiveReloadMiddleware::new())
            .build()
    }
}

#[cot::main]
fn main() -> impl Project {
    AdminProject
}
