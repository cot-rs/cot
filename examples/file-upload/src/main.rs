use base64::Engine;
use cot::cli::CliMetadata;
use cot::config::ProjectConfig;
use cot::form::fields::InMemoryUploadedFile;
use cot::form::{Form, FormContext};
use cot::html::Html;
use cot::project::{MiddlewareContext, RegisterAppsContext, RootHandler};
use cot::request::extractors::RequestForm;
use cot::router::{Route, Router, Urls};
use cot::static_files::StaticFilesMiddleware;
use cot::{App, AppBuilder, Project, Template};

#[derive(Debug, Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    urls: &'a Urls,
    form: <FileForm as Form>::Context,
}

async fn index(urls: Urls) -> cot::Result<Html> {
    let index_template = IndexTemplate {
        urls: &urls,
        form: <<FileForm as Form>::Context as FormContext>::new(),
    };
    let rendered = index_template.render()?;

    Ok(Html::new(rendered))
}

#[derive(Debug, Form)]
struct FileForm {
    #[form(opts(max_length = 100))]
    title: String,
    #[form(opts(accept = vec!["image/*".to_string()]))]
    file: InMemoryUploadedFile,
}

#[derive(Debug, Template)]
#[template(path = "uploaded.html")]
struct UploadedTemplate<'a> {
    urls: &'a Urls,
    content_type: String,
    file_as_b64: String,
}

async fn upload(urls: Urls, RequestForm(file_form): RequestForm<FileForm>) -> cot::Result<Html> {
    let file_form = file_form.unwrap();

    let index_template = UploadedTemplate {
        urls: &urls,
        content_type: file_form
            .file
            .content_type()
            .map(|s| s.to_owned())
            .unwrap_or("image/png".to_owned()),
        file_as_b64: base64::prelude::BASE64_STANDARD.encode(file_form.file.content()),
    };
    let rendered = index_template.render()?;

    Ok(Html::new(rendered))
}

struct FileUploadApp;

impl App for FileUploadApp {
    fn name(&self) -> &'static str {
        env!("CARGO_PKG_NAME")
    }

    fn router(&self) -> Router {
        Router::with_urls([
            Route::with_handler_and_name("/", index, "index"),
            Route::with_handler_and_name("/upload", upload, "upload"),
        ])
    }
}

struct FileUploadProject;

impl Project for FileUploadProject {
    fn cli_metadata(&self) -> CliMetadata {
        cot::cli::metadata!()
    }

    fn config(&self, _config_name: &str) -> cot::Result<ProjectConfig> {
        Ok(ProjectConfig::default())
    }

    fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
        apps.register_with_views(FileUploadApp, "");
    }

    fn middlewares(
        &self,
        handler: cot::project::RootHandlerBuilder,
        context: &MiddlewareContext,
    ) -> RootHandler {
        handler
            .middleware(StaticFilesMiddleware::from_context(context))
            .build()
    }
}

#[cot::main]
fn main() -> impl Project {
    FileUploadProject
}
