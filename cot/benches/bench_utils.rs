//! Cot application benchmarking utilities using Criterion.
//!
//! This module provides scaffolding for benchmarking Cot-based web servers,
//! supporting both independent benchmarks and grouped benchmarks.
//!
//! **Note:** Requires the `test` feature to be enabled.
//! Run with: `cargo bench --features test`

use cot::cli::CliMetadata;
use cot::config::ProjectConfig;
use cot::project::RegisterAppsContext;
use cot::router::Router;
use cot::{App, AppBuilder, Project};
use criterion::{Criterion, Throughput};
use futures_util::future::join_all;
use reqwest::{Client, Request};

pub(crate) struct ProjectBenchmarkBuilder<'a> {
    name: &'static str,
    criterion: &'a mut Criterion,
    method: reqwest::Method,
    path: Option<&'static str>,
    body: Option<String>,
    content_type: Option<&'static str>,
    expected_status_code: reqwest::StatusCode,
    requests_per_iteration: u64,
}

macro_rules! builder_method {
    ($name:ident, $ty:ty) => {
        #[must_use]
        pub(crate) fn $name(mut self, $name: $ty) -> Self {
            self.$name = $name;
            self
        }
    };
    ($name:ident, $ty:ty, opt) => {
        #[must_use]
        pub(crate) fn $name(mut self, $name: $ty) -> Self {
            self.$name = Some($name);
            self
        }
    };
}

#[allow(clippy::allow_attributes, reason = "For the dead_code allowance")]
#[allow(
    dead_code,
    reason = "Clippy warns about unused code despite using it in benchmarks"
)]
pub(crate) fn bench<'a>(
    criterion: &'a mut Criterion,
    name: &'static str,
) -> ProjectBenchmarkBuilder<'a> {
    ProjectBenchmarkBuilder::new(criterion, name)
}

#[allow(clippy::allow_attributes, reason = "For the dead_code allowance")]
#[allow(dead_code, reason = "For the benchmark functions not used yet")]
impl<'a> ProjectBenchmarkBuilder<'a> {
    pub(crate) fn new(criterion: &'a mut Criterion, name: &'static str) -> Self {
        Self {
            name,
            criterion,
            method: reqwest::Method::GET,
            path: None,
            body: None,
            content_type: None,
            expected_status_code: reqwest::StatusCode::OK,
            requests_per_iteration: 50,
        }
    }

    builder_method!(path, &'static str, opt);
    builder_method!(body, String, opt);
    builder_method!(content_type, &'static str, opt);
    builder_method!(requests_per_iteration, u64);
    builder_method!(method, reqwest::Method);
    builder_method!(expected_status_code, reqwest::StatusCode);

    pub(crate) fn plain_body<T: ToString>(mut self, body: &T) -> Self {
        self.body = Some(body.to_string());
        self.content_type = Some("text/plain");
        self
    }

    pub(crate) fn json_body<T: serde::Serialize>(mut self, body: &T) -> Self {
        self.body = Some(serde_json::to_string(body).expect("Failed to serialize JSON"));
        self.content_type = Some("application/json");
        self
    }

    pub(crate) fn form_body<T: serde::Serialize>(mut self, body: &T) -> Self {
        self.body = Some(serde_urlencoded::to_string(body).expect("Failed to serialize form"));
        self.content_type = Some("application/x-www-form-urlencoded");
        self
    }

    pub(crate) fn run_with_router<F>(self, f: F)
    where
        F: FnOnce() -> Router,
    {
        let project = SimpleCotProject::new(self.name, f());
        self.run_with_project(project);
    }

    pub(crate) fn run_with_project<P>(self, project: P)
    where
        P: Project + Send + 'static,
    {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let _server_handle = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to create tokio runtime");

            let config = project
                .config("benchmark")
                .expect("Failed to get project config");

            runtime.block_on(async move {
                let bootstrapper = cot::project::Bootstrapper::new(project)
                    .with_config(config)
                    .boot()
                    .await
                    .expect("Failed to create bootstrapper");
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                    .await
                    .expect("Failed to bind TCP listener");
                let addr = listener.local_addr().expect("Failed to get local address");
                tx.send(addr)
                    .expect("Failed to send address to main thread");

                cot::project::run_at(bootstrapper, listener).await
            })
        });

        let local_port = rx
            .recv()
            .expect("Failed to receive address from server thread")
            .port();

        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime");

        let client = Client::new();
        let base_url = format!("http://127.0.0.1:{local_port}");
        let request = self.build_request(&client, &base_url);

        self.criterion
            .benchmark_group(self.name)
            .throughput(Throughput::Elements(self.requests_per_iteration))
            .bench_function(self.name, |b| {
                b.to_async(&runtime).iter(|| {
                    let futures: Vec<_> = (0..self.requests_per_iteration)
                        .map(|_| {
                            let request = request.try_clone().expect("Failed to clone request");
                            client.execute(request)
                        })
                        .collect();

                    async move {
                        let outputs = join_all(futures);
                        for response in outputs.await {
                            let response = response.expect("Failed to execute request");
                            assert_eq!(response.status(), self.expected_status_code);
                        }
                    }
                });
            });
    }

    fn build_request(&self, client: &Client, base_url: &str) -> Request {
        let request_builder = client.request(
            self.method.clone(),
            format!("{}{}", base_url, self.path.unwrap_or("/")),
        );

        let request_builder = if let Some(body) = &self.body {
            request_builder.body(body.clone())
        } else {
            request_builder
        };

        let request_builder = if let Some(content_type) = self.content_type {
            request_builder.header("Content-Type", content_type)
        } else {
            request_builder
        };

        request_builder.build().expect("Failed to build request")
    }
}

/// A simple Cot project implementation for benchmarking purposes
#[derive(Debug)]
pub(crate) struct SimpleCotProject {
    name: &'static str,
    router: Router,
}

impl SimpleCotProject {
    pub(crate) fn new(name: &'static str, router: Router) -> Self {
        Self { name, router }
    }
}

impl Project for SimpleCotProject {
    fn cli_metadata(&self) -> CliMetadata {
        CliMetadata {
            name: self.name,
            version: "0.1.0",
            authors: "benchmark",
            description: "Benchmark application",
        }
    }

    fn config(&self, _config_name: &str) -> cot::Result<ProjectConfig> {
        Ok(ProjectConfig::dev_default())
    }

    fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
        apps.register_with_views(
            SimpleApp {
                router: self.router.clone(),
            },
            "",
        );
    }
}

struct SimpleApp {
    router: Router,
}

impl App for SimpleApp {
    fn name(&self) -> &'static str {
        "benchmark_app"
    }

    fn router(&self) -> Router {
        self.router.clone()
    }
}
