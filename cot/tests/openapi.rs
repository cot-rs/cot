use aide::openapi::{Parameter, PathItem, ReferenceOr};
use cot::openapi::{AsApiRoute, NoApi, RouteContext};
use cot::request::extractors::{Json, Path, UrlQuery};
use cot::response::{Response, ResponseExt};
use cot::router::method::openapi::{ApiMethodRouter, api_get, api_post};
use cot::router::{Route, Router};
use cot::test::TestRequestBuilder;
use cot::{Body, RequestHandler, StatusCode};
use schemars::SchemaGenerator;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, schemars::JsonSchema)]
struct TestRequest {
    field1: String,
    field2: i32,
    optional_field: Option<bool>,
}

#[derive(Serialize, schemars::JsonSchema)]
struct TestResponse {
    result: String,
}

async fn test_handler() -> cot::Result<Response> {
    Ok(Response::new_html(StatusCode::OK, Body::fixed("test")))
}

async fn test_json_handler(Json(req): Json<TestRequest>) -> cot::Result<Response> {
    Response::new_json(
        StatusCode::OK,
        &TestResponse {
            result: format!("Got: {}, {}", req.field1, req.field2),
        },
    )
}

async fn test_path_handler(Path(id): Path<i32>) -> cot::Result<Response> {
    Ok(Response::new_html(
        StatusCode::OK,
        Body::fixed(format!("ID: {id}")),
    ))
}

async fn test_query_handler(UrlQuery(query): UrlQuery<TestRequest>) -> cot::Result<Response> {
    Ok(Response::new_html(
        StatusCode::OK,
        Body::fixed(format!("Query: {}, {}", query.field1, query.field2)),
    ))
}

#[cot::test]
async fn api_route_integration() {
    let router = Router::with_urls([
        Route::with_api_handler("/test", api_get(test_handler)),
        Route::with_api_handler("/json", api_post(test_json_handler)),
        Route::with_api_handler("/path/{id}", api_get(test_path_handler)),
        Route::with_api_handler_and_name("/query", api_get(test_query_handler), "query"),
    ]);

    // Test the OpenAPI data
    let aide::openapi::Paths {
        paths: api_spec, ..
    } = router.as_api();

    assert!(api_spec.contains_key("/test"));
    assert!(api_spec.contains_key("/json"));
    assert!(api_spec.contains_key("/path/{id}"));
    assert!(api_spec.contains_key("/query"));

    assert!(matches!(
        api_spec.get("/test"),
        Some(ReferenceOr::Item(PathItem { get: Some(_), .. }))
    ));
    assert!(matches!(
        api_spec.get("/json"),
        Some(ReferenceOr::Item(PathItem { post: Some(_), .. }))
    ));

    if let Some(ReferenceOr::Item(PathItem {
        get: Some(operation),
        ..
    })) = api_spec.get("/path/{id}")
    {
        assert_eq!(operation.parameters.len(), 1);
        if let ReferenceOr::Item(Parameter::Path { parameter_data, .. }) = &operation.parameters[0]
        {
            assert_eq!(parameter_data.name, "id");
        } else {
            panic!("Expected path parameter");
        }
    } else {
        panic!("Expected GET operation for /path/{{id}}");
    }

    if let Some(ReferenceOr::Item(PathItem {
        get: Some(operation),
        ..
    })) = api_spec.get("/query")
    {
        assert_eq!(operation.parameters.len(), 3); // field1, field2, optional_field

        for param in &operation.parameters {
            assert!(matches!(param, ReferenceOr::Item(Parameter::Query { .. })));
        }
    } else {
        panic!("Expected GET operation for /query");
    }

    // Test the API routes
    let request = TestRequestBuilder::get("/test").build();
    let response = router.handle(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.into_body().into_bytes().await.unwrap(), "test");

    let request = TestRequestBuilder::get("/path/123").build();
    let response = router.handle(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.into_body().into_bytes().await.unwrap(), "ID: 123");
}

#[test]
fn api_router_nested() {
    let router = Router::with_urls([Route::with_api_handler(
        "/test",
        ApiMethodRouter::new().get(test_handler),
    )]);
    let nested_router = Router::with_urls(vec![Route::with_router("/b", router)]);
    let root_router = Router::with_urls(vec![Route::with_router("/a", nested_router)]);

    let aide::openapi::Paths {
        paths: api_spec, ..
    } = root_router.as_api();

    assert!(matches!(
        api_spec.get("/a/b/test"),
        Some(ReferenceOr::Item(PathItem { get: Some(_), .. }))
    ));
}

#[test]
fn api_method_router() {
    let router = ApiMethodRouter::new()
        .get(test_handler)
        .post(test_json_handler);

    let route_context = RouteContext::new();
    let mut schema_generator = SchemaGenerator::default();

    let path_item = router.as_api_route(&route_context, &mut schema_generator);

    assert!(path_item.get.is_some());
    assert!(path_item.post.is_some());

    assert!(path_item.put.is_none());
    assert!(path_item.delete.is_none());
    assert!(path_item.options.is_none());
    assert!(path_item.head.is_none());
    assert!(path_item.patch.is_none());
    assert!(path_item.trace.is_none());

    if let Some(operation) = path_item.post {
        assert!(operation.request_body.is_some());
    }
}

#[cot::test]
async fn no_api_in_method_router() {
    let router = ApiMethodRouter::new()
        .get(test_handler)
        .post(NoApi(test_json_handler));

    let route_context = RouteContext::new();
    let mut schema_generator = SchemaGenerator::default();

    let path_item = router.as_api_route(&route_context, &mut schema_generator);

    assert!(path_item.get.is_some());
    assert!(path_item.post.is_none());

    // Test the API routes
    let request = TestRequestBuilder::post("/test")
        .json(&TestRequest {
            field1: "test".to_string(),
            field2: 42,
            optional_field: Some(true),
        })
        .build();
    let response = router.handle(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[cot::test]
async fn no_api_in_params() {
    async fn noapi_handler(
        NoApi(Path(id)): NoApi<Path<i32>>,
        NoApi(Json(req)): NoApi<Json<TestRequest>>,
    ) -> cot::Result<Response> {
        Ok(Response::new_html(
            StatusCode::OK,
            Body::fixed(format!("Got: {id}, {}, {}", req.field1, req.field2)),
        ))
    }

    let router = Router::with_urls([Route::with_api_handler(
        "/test/{id}",
        api_post(noapi_handler),
    )]);

    let request = TestRequestBuilder::post("/test/123")
        .json(&TestRequest {
            field1: "test".to_string(),
            field2: 42,
            optional_field: Some(true),
        })
        .build();
    let response = router.handle(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().into_bytes().await.unwrap();
    assert_eq!(body, "Got: 123, test, 42");
}
