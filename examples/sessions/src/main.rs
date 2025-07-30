use askama::Template;
use cot::cli::CliMetadata;
use cot::config::{
    DatabaseConfig, MiddlewareConfig, ProjectConfig, SessionMiddlewareConfig, SessionStoreConfig,
    SessionStoreTypeConfig,
};
use cot::form::Form;
use cot::html::Html;
use cot::middleware::SessionMiddleware;
use cot::project::{MiddlewareContext, RegisterAppsContext, RootHandler};
use cot::request::Request;
use cot::response::{IntoResponse, Response};
use cot::router::{Route, Router, Urls};
use cot::session::Session;
use cot::session::db::SessionApp;
use cot::{App, AppBuilder, Project, reverse_redirect};

#[derive(Debug, Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    name: String,
}

#[derive(Debug, Template)]
#[template(path = "name.html")]
struct NameTemplate<'a> {
    urls: &'a Urls,
}

#[derive(Debug, Form)]
struct NameForm {
    #[form(opt(max_length = 100))]
    name: String,
}

async fn hello(urls: Urls, session: Session) -> cot::Result<Response> {
    let name: String = session
        .get("user_name")
        .await
        .expect("Invalid session value")
        .unwrap_or_default();
    if name.is_empty() {
        return Ok(reverse_redirect!(urls, "name")?);
    }

    let template = IndexTemplate { name };

    Html::new(template.render()?).into_response()
}

async fn name(urls: Urls, session: Session, mut request: Request) -> cot::Result<Response> {
    if request.method() == cot::Method::POST {
        let name_form = NameForm::from_request(&mut request).await?.unwrap();
        session.insert("user_name", name_form.name).await?;

        return Ok(reverse_redirect!(urls, "index")?);
    }

    let template = NameTemplate { urls: &urls };

    Html::new(template.render()?).into_response()
}

struct HelloApp;

impl App for HelloApp {
    fn name(&self) -> &'static str {
        env!("CARGO_PKG_NAME")
    }

    fn router(&self) -> Router {
        Router::with_urls([
            Route::with_handler_and_name("/", hello, "index"),
            Route::with_handler_and_name("/name", name, "name"),
        ])
    }
}

struct SessionsProject;

impl Project for SessionsProject {
    fn cli_metadata(&self) -> CliMetadata {
        cot::cli::metadata!()
    }

    fn config(&self, _config_name: &str) -> cot::Result<ProjectConfig> {
        Ok(ProjectConfig::builder()
            .database(
                DatabaseConfig::builder()
                    .url("sqlite://example-session.sqlite3?mode=rwc")
                    .build(),
            )
            .middlewares(
                MiddlewareConfig::builder()
                    .session(
                        SessionMiddlewareConfig::builder()
                            .secure(false)
                            .store(
                                SessionStoreConfig::builder()
                                    .store_type(SessionStoreTypeConfig::Database)
                                    .build(),
                            )
                            .build(),
                    )
                    .build(),
            )
            .build())
    }

    fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
        apps.register_with_views(HelloApp, "");
        apps.register(SessionApp::new());
    }

    fn middlewares(
        &self,
        handler: cot::project::RootHandlerBuilder,
        context: &MiddlewareContext,
    ) -> RootHandler {
        handler
            .middleware(SessionMiddleware::from_context(context))
            .build()
    }
}

#[cot::main]
fn main() -> impl Project {
    SessionsProject
}
