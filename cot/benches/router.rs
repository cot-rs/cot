use cot::router::*;
use criterion::{Criterion, criterion_group, criterion_main};

pub fn criterion_benchmark(c: &mut Criterion) {
    let mut router_group = c.benchmark_group("router construction");

    router_group.bench_function("empty", |b| b.iter(|| Router::empty()));
    router_group.bench_function("single root route", |b| {
        b.iter(|| Router::with_urls([Route::with_handler("/", async || "Hello Cot")]))
    });
    router_group.bench_function("empty nested router", |b| {
        b.iter(|| Router::with_urls([Route::with_router("/", Router::empty())]))
    });
    router_group.bench_function("multiple nested routers", |b| {
        b.iter(|| {
            let c = Router::with_urls([Route::with_handler("/c", async || "Hello Cot")]);
            let b = Router::with_urls([Route::with_router("/b", c)]);
            let _a = Router::with_urls([Route::with_router("/a", b)]);
        })
    });

    let mut handle_group = c.benchmark_group("router request handling");
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
