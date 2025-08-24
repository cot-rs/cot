use std::env;
use std::path::PathBuf;

use cot::config::{
    CacheUrl, DatabaseConfig, MiddlewareConfig, ProjectConfig, SessionMiddlewareConfig,
    SessionStoreConfig, SessionStoreTypeConfig,
};
use cot::middleware::SessionMiddleware;
use cot::project::{RegisterAppsContext, WithDatabase};
use cot::response::Response;
use cot::session::Session;
use cot::test::TestRequestBuilder;
use cot::{AppBuilder, Body, Bootstrapper, Error, Project, ProjectContext};
use http::Request;
use tower::{Layer, Service, ServiceExt};

async fn create_svc_and_call_with_req(context: &ProjectContext<WithDatabase>) {
    let store = SessionMiddleware::from_context(context);
    let svc = tower::service_fn(|req: Request<Body>| async move {
        assert!(req.extensions().get::<Session>().is_some());
        Ok::<_, Error>(Response::new(Body::empty()))
    });
    let mut svc = store.layer(svc);
    let request = TestRequestBuilder::get("/").build();
    svc.ready().await.unwrap().call(request).await.unwrap();
}

fn create_project_config(store: SessionStoreTypeConfig) -> ProjectConfig {
    let mut project = ProjectConfig::builder();
    let mut project = match store {
        SessionStoreTypeConfig::Database => project.database(
            DatabaseConfig::builder()
                .url("sqlite::memory:".to_string())
                .build(),
        ),
        _ => &mut project,
    };

    project
        .middlewares(
            MiddlewareConfig::builder()
                .session(
                    SessionMiddlewareConfig::builder()
                        .store(SessionStoreConfig::builder().store_type(store).build())
                        .build(),
                )
                .build(),
        )
        .build()
}

struct TestProject;

impl Project for TestProject {
    fn register_apps(&self, _apps: &mut AppBuilder, _context: &RegisterAppsContext) {}
}

#[cot::test]
async fn memory_store_factory_produces_working_store() {
    let config = create_project_config(SessionStoreTypeConfig::Memory);
    let bootstrapper = Bootstrapper::new(TestProject)
        .with_config(config)
        .with_apps()
        .with_database()
        .await
        .expect("bootstrap failed");
    let context = bootstrapper.context();

    create_svc_and_call_with_req(context).await;
}

#[cfg(feature = "json")]
#[cot::test]
async fn session_middleware_file_config_to_session_store() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path: PathBuf = dir.path().to_path_buf();
    let config = create_project_config(SessionStoreTypeConfig::File { path });

    let bootstrapper = Bootstrapper::new(TestProject)
        .with_config(config)
        .with_apps()
        .with_database()
        .await
        .expect("bootstrap failed");
    let context = bootstrapper.context();

    create_svc_and_call_with_req(context).await;
}

#[cfg(all(feature = "cache", feature = "redis"))]
#[cot::test]
#[ignore = "requires external Redis service"]
async fn session_middleware_redis_config_to_session_store() {
    let redis_url = env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let uri = CacheUrl::from(redis_url);
    let config = create_project_config(SessionStoreTypeConfig::Cache { uri });
    let bootstrapper = Bootstrapper::new(TestProject)
        .with_config(config)
        .with_apps()
        .with_database()
        .await
        .expect("bootstrap failed");
    let context = bootstrapper.context();

    create_svc_and_call_with_req(context).await;
}

#[cfg(all(feature = "db", feature = "json"))]
#[cot::test]
#[cfg_attr(
    miri,
    ignore = "unsupported operation: can't call foreign function `sqlite3_open_v2`"
)]
async fn session_middleware_database_config_to_session_store() {
    let config = create_project_config(SessionStoreTypeConfig::Database);
    let bootstrapper = Bootstrapper::new(TestProject)
        .with_config(config)
        .with_apps()
        .with_database()
        .await
        .expect("bootstrap failed");
    let context = bootstrapper.context();

    create_svc_and_call_with_req(context).await;
}
