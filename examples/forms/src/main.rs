mod migrations;

use chrono::{DateTime, Duration, FixedOffset, NaiveDate, NaiveDateTime, NaiveTime};
use chrono_tz::Tz;
use cot::cli::CliMetadata;
use cot::config::ProjectConfig;
use cot::db::migrations::SyncDynMigration;
use cot::db::{Auto, Database, Model, model};
use cot::form::Form;
use cot::form::fields::Step;
use cot::html::Html;
use cot::middleware::{AuthMiddleware, LiveReloadMiddleware, SessionMiddleware};
use cot::project::{MiddlewareContext, RegisterAppsContext, RootHandler, RootHandlerBuilder};
use cot::request::Request;
use cot::request::extractors::{RequestForm, StaticFiles};
use cot::response::Response;
use cot::router::{Route, Router, Urls};
use cot::static_files::{StaticFile, StaticFilesMiddleware};
use cot::{App, AppBuilder, Project, Template, reverse_redirect, static_files};

#[derive(Debug, Clone)]
#[model]
struct ExampleFormItem {
    #[model(primary_key)]
    id: Auto<i32>,
    title: String,
    datetime: NaiveDateTime,
    datetime_tz: DateTime<FixedOffset>,
    time: NaiveTime,
    date: NaiveDate,
}

#[derive(Debug, Form)]
struct ExampleForm {
    #[form(opts(max_length = 100))]
    title: String,
    datetime: NaiveDateTime,
    #[form(
        opts(
            timezone=Tz::America__New_York,
            step=Step::Value(Duration::seconds(70)),
            prefer_latest = true
        )
    )]
    datetime_tz: DateTime<FixedOffset>,
    #[form(
        opts(
            min = NaiveTime::parse_from_str("11:00:00", "%H:%M:%S").unwrap(),
            max = NaiveTime::parse_from_str("11:30:40", "%H:%M:%S").unwrap(),
            step = Step::Value(Duration::seconds(70))
        )
    )]
    time: NaiveTime,
    #[form(
        opts(
            min = NaiveDate::parse_from_str("2025-01-01", "%Y-%m-%d").unwrap(),
            max = NaiveDate::parse_from_str("2025-12-31", "%Y-%m-%d").unwrap(),
            step = Step::Value(Duration::days(7))
        )
    )]
    date: NaiveDate,
}

#[derive(Debug, Template)]
#[template(path = "index.html")]
struct IndexTemplate<'a> {
    static_files: StaticFiles,
    urls: &'a Urls,
    example_form_items: Vec<ExampleFormItem>,
    form: <ExampleForm as Form>::Context,
}

async fn index(
    urls: Urls,
    static_files: StaticFiles,
    mut request: Request,
    db: Database,
) -> cot::Result<Html> {
    let example_form_items = ExampleFormItem::objects().all(&db).await?;
    let index_template = IndexTemplate {
        urls: &urls,
        example_form_items,
        form: ExampleForm::build_context(&mut request).await?,
        static_files,
    };
    let rendered = index_template.render()?;

    Ok(Html::new(rendered))
}

async fn add_example_form(
    urls: Urls,
    db: Database,
    RequestForm(example_form): RequestForm<ExampleForm>,
) -> cot::Result<Response> {
    let example_form = example_form.unwrap();

    ExampleFormItem {
        id: Auto::auto(),
        title: example_form.title,
        date: example_form.date,
        datetime_tz: example_form.datetime_tz,
        datetime: example_form.datetime,
        time: example_form.time,
    }
    .save(&db)
    .await?;
    Ok(reverse_redirect!(urls, "index")?)
}

struct FormsApp;

impl App for FormsApp {
    fn name(&self) -> &'static str {
        env!("CARGO_CRATE_NAME")
    }

    fn migrations(&self) -> Vec<Box<SyncDynMigration>> {
        cot::db::migrations::wrap_migrations(migrations::MIGRATIONS)
    }

    fn router(&self) -> Router {
        Router::with_urls([
            Route::with_handler_and_name("/", index, "index"),
            Route::with_handler_and_name("/add", add_example_form, "add"),
        ])
    }

    fn static_files(&self) -> Vec<StaticFile> {
        static_files!("css/main.css")
    }
}

struct FormsProject;

impl Project for FormsProject {
    fn cli_metadata(&self) -> CliMetadata {
        cot::cli::metadata!()
    }

    fn config(&self, _config_name: &str) -> cot::Result<ProjectConfig> {
        Ok(ProjectConfig::dev_default())
    }

    fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
        apps.register_with_views(FormsApp, "");
    }

    fn middlewares(&self, handler: RootHandlerBuilder, context: &MiddlewareContext) -> RootHandler {
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
    FormsProject
}
