use criterion::{Criterion, criterion_group, criterion_main};
use serde::{Deserialize, Serialize};

mod bench_utils;
use bench_utils::bench;
use cot::json::Json;
use cot::router::{Route, Router};

async fn hello_world() -> &'static str {
    "Hello, World!"
}

// JSON API endpoint
#[derive(Debug, Serialize, Deserialize)]
struct AddRequest {
    a: i32,
    b: i32,
}

#[derive(Debug, Serialize, Deserialize)]
struct AddResponse {
    result: i32,
}

async fn add_numbers(Json(req): Json<AddRequest>) -> Json<AddResponse> {
    Json(AddResponse {
        result: req.a + req.b,
    })
}

pub fn criterion_benchmark(c: &mut Criterion) {
    bench(c, "empty_router")
        .expected_status_code(reqwest::StatusCode::NOT_FOUND)
        .run_with_router(Router::empty);

    bench(c, "single_root_route")
        .path("/")
        .run_with_router(|| Router::with_urls([Route::with_handler("/", hello_world)]));

    bench(c, "single_root_route_burst")
        .path("/")
        .requests_per_iteration(1000)
        .run_with_router(|| Router::with_urls([Route::with_handler("/", hello_world)]));

    bench(c, "nested_routers")
        .path("/a/b/c/d/e/f/g")
        .run_with_router(|| {
            Router::with_urls([Route::with_router(
                "/a",
                Router::with_urls([Route::with_router(
                    "/b",
                    Router::with_urls([Route::with_router(
                        "/c",
                        Router::with_urls([Route::with_router(
                            "/d",
                            Router::with_urls([Route::with_router(
                                "/e",
                                Router::with_urls([Route::with_router(
                                    "/f",
                                    Router::with_urls([Route::with_handler("/g", hello_world)]),
                                )]),
                            )]),
                        )]),
                    )]),
                )]),
            )])
        });

    bench(c, "json_api")
        .path("/")
        .method(reqwest::Method::POST)
        .json_body(&AddRequest { a: 10, b: 20 })
        .run_with_router(|| Router::with_urls([Route::with_handler("/", add_numbers)]));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
