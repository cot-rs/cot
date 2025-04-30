mod migrations;

use askama::Template;
use cot::auth::db::DatabaseUserApp;
use cot::cli::CliMetadata;
use cot::config::{DatabaseConfig, ProjectConfig};
use cot::db::migrations::SyncDynMigration;
use cot::db::{Auto, Model, model, query};
use cot::form::Form;
use cot::html::Html;
use cot::project::{MiddlewareContext, RegisterAppsContext};
use cot::request::extractors::{Path, RequestDb, RequestForm};
use cot::response::Response;
use cot::router::{Route, Router, Urls};
use cot::static_files::StaticFilesMiddleware;
use cot::{App, AppBuilder, BoxedHandler, Project, reverse_redirect};

#[derive(Debug, Clone)]
#[model]
struct TodoItem {
    #[model(primary_key)]
    id: Auto<i32>,
    title: String,
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

#[derive(Debug, Form)]
struct TodoForm {
    #[form(opt(max_length = 100))]
    title: String,
}

async fn add_todo(
    urls: Urls,
    RequestDb(db): RequestDb,
    RequestForm(todo_form): RequestForm<TodoForm>,
) -> cot::Result<Response> {
    let todo_form = todo_form.unwrap();

    TodoItem {
        id: Auto::auto(),
        title: todo_form.title,
    }
    .save(&db)
    .await?;

    Ok(reverse_redirect!(urls, "index")?)
}

async fn remove_todo(
    urls: Urls,
    RequestDb(db): RequestDb,
    Path(todo_id): Path<i32>,
) -> cot::Result<Response> {
    query!(TodoItem, $id == todo_id).delete(&db).await?;

    Ok(reverse_redirect!(urls, "index")?)
}

struct TodoApp;

impl App for TodoApp {
    fn name(&self) -> &'static str {
        "todo-app"
    }

    fn migrations(&self) -> Vec<Box<SyncDynMigration>> {
        cot::db::migrations::wrap_migrations(migrations::MIGRATIONS)
    }

    fn router(&self) -> Router {
        Router::with_urls([
            Route::with_handler_and_name("/", index, "index"),
            Route::with_handler_and_name("/todos/add", add_todo, "add-todo"),
            Route::with_handler_and_name("/todos/{todo_id}/remove", remove_todo, "remove-todo"),
        ])
    }
}

struct TodoProject;

impl Project for TodoProject {
    fn cli_metadata(&self) -> CliMetadata {
        cot::cli::metadata!()
    }

    fn config(&self, _config_name: &str) -> cot::Result<ProjectConfig> {
        Ok(ProjectConfig::builder()
            .debug(true)
            .database(DatabaseConfig::builder().url("sqlite::memory:").build())
            .build())
    }

    fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
        apps.register(DatabaseUserApp::new());
        apps.register_with_views(TodoApp, "");
    }

    fn middlewares(
        &self,
        handler: cot::project::RootHandlerBuilder,
        context: &MiddlewareContext,
    ) -> BoxedHandler {
        handler
            .middleware(StaticFilesMiddleware::from_context(context))
            .build()
    }
}

#[cot::main]
fn main() -> impl Project {
    TodoProject
}
