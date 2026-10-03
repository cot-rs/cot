use bytes::Bytes;
use cot::config::ProjectConfig;
use cot::html::Html;
use cot::project::RegisterAppsContext;
use cot::request::{Request, RequestExt};
use cot::router::{Route, Router};
use cot::test::Client;
use cot::{App, AppBuilder, Project, StatusCode};

async fn index() -> Html {
    Html::new("Hello world!")
}

async fn parameterized(request: Request) -> Html {
    let name = request.path_params().get("name").unwrap().to_owned();
    Html::new(name)
}

async fn multi_param(request: Request) -> Html {
    let id = request.path_params().get("id").unwrap().to_owned();
    let post_id = request.path_params().get("post_id").unwrap().to_owned();
    Html::new(format!("{id}/{post_id}"))
}

async fn catch_all(request: Request) -> Html {
    let path = request.path_params().get("path").unwrap().to_owned();
    Html::new(path)
}

async fn nested(request: Request) -> Html {
    let id = request.path_params().get("id").unwrap().to_owned();
    Html::new(format!("nested/{id}"))
}

#[cot::test]
#[cfg_attr(
    miri,
    ignore = "unsupported operation: can't call foreign function `sqlite3_open_v2`"
)]
async fn test_index() {
    let client = Client::new(project());

    let response = client.await.get("/").await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.into_body().into_bytes().await.unwrap(),
        Bytes::from("Hello world!")
    );
}

#[cot::test]
#[cfg_attr(
    miri,
    ignore = "unsupported operation: can't call foreign function `sqlite3_open_v2`"
)]
async fn path_params() {
    let client = Client::new(project());

    let response = client.await.get("/get/John").await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.into_body().into_bytes().await.unwrap(),
        Bytes::from("John")
    );
}

#[cot::test]
#[cfg_attr(
    miri,
    ignore = "unsupported operation: can't call foreign function `sqlite3_open_v2`"
)]
async fn multi_path_params() {
    let client = Client::new(project());

    let response = client.await.get("/multi/1/posts/2").await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.into_body().into_bytes().await.unwrap(),
        Bytes::from("1/2")
    );
}

#[cot::test]
#[cfg_attr(
    miri,
    ignore = "unsupported operation: can't call foreign function `sqlite3_open_v2`"
)]
async fn wildcard_catch_all() {
    let client = Client::new(project());

    let response = client.await.get("/static/css/app.css").await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.into_body().into_bytes().await.unwrap(),
        Bytes::from("css/app.css")
    );
}

#[cot::test]
#[cfg_attr(
    miri,
    ignore = "unsupported operation: can't call foreign function `sqlite3_open_v2`"
)]
async fn nested_router() {
    let client = Client::new(project());

    let response = client.await.get("/nested/inner/42").await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.into_body().into_bytes().await.unwrap(),
        Bytes::from("nested/42")
    );
}

#[cot::test]
#[cfg_attr(
    miri,
    ignore = "unsupported operation: can't call foreign function `sqlite3_open_v2`"
)]
async fn unmatched_path_returns_404() {
    let client = Client::new(project());

    let response = client.await.get("/does-not-exist").await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[cot::test]
#[cfg_attr(
    miri,
    ignore = "unsupported operation: can't call foreign function `sqlite3_open_v2`"
)]
async fn static_route_priority_over_dynamic() {
    let client = Client::new(project());

    let response = client.await.get("/get/new").await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.into_body().into_bytes().await.unwrap(),
        Bytes::from("new")
    );
}

#[must_use]
fn project() -> impl Project {
    struct RouterApp;
    impl App for RouterApp {
        fn name(&self) -> &'static str {
            "router-app"
        }

        fn router(&self) -> Router {
            let nested_router = Router::with_urls([Route::with_handler_and_name(
                "/inner/{id}",
                nested,
                "nested",
            )]);

            Router::with_urls([
                Route::with_handler_and_name("/", index, "index"),
                Route::with_handler_and_name("/get/{name}", parameterized, "parameterized"),
                Route::with_handler_and_name(
                    "/multi/{id}/posts/{post_id}",
                    multi_param,
                    "multi_param",
                ),
                Route::with_handler_and_name("/static/{*path}", catch_all, "catch_all"),
                Route::with_router("/nested", nested_router),
            ])
        }
    }

    struct TestProject;
    impl Project for TestProject {
        fn config(&self, _config_name: &str) -> cot::Result<ProjectConfig> {
            Ok(ProjectConfig::default())
        }

        fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
            apps.register_with_views(RouterApp, "");
        }
    }

    TestProject
}
