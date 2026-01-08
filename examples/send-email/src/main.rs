use askama::Template;
use cot::cli::CliMetadata;
use cot::common_types::Email;
use cot::config::{EmailConfig, EmailTransportConfig, EmailTransportTypeConfig, ProjectConfig};
use cot::email::EmailMessage;
use cot::form::Form;
use cot::html::Html;
use cot::middleware::LiveReloadMiddleware;
use cot::project::{RegisterAppsContext, RootHandler};
use cot::request::extractors::{StaticFiles, UrlQuery};
use cot::request::{Request, RequestExt};
use cot::response::Response;
use cot::router::{Route, Router, Urls};
use cot::static_files::{StaticFile, StaticFilesMiddleware};
use cot::{App, AppBuilder, Project, reverse_redirect, static_files};
use serde::{Deserialize, Serialize};

struct EmailApp;

impl App for EmailApp {
    fn name(&self) -> &'static str {
        env!("CARGO_CRATE_NAME")
    }

    fn router(&self) -> Router {
        Router::with_urls([
            Route::with_handler_and_name("/", index, "index/"),
            Route::with_handler_and_name("/send", send_email, "send_email"),
        ])
    }

    fn static_files(&self) -> Vec<StaticFile> {
        static_files!("css/main.css")
    }
}

#[derive(Debug, Form)]
struct EmailForm {
    from: Email,
    to: Email,
    subject: String,
    message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Status {
    Success,
    Failure,
}

impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Status::Success => write!(f, "Success"),
            Status::Failure => write!(f, "Failure"),
        }
    }
}

#[derive(Debug, Template)]
#[allow(unused)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    static_files: StaticFiles,
    urls: &'a Urls,
    form: <EmailForm as Form>::Context,
    status: &'a str,
}

#[derive(Serialize, Deserialize, Debug)]
struct IndexQuery {
    status: Option<Status>,
}

async fn index(
    urls: Urls,
    mut request: Request,
    static_files: StaticFiles,
    UrlQuery(query): UrlQuery<IndexQuery>,
) -> cot::Result<Html> {
    let status = match query.status {
        Some(s) => s.to_string(),
        None => "".to_string(),
    };
    let index_template = IndexTemplate {
        urls: &urls,
        form: EmailForm::build_context(&mut request).await?,
        status: &status,
        static_files,
    };
    let rendered = index_template.render()?;

    Ok(Html::new(rendered))
}

async fn send_email(urls: Urls, mut request: Request) -> cot::Result<Response> {
    let form = EmailForm::from_request(&mut request).await?;

    let form = form.unwrap();

    let message = EmailMessage::builder()
        .from(form.from)
        .to(vec![form.to])
        .subject(form.subject)
        .body(form.message)
        .build()?;

    request.email().send(message).await?;

    // TODO: We should redirect with the status when reverse_redirect! supports
    // query parameters.
    Ok(reverse_redirect!(&urls, "index/")?)
}

struct MyProject;
impl Project for MyProject {
    fn cli_metadata(&self) -> CliMetadata {
        cot::cli::metadata!()
    }

    fn config(&self, _config_name: &str) -> cot::Result<ProjectConfig> {
        Ok(ProjectConfig::builder()
            .email(
                EmailConfig::builder()
                    .transport(
                        EmailTransportConfig::builder()
                            .transport_type(EmailTransportTypeConfig::Console)
                            .build(),
                    )
                    .build(),
            )
            .build())
    }

    fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
        apps.register_with_views(EmailApp, "");
    }

    fn middlewares(
        &self,
        handler: cot::project::RootHandlerBuilder,
        context: &cot::project::MiddlewareContext,
    ) -> RootHandler {
        handler
            .middleware(StaticFilesMiddleware::from_context(context))
            .middleware(LiveReloadMiddleware::from_context(context))
            .build()
    }
}

#[cot::main]
fn main() -> impl Project {
    MyProject
}
