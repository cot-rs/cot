use std::error::Error;

use async_trait::async_trait;
use cot::admin::AdminApp;
use cot::auth::db::{DatabaseUser, DatabaseUserApp};
use cot::cli::CliMetadata;
use cot::config::{
    AuthBackendConfig, DatabaseConfig, MiddlewareConfig, ProjectConfig, SessionMiddlewareConfig,
};
use cot::middleware::{AuthMiddleware, SessionMiddleware};
use cot::project::{MiddlewareContext, RegisterAppsContext};
use cot::static_files::StaticFilesMiddleware;
use cot::test::{TestServer, TestServerBuilder};
use cot::{App, AppBuilder, BoxedHandler, Project, ProjectContext};
use fantoccini::{Client, ClientBuilder, Locator};

const DEFAULT_USERNAME: &str = "admin";
const DEFAULT_PASSWORD: &str = "admin";

struct HelloApp;

#[async_trait]
impl App for HelloApp {
    fn name(&self) -> &'static str {
        env!("CARGO_PKG_NAME")
    }

    async fn init(&self, context: &mut ProjectContext) -> cot::Result<()> {
        DatabaseUser::create_user(context.database(), DEFAULT_USERNAME, DEFAULT_PASSWORD).await?;
        Ok(())
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
            .database(DatabaseConfig::builder().url("sqlite::memory:").build())
            .auth_backend(AuthBackendConfig::Database)
            .middlewares(
                MiddlewareConfig::builder()
                    .session(SessionMiddlewareConfig::builder().secure(false).build())
                    .build(),
            )
            .build())
    }

    fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
        apps.register(DatabaseUserApp::new());
        apps.register_with_views(AdminApp::new(), "/admin");
        apps.register(HelloApp);
    }

    fn middlewares(
        &self,
        handler: cot::project::RootHandlerBuilder,
        context: &MiddlewareContext,
    ) -> BoxedHandler {
        handler
            .middleware(StaticFilesMiddleware::from_context(context))
            .middleware(AuthMiddleware::new())
            .middleware(SessionMiddleware::from_context(context))
            .build()
    }
}

#[ignore = "This test requires a Webdriver to be running"]
#[cot::e2e_test]
async fn admin_e2e_login() -> Result<(), Box<dyn Error>> {
    let server = TestServerBuilder::new(AdminProject).start().await;
    let driver = create_webdriver().await?;

    login(&server, &driver).await?;

    let welcome_message = driver
        .find(Locator::XPath(
            "//h2[contains(text(), 'Choose a model to manage')]",
        ))
        .await?;
    assert!(welcome_message.is_displayed().await?);

    driver.close().await?;
    server.close().await;
    Ok(())
}

#[ignore = "This test requires a Webdriver to be running"]
#[cot::e2e_test]
async fn admin_e2e_change_password() -> Result<(), Box<dyn Error>> {
    const NEW_PASSWORD: &str = "test";

    let server = TestServerBuilder::new(AdminProject).start().await;
    let driver = create_webdriver().await?;

    login(&server, &driver).await?;

    let database_user_link = driver.find(Locator::LinkText("DatabaseUser")).await?;
    database_user_link.click().await?;
    let admin_user_link = driver.find(Locator::LinkText(DEFAULT_USERNAME)).await?;
    admin_user_link.click().await?;
    let password_form = driver.find(Locator::Id("password")).await?;
    password_form.send_keys(NEW_PASSWORD).await?;
    let submit_button = driver.find(Locator::Css("button[type=submit]")).await?;
    submit_button.click().await?;

    // Check the user was logged out
    assert!(
        driver
            .current_url()
            .await?
            .as_str()
            .ends_with("/admin/login/")
    );

    // Try to log in with the old password
    login(&server, &driver).await?;
    let error_alert = driver.find(Locator::Css("div.form-errors")).await?;
    assert!(error_alert.is_displayed().await?);
    let error_message = error_alert.text().await?.clone();
    assert!(
        error_message.contains("Invalid username or password"),
        "Error message not found, actual message: {error_message}"
    );

    // Log in with the new password
    login_with(&server, &driver, DEFAULT_USERNAME, NEW_PASSWORD).await?;

    driver.close().await?;
    server.close().await;
    Ok(())
}

async fn login(server: &TestServer<AdminProject>, driver: &Client) -> Result<(), Box<dyn Error>> {
    login_with(server, driver, DEFAULT_USERNAME, DEFAULT_PASSWORD).await
}

async fn login_with(
    server: &TestServer<AdminProject>,
    driver: &Client,
    username: &str,
    password: &str,
) -> Result<(), Box<dyn Error>> {
    driver.goto(&format!("{}/admin/", server.url())).await?;

    let username_form = driver.find(Locator::Id("username")).await?;
    username_form.send_keys(username).await?;
    let password_form = driver.find(Locator::Id("password")).await?;
    password_form.send_keys(password).await?;
    let submit_button = driver.find(Locator::Css("button[type=submit]")).await?;
    submit_button.click().await?;

    Ok(())
}

async fn create_webdriver() -> Result<Client, Box<dyn Error>> {
    Ok(ClientBuilder::native()
        .connect("http://localhost:4444")
        .await?)
}
