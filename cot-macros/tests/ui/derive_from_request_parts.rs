use cot::axum::extract::RequestParts;
use cot::http::Request;
use cot::lib::FromRequestParts;

#[tokio::test]
async fn test_derive_from_request_parts_success() {
    #[derive(FromRequestParts, Debug, PartialEq)]
    struct TestContext {
        user_id: i32,
        username: String,
    }

    let mut parts = RequestParts::new(Request::default());
    parts.extensions.insert(10_i32);
    parts.extensions.insert("test_user".to_string());

    let context = TestContext::from_request_parts(&mut parts).await.unwrap();

    assert_eq!(
        context,
        TestContext {
            user_id: 10,
            username: "test_user".to_string()
        }
    );
}

#[tokio::test]
async fn test_derive_from_request_parts_different_types() {
    #[derive(FromRequestParts, Debug, PartialEq)]
    struct TestContext {
        value_i64: i64,
        value_bool: bool,
        value_string: String,
    }

    let mut parts = RequestParts::new(Request::default());
    parts.extensions.insert(12345_i64);
    parts.extensions.insert(true);
    parts.extensions.insert("another_test".to_string());

    let context = TestContext::from_request_parts(&mut parts).await.unwrap();

    assert_eq!(
        context,
        TestContext {
            value_i64: 12345,
            value_bool: true,
            value_string: "another_test".to_string()
        }
    );
}

#[tokio::test]
async fn test_derive_from_request_parts_missing_extension() {
    #[derive(FromRequestParts, Debug)]
    struct TestContext {
        missing_value: i32,
    }

    let mut parts = RequestParts::new(Request::default());

    let result = TestContext::from_request_parts(&mut parts).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_derive_from_request_parts_empty_struct() {
    #[derive(FromRequestParts, Debug, PartialEq)]
    struct EmptyContext {}

    let mut parts = RequestParts::new(Request::default());
    let context = EmptyContext::from_request_parts(&mut parts).await.unwrap();
    assert_eq!(EmptyContext {}, context);
}

#[tokio::test]
async fn test_derive_from_request_parts_tuple_struct() {
    #[derive(FromRequestParts, Debug, PartialEq)]
    struct BaseContext(i32, String);

    let mut parts = RequestParts::new(Request::default());
    parts.extensions.insert(42_i32);
    parts.extensions.insert("tuple_test".to_string());

    let context = BaseContext::from_request_parts(&mut parts).await.unwrap();
    assert_eq!(context, BaseContext(42, "tuple_test".to_string()));
}

#[tokio::test]
async fn test_derive_from_request_parts_tuple_struct_multiple_types() {
    #[derive(FromRequestParts, Debug, PartialEq)]
    struct MultiContext(i64, bool, String, i32);

    let mut parts = RequestParts::new(Request::default());
    parts.extensions.insert(999_i64);
    parts.extensions.insert(false);
    parts.extensions.insert("multi_tuple".to_string());
    parts.extensions.insert(123_i32);

    let context = MultiContext::from_request_parts(&mut parts).await.unwrap();
    assert_eq!(
        context,
        MultiContext(999, false, "multi_tuple".to_string(), 123)
    );
}

#[tokio::test]
async fn test_derive_from_request_parts_tuple_struct_missing_extension() {
    #[derive(FromRequestParts, Debug)]
    struct IncompleteContext(i32, String);

    let mut parts = RequestParts::new(Request::default());
    // Only insert one of the required types
    parts.extensions.insert(100_i32);
    // Missing String extension

    let result = IncompleteContext::from_request_parts(&mut parts).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_derive_from_request_parts_single_field_tuple() {
    #[derive(FromRequestParts, Debug, PartialEq)]
    struct SingleContext(String);

    let mut parts = RequestParts::new(Request::default());
    parts.extensions.insert("single_field".to_string());

    let context = SingleContext::from_request_parts(&mut parts).await.unwrap();
    assert_eq!(context, SingleContext("single_field".to_string()));
}

#[tokio::test]
async fn test_derive_from_request_parts_unit_struct() {
    #[derive(FromRequestParts, Debug, PartialEq)]
    struct UnitContext;

    let mut parts = RequestParts::new(Request::default());

    let context = UnitContext::from_request_parts(&mut parts).await.unwrap();
    assert_eq!(context, UnitContext);
}
