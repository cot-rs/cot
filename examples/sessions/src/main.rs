use askama::Template;
use cot::cli::CliMetadata;
use cot::config::ProjectConfig;
use cot::form::Form;
use cot::middleware::SessionMiddleware;
use cot::project::{MiddlewareContext, RegisterAppsContext};
use cot::request::Request;
use cot::response::{Response, ResponseExt};
use cot::router::{Route, Router, Urls};
use cot::session::Session;
use cot::{App, AppBuilder, Body, BoxedHandler, Project, StatusCode, reverse_redirect};

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

    Ok(Response::new_html(
        StatusCode::OK,
        Body::fixed(template.render()?),
    ))
}

async fn name(urls: Urls, session: Session, mut request: Request) -> cot::Result<Response> {
    if request.method() == cot::Method::POST {
        let name_form = NameForm::from_request(&mut request).await?.unwrap();
        session.insert("user_name", name_form.name).await?;

        return Ok(reverse_redirect!(urls, "index")?);
    }

    let template = NameTemplate { urls: &urls };

    Ok(Response::new_html(
        StatusCode::OK,
        Body::fixed(template.render()?),
    ))
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
        Ok(ProjectConfig::dev_default())
    }

    fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
        apps.register_with_views(HelloApp, "");
    }

    fn middlewares(
        &self,
        handler: cot::project::RootHandlerBuilder,
        _context: &MiddlewareContext,
    ) -> BoxedHandler {
        handler.middleware(SessionMiddleware::new()).build()
    }
}

#[cot::main]
fn main() -> impl Project {
    SessionsProject
}
