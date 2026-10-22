use cot::request::RequestExt;
use cot::test::TestRequestBuilder;

#[cfg(feature = "cache")]
#[cot::test]
async fn request_cache() {
    let mut request_builder = TestRequestBuilder::get("/");
    let mut request = request_builder.build();
    let request_cache: cot::cache::Cache = request.extract_from_head().await.unwrap();

    // this will use the default in-memory cache
    request_cache
        .insert("user:1", serde_json::json!({"name": "Alice"}))
        .await
        .unwrap();

    let user: Option<serde_json::Value> = request_cache.get("user:1").await.unwrap();
    assert!(user.is_some());
    assert_eq!(user.unwrap()["name"], "Alice");
}

#[cfg(feature = "email")]
#[cot::test]
async fn request_email() {
    use cot::common_types::Email;
    use cot::email::EmailMessage;

    let mut request_builder = TestRequestBuilder::get("/");
    let mut request = request_builder.build();
    let email_service: cot::email::Email = request.extract_from_head().await.unwrap();

    let message = EmailMessage::builder()
        .from(Email::new("sender@example.com").unwrap())
        .to(vec![Email::new("recipient@example.com").unwrap()])
        .subject("Test Email")
        .body("Hello, this is a test email.")
        .build()
        .unwrap();

    assert!(email_service.send(message).await.is_ok());
}

#[cot::e2e_test]
#[cfg_attr(
    miri,
    ignore = "unsupported operation: setsockopt: option 0x4 is unsupported for level IPPROTO_TCP"
)]
async fn client_ip_addr() {
    use cot::cli::CliMetadata;
    use cot::config::ProjectConfig;
    use cot::html::Html;
    use cot::project::RegisterAppsContext;
    use cot::request::extractors::ClientIpAddr;
    use cot::router::{Route, Router};
    use cot::test::TestServerBuilder;
    use cot::{App, AppBuilder, Project};

    async fn client_ip(client_ip: ClientIpAddr) -> Html {
        Html::new(client_ip.ip().to_string())
    }

    struct ClientIpApp;
    impl App for ClientIpApp {
        fn name(&self) -> &'static str {
            "client_ip"
        }

        fn router(&self) -> Router {
            Router::with_urls([Route::with_handler("/", client_ip)])
        }
    }

    struct ClientIpProject;
    impl Project for ClientIpProject {
        fn cli_metadata(&self) -> CliMetadata {
            cot::cli::metadata!()
        }

        fn config(&self, _config_name: &str) -> cot::Result<ProjectConfig> {
            ProjectConfig::from_toml(
                r#"
                [client_ip]
                proxies = ["127.0.0.1/32"]
                "#,
            )
        }

        fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
            apps.register_with_views(ClientIpApp, "");
        }
    }

    let server = TestServerBuilder::new(ClientIpProject).start().await;
    let response = reqwest::Client::new()
        .get(server.url())
        .header("X-Forwarded-For", "6.6.6.6, 203.0.113.1")
        .send()
        .await
        .unwrap();

    assert_eq!(response.text().await.unwrap(), "203.0.113.1");
    server.close().await;
}
