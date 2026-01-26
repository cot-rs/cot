use std::any::Any;
use std::panic::PanicHookInfo;
use std::sync::Arc;

use cot_core::error::backtrace::{__cot_create_backtrace, Backtrace};
use tracing::{Level, error, warn};

use crate::config::ProjectConfig;
use crate::error::NotFound;
use crate::router::Router;
use crate::{Error, Result, StatusCode, Template};

#[derive(Debug)]
pub(super) struct Diagnostics {
    project_config: ProjectConfig,
    router: Arc<Router>,
    request_head: Option<crate::request::RequestHead>,
}

impl Diagnostics {
    #[must_use]
    pub(super) fn new(
        project_config: ProjectConfig,
        router: Arc<Router>,
        request_head: Option<crate::request::RequestHead>,
    ) -> Self {
        Self {
            project_config,
            router,
            request_head,
        }
    }
}

#[derive(Debug, Template)]
#[template(path = "error.html")]
struct ErrorPageTemplate {
    kind: Kind,
    error_message: Option<String>,
    panic_string: Option<String>,
    panic_location: Option<String>,
    backtrace: Option<Backtrace>,
    error_data: Vec<ErrorData>,
    route_data: Vec<RouteData>,
    request_data: Option<RequestData>,
    project_config: String,
}

fn error_css() -> &'static str {
    include_str!(concat!(env!("OUT_DIR"), "/templates/css/error.css"))
}

#[derive(Debug, Default, Clone)]
struct ErrorPageTemplateBuilder {
    kind: Kind,
    error_message: Option<String>,
    panic_string: Option<String>,
    panic_location: Option<String>,
    backtrace: Option<Backtrace>,
    error_data: Vec<ErrorData>,
    route_data: Vec<RouteData>,
    request_data: Option<RequestData>,
    project_config: String,
}

impl ErrorPageTemplateBuilder {
    #[must_use]
    fn not_found(error: &Error) -> Self {
        let mut error_data = Vec::new();
        let mut error_message = None;

        if let Some(not_found) = error.inner().downcast_ref::<NotFound>() {
            use crate::error::NotFoundKind as Kind;
            match &not_found.kind {
                Kind::Custom => {
                    Self::build_error_data(&mut error_data, error);
                }
                Kind::WithMessage(message) => {
                    Self::build_error_data(&mut error_data, error);
                    error_message = Some(message.clone());
                }
                // We don't need to build error data for Kind::FromRouter
                _ => {}
            }
        }

        Self {
            kind: Kind::NotFound,
            error_message,
            error_data,
            ..Default::default()
        }
    }

    #[must_use]
    fn error(error: &Error) -> Self {
        let mut error_data = Vec::new();
        Self::build_error_data(&mut error_data, error);

        Self {
            kind: Kind::Error,
            error_data,
            backtrace: Some(error.backtrace().clone()),
            ..Default::default()
        }
    }

    #[must_use]
    fn panic(
        panic_payload: &Box<dyn Any + Send>,
        panic_location: Option<String>,
        backtrace: Option<Backtrace>,
    ) -> Self {
        Self {
            kind: Kind::Panic,
            panic_string: Self::get_panic_string(panic_payload),
            panic_location,
            backtrace,
            ..Default::default()
        }
    }

    fn diagnostics(&mut self, diagnostics: &Diagnostics) -> &mut Self {
        self.project_config = format!("{:#?}", diagnostics.project_config);
        self.route_data.clear();
        Self::build_route_data(&mut self.route_data, &diagnostics.router, "", "");
        self.request_data = diagnostics
            .request_head
            .as_ref()
            .map(Self::build_request_data);
        self
    }

    fn build_route_data(
        route_data: &mut Vec<RouteData>,
        router: &Router,
        url_prefix: &str,
        index_prefix: &str,
    ) {
        for (index, route) in router.routes().iter().enumerate() {
            route_data.push(RouteData {
                index: format!("{index_prefix}{index}"),
                path: format!("{url_prefix}{}", route.url()),
                kind: match route.kind() {
                    crate::router::RouteKind::Router => if route_data.is_empty() {
                        "Root Router"
                    } else {
                        "Router"
                    }
                    .to_owned(),
                    crate::router::RouteKind::Handler => "View".to_owned(),
                },
                name: route.name().unwrap_or_default().to_owned(),
            });

            if let Some(inner_router) = route.router() {
                Self::build_route_data(
                    route_data,
                    inner_router,
                    &format!("{}{}", url_prefix, route.url()),
                    &format!("{index_prefix}{index}."),
                );
            }
        }
    }

    fn build_error_data(vec: &mut Vec<ErrorData>, error: &(dyn std::error::Error + 'static)) {
        let data = ErrorData {
            description: error.to_string(),
            debug_str: format!("{error:#?}"),
            is_cot_error: error.is::<Error>(),
        };
        vec.push(data);

        if let Some(source) = error.source() {
            Self::build_error_data(vec, source);
        }
    }

    #[must_use]
    fn build_request_data(head: &crate::request::RequestHead) -> RequestData {
        RequestData {
            method: head.method.to_string(),
            url: head.uri.to_string(),
            protocol_version: format!("{:?}", head.version),
            headers: head
                .headers
                .iter()
                .map(|(name, value)| {
                    (
                        name.as_str().to_owned(),
                        String::from_utf8_lossy(value.as_ref()).into_owned(),
                    )
                })
                .collect(),
        }
    }

    #[must_use]
    fn get_panic_string(panic_payload: &Box<dyn Any + Send>) -> Option<String> {
        if let Some(&panic_string) = panic_payload.downcast_ref::<&str>() {
            Some(panic_string.to_owned())
        } else {
            panic_payload.downcast_ref::<String>().cloned()
        }
    }

    fn render(&self) -> Result<String> {
        Ok(ErrorPageTemplate {
            kind: self.kind,
            error_message: self.error_message.clone(),
            panic_string: self.panic_string.clone(),
            panic_location: self.panic_location.clone(),
            backtrace: self.backtrace.clone(),
            error_data: self.error_data.clone(),
            route_data: self.route_data.clone(),
            request_data: self.request_data.clone(),
            project_config: self.project_config.clone(),
        }
        .render()?)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
enum Kind {
    NotFound,
    #[default]
    Error,
    Panic,
}

#[derive(Debug, Clone)]
struct ErrorData {
    description: String,
    debug_str: String,
    is_cot_error: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RouteData {
    index: String,
    path: String,
    kind: String,
    name: String,
}

#[derive(Debug, Clone)]
struct RequestData {
    method: String,
    url: String,
    protocol_version: String,
    headers: Vec<(String, String)>,
}

#[must_use]
pub(super) fn handle_not_found(
    error: &Error,
    diagnostics: &Diagnostics,
) -> axum::response::Response {
    log_not_found(
        error,
        diagnostics
            .request_head
            .as_ref()
            .map(ErrorPageTemplateBuilder::build_request_data)
            .as_ref(),
    );
    build_response(
        build_not_found_response(error, diagnostics),
        StatusCode::NOT_FOUND,
    )
}

#[must_use]
pub(super) fn handle_response_panic(
    panic_payload: &Box<dyn Any + Send>,
    diagnostics: &Diagnostics,
) -> axum::response::Response {
    let request_data = diagnostics
        .request_head
        .as_ref()
        .map(ErrorPageTemplateBuilder::build_request_data);

    let panic_location = PANIC_LOCATION.take();
    let backtrace = PANIC_BACKTRACE.take();
    log_panic(
        panic_payload,
        panic_location.as_deref(),
        request_data.as_ref(),
    );
    build_response(
        build_panic_response(panic_payload, diagnostics, panic_location, backtrace),
        StatusCode::INTERNAL_SERVER_ERROR,
    )
}

#[must_use]
pub(super) fn handle_response_error(
    error: &Error,
    diagnostics: &Diagnostics,
) -> axum::response::Response {
    if error.status_code() == StatusCode::NOT_FOUND {
        return handle_not_found(error, diagnostics);
    }

    log_error(
        error,
        diagnostics
            .request_head
            .as_ref()
            .map(ErrorPageTemplateBuilder::build_request_data)
            .as_ref(),
    );
    build_response(
        build_error_response(error, diagnostics),
        StatusCode::INTERNAL_SERVER_ERROR,
    )
}

#[must_use]
fn build_response(
    response_string: Result<String>,
    status_code: StatusCode,
) -> axum::response::Response {
    match response_string {
        Ok(error_str) => axum::response::Response::builder()
            .status(status_code)
            .header(
                http::header::CONTENT_TYPE,
                cot_core::headers::HTML_CONTENT_TYPE,
            )
            .body(axum::body::Body::new(error_str))
            .unwrap_or_else(|_| build_cot_failure_page()),
        Err(error) => {
            error!("Failed to render error page: {}", error);
            build_cot_failure_page()
        }
    }
}

fn build_not_found_response(error: &Error, diagnostics: &Diagnostics) -> Result<String> {
    ErrorPageTemplateBuilder::not_found(error)
        .diagnostics(diagnostics)
        .render()
}

fn build_panic_response(
    panic_payload: &Box<dyn Any + Send>,
    diagnostics: &Diagnostics,
    panic_location: Option<String>,
    backtrace: Option<Backtrace>,
) -> Result<String> {
    ErrorPageTemplateBuilder::panic(panic_payload, panic_location, backtrace)
        .diagnostics(diagnostics)
        .render()
}

fn build_error_response(error: &Error, diagnostics: &Diagnostics) -> Result<String> {
    ErrorPageTemplateBuilder::error(error)
        .diagnostics(diagnostics)
        .render()
}

const DEFAULT_SERVER_ERROR_PAGE: &[u8] = include_bytes!("../templates/500.html");
const FAILURE_PAGE: &[u8] = include_bytes!("../templates/fail.html");

/// A last-resort Internal Server Error page.
///
/// Returned when a custom error page fails to render.
pub(super) fn build_cot_server_error_page() -> axum::response::Response {
    axum::response::Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(axum::body::Body::from(DEFAULT_SERVER_ERROR_PAGE))
        .expect("Building the Cot server error page should never fail")
}

/// A last-resort error page.
///
/// This page is displayed when an error occurs that prevents Cot from rendering
/// a proper error page. This page is very simple and should only be displayed
/// in the event of a catastrophic failure, likely caused by a bug in Cot
/// itself.
#[must_use]
fn build_cot_failure_page() -> axum::response::Response {
    axum::response::Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(axum::body::Body::from(FAILURE_PAGE))
        .expect("Building the Cot failure page should never fail")
}

thread_local! {
    static PANIC_LOCATION: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
    static PANIC_BACKTRACE: std::cell::RefCell<Option<Backtrace>> = const { std::cell::RefCell::new(None) };
}

pub(super) fn error_page_panic_hook(info: &PanicHookInfo<'_>) {
    let location = info.location().map(|location| format!("{location}"));
    PANIC_LOCATION.replace(location);

    let backtrace = __cot_create_backtrace();
    PANIC_BACKTRACE.replace(Some(backtrace));
}

fn log_error(error: &Error, request_data: Option<&RequestData>) {
    let span = tracing::span!(Level::ERROR,
        "request_error",
        error_message = %error
    );
    let _enter = span.enter();
    if let Some(req) = request_data {
        error!(method = %req.method, path = %req.url, "Request failed with error!");
    } else {
        error!("Error occurred without request context!");
    }
}

fn log_panic(
    panic_payload: &Box<dyn Any + Send>,
    panic_location: Option<&str>,
    request_data: Option<&RequestData>,
) {
    let span = tracing::span!(
        Level::ERROR,
        "request_panic",
        panic_message = ?ErrorPageTemplateBuilder::get_panic_string(panic_payload),
        location = ?panic_location
    );
    let _enter = span.enter();
    if let Some(req) = request_data {
        error!(method = %req.method, path = %req.url, "Request handler panicked!");
    } else {
        error!("Panic occurred without a request context");
    }
}

fn log_not_found(error: &Error, request_data: Option<&RequestData>) {
    let span = tracing::span!(
        Level::WARN,
        "not_found",
        message = %error
    );
    let _enter = span.enter();

    if let Some(req) = request_data {
        warn!(method = %req.method, path = %req.url, "Route not found!");
    } else {
        warn!("Not Found error occurred without a request context!");
    }
}

#[cfg(test)]
mod tests {
    use std::panic;
    use std::sync::Arc;

    use tracing_test::traced_test;

    use super::*;
    use crate::router::{Route, Router};
    use crate::test::TestRequestBuilder;

    fn create_test_request_data() -> RequestData {
        RequestData {
            method: "GET".to_string(),
            url: "/test".to_string(),
            protocol_version: "HTTP/1.1".to_string(),
            headers: vec![("content-type".to_string(), "text/plain".to_string())],
        }
    }

    #[test]
    #[traced_test]
    fn test_log_error() {
        let error = Error::internal("Test Error!");
        let request_data = Some(create_test_request_data());

        log_error(&error, request_data.as_ref());
        assert!(logs_contain("Request failed with error!"));
        assert!(logs_contain("Test Error!"));
        assert!(logs_contain("GET"));
        assert!(logs_contain("/test"));
    }
    #[test]
    #[traced_test]
    fn test_log_panic() {
        let panic_payload: Box<dyn Any + Send> = Box::new("Test panic");
        let panic_location = Some("src/test.rs:10");
        let request_data = Some(create_test_request_data());

        log_panic(&panic_payload, panic_location, request_data.as_ref());

        assert!(logs_contain("Request handler panicked"));
        assert!(logs_contain("Test panic"));
        assert!(logs_contain("src/test.rs:10"));
    }

    #[test]
    #[traced_test]
    fn test_log_not_found() {
        let error = Error::from(NotFound::with_message("Resource not found"));
        let request_data = Some(create_test_request_data());

        log_not_found(&error, request_data.as_ref());

        assert!(logs_contain("Route not found"));
        assert!(logs_contain("Resource not found"));
        assert!(logs_contain("GET"));
        assert!(logs_contain("/test"));
    }

    #[test]
    #[traced_test]
    fn test_log_error_without_request() {
        let error = Error::internal("Test error");
        log_error(&error, None);

        assert!(logs_contain("Error occurred without request context"));
        assert!(logs_contain("Test error"));
    }

    #[test]
    #[traced_test]
    fn test_handle_response_error_logging() {
        let diagnostics = create_diagnostics();
        let error = Error::internal("Test handler error");

        let response = handle_response_error(&error, &diagnostics);

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(logs_contain("Request failed with error"));
        assert!(logs_contain("Test handler error"));
    }

    #[test]
    #[traced_test]
    fn test_handle_response_panic_logging() {
        let diagnostics = create_diagnostics();
        let panic_payload: Box<dyn Any + Send> = Box::new("Test panic in handler");

        let response = handle_response_panic(&panic_payload, &diagnostics);

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(logs_contain("Request handler panicked"));
        assert!(logs_contain("Test panic in handler"));
    }

    fn create_diagnostics() -> Diagnostics {
        let project_config = ProjectConfig::default();
        let router = Arc::new(Router::with_urls(vec![]));
        let request = TestRequestBuilder::get("/").build();
        let (head, _body) = request.into_parts();

        Diagnostics::new(project_config, router, Some(head))
    }

    #[test]
    fn test_handle_not_found() {
        let diagnostics = create_diagnostics();

        let error = Error::from(NotFound::new());
        let response = handle_not_found(&error, &diagnostics);

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_handle_response_panic() {
        let diagnostics = create_diagnostics();
        let panic_payload: Box<dyn Any + Send> = Box::new("panic occurred");

        let response = handle_response_panic(&panic_payload, &diagnostics);

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_handle_response_error() {
        let diagnostics = create_diagnostics();
        let error = Error::internal("error occurred");

        let response = handle_response_error(&error, &diagnostics);

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn build_route_data() {
        let mut route_data = Vec::new();
        let sub_sub_router = Router::with_urls(vec![]);
        let sub_router = Router::with_urls(vec![Route::with_router("/bar", sub_sub_router)]);
        let router = Router::with_urls(vec![Route::with_router("/foo", sub_router)]);

        ErrorPageTemplateBuilder::build_route_data(&mut route_data, &router, "", "");

        assert_eq!(
            route_data,
            vec![
                RouteData {
                    index: "0".to_string(),
                    path: "/foo".to_string(),
                    kind: "Root Router".to_string(),
                    name: String::new()
                },
                RouteData {
                    index: "0.0".to_string(),
                    path: "/foo/bar".to_string(),
                    kind: "Router".to_string(),
                    name: String::new()
                }
            ]
        );
    }

    #[test]
    fn test_build_cot_failure_page() {
        let response = build_cot_failure_page();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}
