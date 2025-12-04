use cot::cli::CliMetadata;
use cot::config::{DatabaseConfig, EmailConfig, EmailTransportTypeConfig, ProjectConfig};
use cot::email::{EmailBackend, EmailMessage, SmtpTransportMode};
use cot::form::Form;
use cot::html::Html;
use cot::project::RegisterAppsContext;
use cot::request::{Request, RequestExt};
use cot::router::{Route, Router};
use cot::{App, AppBuilder, Project};

struct EmailApp;

impl App for EmailApp {
    fn name(&self) -> &str {
        "email"
    }

    fn router(&self) -> Router {
        Router::with_urls([
            Route::with_handler_and_name("/", email_form, "email_form"),
            Route::with_handler_and_name("/send", send_email, "send_email"),
        ])
    }
}

async fn email_form(_request: Request) -> cot::Result<Html> {
    let template = String::from(include_str!("../templates/index.html"));
    Ok(Html::new(template))
}
#[derive(Debug, Form)]
struct EmailForm {
    from: String,
    to: String,
    subject: String,
    body: String,
}
async fn send_email(mut request: Request) -> cot::Result<Html> {
    let form = EmailForm::from_request(&mut request).await?.unwrap();

    let from = form.from;
    let to = form.to;
    let subject = form.subject;
    let body = form.body;

    // Create the email
    let email = EmailMessage {
        subject,
        from: from.into(),
        to: vec![to],
        body,
        alternative_html: None,
        ..Default::default()
    };
    let _database = request.context().database();
    let email_backend = request.context().email_backend();
    {
        let _x = email_backend.lock().unwrap().send_message(&email);
    }
    let template = String::from(include_str!("../templates/sent.html"));
    Ok(Html::new(template))
}
struct MyProject;
impl Project for MyProject {
    fn cli_metadata(&self) -> CliMetadata {
        cot::cli::metadata!()
    }

    fn config(&self, _config_name: &str) -> cot::Result<ProjectConfig> {
        let mut email_config = EmailConfig::builder();
        email_config.backend_type(EmailTransportTypeConfig::Smtp);
        email_config.smtp_mode(SmtpTransportMode::Localhost);
        email_config.port(1025_u16);
        let config = ProjectConfig::builder()
            .debug(true)
            .database(DatabaseConfig::builder().url("sqlite::memory:").build())
            .email_backend(email_config.build())
            .build();
        Ok(config)
    }
    fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
        apps.register_with_views(EmailApp, "");
    }

    fn middlewares(
        &self,
        handler: cot::project::RootHandlerBuilder,
        _context: &cot::project::MiddlewareContext,
    ) -> cot::BoxedHandler {
        // context.config().email_backend().unwrap();
        handler.build()
    }
}

#[cot::main]
fn main() -> impl Project {
    MyProject
}
