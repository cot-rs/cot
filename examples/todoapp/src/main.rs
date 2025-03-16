mod migrations;

use std::fmt::{Display, Formatter};

use cot::__private::async_trait;
use cot::admin::{AdminApp, AdminModel, AdminModelManager, DefaultAdminModelManager};
use cot::auth::db::{DatabaseUser, DatabaseUserApp};
use cot::cli::CliMetadata;
use cot::db::migrations::SyncDynMigration;
use cot::db::{model, query, Auto, Model};
use cot::form::Form;
use cot::middleware::{LiveReloadMiddleware, SessionMiddleware};
use cot::project::{RootHandlerBuilder, WithApps, WithConfig};
use cot::request::{Request, RequestExt};
use cot::response::{Response, ResponseExt};
use cot::router::{Route, Router};
use cot::static_files::StaticFilesMiddleware;
use cot::{
    reverse_redirect, App, AppBuilder, Body, BoxedHandler, Project, ProjectContext, StatusCode,
};
use rinja::Template;

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
    request: &'a Request,
    form: &'a <TodoForm as Form>::Context,
    todo_items: Vec<TodoItem>,
}

async fn index(mut request: Request) -> cot::Result<Response> {
    let todo_items = TodoItem::objects().all(request.db()).await?;
    let context = TodoForm::build_context(&mut request).await.unwrap();
    let index_template = IndexTemplate {
        request: &request,
        form: &context,
        todo_items,
    };
    let rendered = index_template.render()?;

    Ok(Response::new_html(StatusCode::OK, Body::fixed(rendered)))
}

#[derive(Debug, Form)]
struct TodoForm {
    #[form(opt(max_length = 100))]
    title: String,
}

impl TodoForm {
    fn xd(&self) {
        use cot as kcrate_ident;
        {
            {
                let xd = ::std::boxed::Box::new(
                    <
                        <Self as kcrate_ident::form::Form>::Context as kcrate_ident::form::FormContext>
                        ::new()
                );
            }
        }
    }
}

async fn add_todo(mut request: Request) -> cot::Result<Response> {
    let todo_form = TodoForm::from_request(&mut request).await?.unwrap();

    {
        TodoItem {
            id: Auto::auto(),
            title: todo_form.title,
        }
        .save(request.db())
        .await?;
    }

    Ok(reverse_redirect!(request, "index")?)
}

async fn remove_todo(request: Request) -> cot::Result<Response> {
    let todo_id: i32 = request.path_params().parse()?;

    {
        query!(TodoItem, $id == todo_id)
            .delete(request.db())
            .await?;
    }

    Ok(reverse_redirect!(request, "index")?)
}

struct TodoApp;

#[async_trait]
impl App for TodoApp {
    fn name(&self) -> &'static str {
        "todo-app"
    }

    async fn init(&self, context: &mut ProjectContext) -> cot::Result<()> {
        let user = DatabaseUser::get_by_username(context.database(), "admin").await?;
        if user.is_none() {
            DatabaseUser::create_user(context.database(), "admin", "admin").await?;
        }

        Ok(())
    }

    fn admin_model_managers(&self) -> Vec<Box<dyn AdminModelManager>> {
        vec![Box::new(DefaultAdminModelManager::<TodoItem>::new())]
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

struct TodoappProject;

impl Project for TodoappProject {
    fn cli_metadata(&self) -> CliMetadata {
        cot::cli::metadata!()
    }

    fn register_apps(&self, apps: &mut AppBuilder, _context: &ProjectContext<WithConfig>) {
        apps.register(DatabaseUserApp::new());
        apps.register_with_views(AdminApp::new(), "/admin");
        apps.register_with_views(TodoApp, "");
    }

    fn middlewares(
        &self,
        handler: RootHandlerBuilder,
        context: &ProjectContext<WithApps>,
    ) -> BoxedHandler {
        handler
            .middleware(StaticFilesMiddleware::from_context(context))
            .middleware(LiveReloadMiddleware::from_context(context))
            .middleware(SessionMiddleware::new())
            .build()
    }
}

#[cot::main]
fn main() -> impl Project {
    TodoappProject
}
