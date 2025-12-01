use bytes::Bytes;
use cot::config::ProjectConfig;
use cot::html::Html;
use cot::project::RegisterAppsContext;
use cot::request::Request;
use cot::router::{Route, Router};
use cot::test::Client;
use cot::{App, AppBuilder, Project, StatusCode, reverse};

#[cot::test]
#[cfg_attr(
    miri,
    ignore = "unsupported operation: can't call foreign function `sqlite3_open_v2`"
)]
async fn cot_project_router_sub_path() {
    async fn hello(_request: Request) -> Html {
        Html::new("OK")
    }

    struct App1;
    impl App for App1 {
        fn name(&self) -> &'static str {
            "app1"
        }

        fn router(&self) -> Router {
            Router::with_urls([Route::with_handler_and_name("/index", hello, "index")])
        }
    }

    struct App2;
    impl App for App2 {
        fn name(&self) -> &'static str {
            "app2"
        }

        fn router(&self) -> Router {
            Router::with_urls([Route::with_handler_and_name("/hello", hello, "index")])
        }
    }

    struct TestProject;
    impl Project for TestProject {
        fn config(&self, config_name: &str) -> cot::Result<ProjectConfig> {
            assert!(
                (config_name == "test"),
                "unexpected config name: {config_name}"
            );
            Ok(ProjectConfig::default())
        }

        fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
            apps.register_with_views(App1, "");
            apps.register_with_views(App2, "/app");
        }
    }

    let mut client = Client::new(TestProject).await;

    let response = client.get("/app/hello").await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[cot::test]
#[cfg_attr(
    miri,
    ignore = "unsupported operation: can't call foreign function `sqlite3_open_v2`"
)]
async fn cot_router_reverse_local() {
    async fn get_index(request: Request) -> cot::Result<Html> {
        let reversed = reverse!(request, "index")?;
        Ok(Html::new(reversed))
    }

    struct App1;
    impl App for App1 {
        fn name(&self) -> &'static str {
            "app1"
        }

        fn router(&self) -> Router {
            Router::with_urls([Route::with_handler_and_name("/index1", get_index, "index")])
        }
    }

    struct App2;
    impl App for App2 {
        fn name(&self) -> &'static str {
            "app2"
        }

        fn router(&self) -> Router {
            Router::with_urls([Route::with_handler_and_name("/index2", get_index, "index")])
        }
    }

    struct TestProject;
    impl Project for TestProject {
        fn config(&self, _config_name: &str) -> cot::Result<ProjectConfig> {
            Ok(ProjectConfig::default())
        }

        fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
            apps.register_with_views(App1, "");
            apps.register_with_views(App2, "");
        }
    }

    let mut client = Client::new(TestProject).await;

    let response = client.get("/index1").await.unwrap();
    assert_eq!(
        response.into_body().into_bytes().await.unwrap(),
        Bytes::from("/index1")
    );

    let response = client.get("/index2").await.unwrap();
    assert_eq!(
        response.into_body().into_bytes().await.unwrap(),
        Bytes::from("/index2")
    );
}
