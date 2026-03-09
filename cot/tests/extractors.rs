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
