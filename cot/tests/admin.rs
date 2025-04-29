use std::marker::PhantomData;
use std::net::SocketAddr;

use async_trait::async_trait;
use cot::admin::AdminApp;
use cot::auth::db::{DatabaseUser, DatabaseUserApp};
use cot::cli::CliMetadata;
use cot::config::{
    AuthBackendConfig, DatabaseConfig, MiddlewareConfig, ProjectConfig, SessionMiddlewareConfig,
};
use cot::middleware::{AuthMiddleware, SessionMiddleware};
use cot::project::{MiddlewareContext, RegisterAppsContext, run_at_with_shutdown};
use cot::static_files::StaticFilesMiddleware;
use cot::{App, AppBuilder, Bootstrapper, BoxedHandler, Project, ProjectContext};
use fantoccini::{ClientBuilder, Locator};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

struct HelloApp;

#[async_trait]
impl App for HelloApp {
    fn name(&self) -> &'static str {
        env!("CARGO_PKG_NAME")
    }

    async fn init(&self, context: &mut ProjectContext) -> cot::Result<()> {
        DatabaseUser::create_user(context.database(), "admin", "admin").await?;

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
        apps.register_with_views(HelloApp, "");
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
async fn admin_e2e_login() -> Result<(), Box<dyn std::error::Error>> {
    let server = TestServer::new(AdminProject).start().await;

    let driver = ClientBuilder::native()
        .connect("http://localhost:4444")
        .await?;
    driver.goto(&format!("{}/admin/", server.url())).await?;
    let username_form = driver.find(Locator::Id("username")).await?;
    username_form.send_keys("admin").await?;
    let password_form = driver.find(Locator::Id("password")).await?;
    password_form.send_keys("admin").await?;
    let submit_button = driver.find(Locator::Css("button[type=submit]")).await?;
    submit_button.click().await?;

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

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TestServer<T> {
    project: T,
}

impl<T: Project + 'static> TestServer<T> {
    pub fn new(project: T) -> Self {
        Self { project }
    }

    async fn start(self) -> TestServerRunning<T> {
        TestServerRunning::start(self.project).await
    }
}

#[must_use = "TestServerRunning must be used to close the server"]
#[derive(Debug)]
pub struct TestServerRunning<T> {
    address: SocketAddr,
    channel_send: oneshot::Sender<()>,
    server_handle: tokio::task::JoinHandle<()>,
    project: PhantomData<fn() -> T>,
}

impl<T: Project + 'static> TestServerRunning<T> {
    async fn start(project: T) -> Self {
        let tcp_listener = TcpListener::bind("0.0.0.0:0").await.unwrap();
        let address = tcp_listener.local_addr().unwrap();

        let (send, recv) = oneshot::channel::<()>();

        let server_handle = tokio::task::spawn_local(async move {
            let bootstrapper = Bootstrapper::new(project)
                .with_config_name("test")
                .unwrap()
                .boot()
                .await
                .unwrap();
            run_at_with_shutdown(bootstrapper, tcp_listener, async move {
                recv.await.unwrap();
            })
            .await
            .unwrap();
        });

        Self {
            address,
            channel_send: send,
            server_handle,
            project: PhantomData,
        }
    }

    #[must_use]
    pub fn url(&self) -> String {
        if let Ok(host) = std::env::var("COT_TEST_SERVER_HOST") {
            format!("http://{}:{}", host, self.address.port())
        } else {
            format!("http://{}", self.address)
        }
    }

    pub async fn close(self) {
        self.channel_send.send(()).unwrap();
        self.server_handle.await.unwrap();
    }
}
