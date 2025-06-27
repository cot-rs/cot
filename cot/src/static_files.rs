//! Static files middleware.
//!
//! This middleware serves static files from the `static` directory of the
//! project.

use std::collections::HashMap;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use bytes::Bytes;
use cot::config::StaticFilesPathRewriteMode;
use digest::Digest;
use futures_core::ready;
use http::{Request, header};
use pin_project_lite::pin_project;
use tower::Service;

use crate::Body;
use crate::config::StaticFilesConfig;
use crate::project::MiddlewareContext;
use crate::response::{Response, ResponseExt};

/// Macro to define static files by specifying their paths.
///
/// The files are included at compile time using the `include_bytes!` macro.
/// The paths are relative to the `static` directory of the project (under the
/// project root, where the `Cargo.toml` file is).
///
/// This is mainly useful with the
/// [`CotApp::static_files`](crate::App::static_files) trait method.
///
/// # Example
///
/// ```
/// use bytes::Bytes;
/// use cot::static_files::StaticFile;
/// use cot::{App, static_files};
///
/// pub struct ExampleApp;
///
/// // Project structure:
/// // .
/// // ├── Cargo.toml
/// // └── static
/// //     └── test
/// //         └── test.txt
///
/// impl App for ExampleApp {
///     fn name(&self) -> &str {
///         "test_app"
///     }
///
///     fn static_files(&self) -> Vec<StaticFile> {
///         static_files!("test/test.txt")
///     }
/// }
/// ```
#[macro_export]
macro_rules! static_files {
    ($($path:literal),* $(,)?) => {
        ::std::vec![$(
            $crate::static_files::StaticFile::new(
                $path.to_string(),
                $crate::__private::Bytes::from_static(
                    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/static/", $path))
                ),
            )
        ),*]
    };
}

/// Struct representing a collection of static files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StaticFiles {
    url_prefix: String,
    files: HashMap<String, StaticFileWithMeta>,
    rewrite_mode: StaticFilesPathRewriteMode,
    cache_timeout: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StaticFileWithMeta {
    url: String,
    file: StaticFile,
}

impl StaticFiles {
    /// Creates a new `StaticFiles` instance.
    #[must_use]
    pub(crate) fn new(config: &StaticFilesConfig) -> Self {
        Self {
            url_prefix: config.url.clone(),
            files: HashMap::new(),
            rewrite_mode: config.rewrite.clone(),
            cache_timeout: config.cache_timeout,
        }
    }

    pub(crate) fn add_file(&mut self, file: StaticFile) {
        let path = file.path.clone();
        let file = StaticFileWithMeta {
            url: self.file_url(&file),
            file,
        };
        self.files.insert(path, file);
    }

    fn file_url(&self, file: &StaticFile) -> String {
        match self.rewrite_mode {
            StaticFilesPathRewriteMode::None => {
                format!("{}{}", self.url_prefix, file.path.clone())
            }
            StaticFilesPathRewriteMode::QueryParam => {
                format!(
                    "{}{}?v={}",
                    self.url_prefix,
                    file.path.clone(),
                    Self::file_hash(file)
                )
            }
        }
    }

    #[must_use]
    fn file_hash(file: &StaticFile) -> String {
        hex::encode(&sha2::Sha256::digest(&file.content).as_slice()[0..6])
    }

    #[must_use]
    fn get_file(&self, path: &str) -> Option<&StaticFile> {
        self.files
            .get(path)
            .map(|file_with_meta| &file_with_meta.file)
    }

    #[must_use]
    pub(crate) fn path_for(&self, path: &str) -> Option<&str> {
        self.files
            .get(path)
            .map(|file_with_meta| file_with_meta.url.as_str())
    }

    pub(crate) fn collect_into(&self, path: &Path) -> Result<(), std::io::Error> {
        for (file_path, file_with_meta) in &self.files {
            let file_path = path.join(file_path);
            std::fs::create_dir_all(
                file_path
                    .parent()
                    .expect("a joined file path should always have a parent"),
            )?;
            std::fs::write(file_path, &file_with_meta.file.content)?;
        }
        Ok(())
    }
}

impl From<&MiddlewareContext> for StaticFiles {
    fn from(context: &MiddlewareContext) -> Self {
        let mut static_files = StaticFiles::new(&context.config().static_files);

        for module in context.apps() {
            for file in module.static_files() {
                static_files.add_file(file);
            }
        }

        static_files
    }
}

/// A static file that can be served by the application.
///
/// This struct represents a static file that can be served by the application.
/// It contains the file's path, content, and MIME type. The MIME type is
/// automatically detected based on the file extension.
///
/// # Examples
///
/// ```
/// use bytes::Bytes;
/// use cot::static_files::StaticFile;
/// use cot::{App, static_files};
///
/// pub struct ExampleApp;
///
/// // Project structure:
/// // .
/// // ├── Cargo.toml
/// // └── static
/// //     └── test
/// //         └── test.txt
///
/// impl App for ExampleApp {
///     fn name(&self) -> &str {
///         "test_app"
///     }
///
///     fn static_files(&self) -> Vec<StaticFile> {
///         static_files!("test/test.txt")
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StaticFile {
    /// The path of the file, relative to the static files directory.
    path: String,
    /// The content of the file.
    content: Bytes,
    /// The MIME type of the file.
    mime_type: mime_guess::Mime,
}

impl StaticFile {
    /// Creates a new `StaticFile` instance.
    ///
    /// The MIME type is automatically detected based on the file extension.
    /// If the file extension is not recognized, it defaults to
    /// `application/octet-stream`.
    ///
    /// Instead of using this constructor, it's typically more convenient
    /// to use the [`static_files!`](macro@static_files) macro.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::static_files::StaticFile;
    ///
    /// let file = StaticFile::new("style.css", "body { color: red; }");
    /// ```
    #[must_use]
    pub fn new<Path, Content>(path: Path, content: Content) -> Self
    where
        Path: Into<String>,
        Content: Into<Bytes>,
    {
        let path = path.into();
        let content = content.into();
        let mime_type = mime_guess::from_path(&path).first_or_octet_stream();
        Self {
            path,
            content,
            mime_type,
        }
    }

    #[must_use]
    fn as_response(&self) -> Response {
        Response::builder()
            .header(header::CONTENT_TYPE, self.mime_type.to_string())
            .body(Body::fixed(self.content.clone()))
            .expect("failed to build static file response")
    }
}

/// Middleware for serving static files.
///
/// This middleware serves static files defined by the applications by using
/// the [`CotApp::static_files`](crate::App::static_files) trait
/// method. The middleware serves files from the `/static/` path.
///
/// If a request is made to a path starting with `/static/`, the middleware
/// checks if the file exists in the static files collection. If it does, the
/// file is served. Otherwise, the request is passed to the inner service.
#[derive(Debug, Clone)]
pub struct StaticFilesMiddleware {
    static_files: Arc<StaticFiles>,
}

impl StaticFilesMiddleware {
    /// Creates a new `StaticFilesMiddleware` instance from the project
    /// context.
    #[must_use]
    pub fn from_context(context: &MiddlewareContext) -> Self {
        Self {
            static_files: Arc::new(StaticFiles::from(context)),
        }
    }
}

impl<S> tower::Layer<S> for StaticFilesMiddleware {
    type Service = StaticFilesService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        StaticFilesService::new(Arc::clone(&self.static_files), inner)
    }
}

/// Service for handling requests and serving static files.
#[derive(Clone, Debug)]
pub struct StaticFilesService<S> {
    static_files: Arc<StaticFiles>,
    inner: S,
}

impl<S> StaticFilesService<S> {
    /// Create a new static files service.
    #[must_use]
    fn new(static_files: Arc<StaticFiles>, inner: S) -> Self {
        Self {
            static_files,
            inner,
        }
    }
}

impl<ReqBody, S> Service<Request<ReqBody>> for StaticFilesService<S>
where
    S: Service<Request<ReqBody>, Response = Response>,
{
    type Error = S::Error;
    type Future = ResponseFuture<S::Future>;
    type Response = S::Response;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<ReqBody>) -> Self::Future {
        let path = req.uri().path();
        let file_contents =
            if let Some(stripped_path) = path.strip_prefix(&self.static_files.url_prefix) {
                self.static_files
                    .get_file(stripped_path)
                    .map(StaticFile::as_response)
            } else {
                None
            };

        if let Some(mut response) = file_contents {
            if let Some(timeout) = self.static_files.cache_timeout {
                response.headers_mut().insert(
                    header::CACHE_CONTROL,
                    header::HeaderValue::from_str(&format!("max-age={}", timeout.as_secs()))
                        .expect("failed to create cache control header"),
                );
            }
            ResponseFuture::StaticFileResponse { response }
        } else {
            req.extensions_mut().insert(Arc::clone(&self.static_files));
            ResponseFuture::Inner {
                future: self.inner.call(req),
            }
        }
    }
}

pin_project! {
    /// Future representing the response for a static file request.
    #[project = ResponseFutureProj]
    #[expect(missing_docs)]  // because of: https://github.com/taiki-e/pin-project-lite/issues/3
    pub enum ResponseFuture<F> {
        /// Response for a static file.
        StaticFileResponse {
            // A [`Response`] object for a static file.
            response: Response,
        },
        /// Response from the inner service.
        Inner {
            // The inner service's future.
            #[pin]
            future: F,
        },
    }
}

impl<F, E> Future for ResponseFuture<F>
where
    F: Future<Output = Result<Response, E>>,
{
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        match this {
            ResponseFutureProj::StaticFileResponse { response } => {
                Poll::Ready(Ok(std::mem::take(response)))
            }
            ResponseFutureProj::Inner { future } => {
                let res = ready!(future.poll(cx)?);
                Poll::Ready(Ok(res))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use http::{Request, StatusCode};
    use tower::{Layer, ServiceExt};

    use super::*;
    use crate::config::{ProjectConfig, StaticFilesConfig, StaticFilesPathRewriteMode};
    use crate::project::RegisterAppsContext;
    use crate::{App, AppBuilder, Bootstrapper, Project};

    #[test]
    #[cfg_attr(
        miri,
        ignore = "unsupported operation: can't call foreign function `sqlite3_open_v2`"
    )]
    fn static_files_add_and_get_file() {
        let mut static_files = StaticFiles::new(&StaticFilesConfig::default());
        static_files.add_file(StaticFile::new("test.txt", "This is a test file"));

        let file = static_files.get_file("test.txt");

        assert!(file.is_some());
        assert_eq!(file.unwrap().content, Bytes::from("This is a test file"));
    }

    #[cot::test]
    async fn file_as_response() {
        let file = StaticFile {
            path: "test.txt".to_owned(),
            content: Bytes::from("This is a test file"),
            mime_type: mime_guess::mime::TEXT_PLAIN,
        };

        let response = file.as_response();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "text/plain");
        assert_eq!(
            response.into_body().into_bytes().await.unwrap(),
            Bytes::from("This is a test file")
        );
    }

    fn create_static_files() -> StaticFiles {
        let mut static_files = StaticFiles::new(&StaticFilesConfig::default());
        static_files.add_file(StaticFile::new("test.txt", "This is a test file"));
        static_files
    }

    #[cot::test]
    async fn static_files_middleware() {
        let static_files = Arc::new(create_static_files());
        let middleware = StaticFilesMiddleware {
            static_files: Arc::clone(&static_files),
        };

        let service = middleware.layer(tower::service_fn(|_req| async {
            Ok::<_, std::convert::Infallible>(Response::new(Body::empty()))
        }));

        let request = Request::builder()
            .uri("/static/test.txt")
            .body(Body::empty())
            .unwrap();

        let response = service.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "text/plain");
        assert_eq!(
            response.into_body().into_bytes().await.unwrap(),
            Bytes::from("This is a test file")
        );
    }

    #[cot::test]
    async fn static_files_middleware_with_config() {
        let mut static_files = StaticFiles::new(
            &StaticFilesConfig::builder()
                .url("/assets/")
                .rewrite(StaticFilesPathRewriteMode::QueryParam)
                .cache_timeout(Duration::from_secs(300))
                .build(),
        );
        static_files.add_file(StaticFile::new("test.txt", "This is a test file"));
        let static_files = Arc::new(static_files);

        let middleware = StaticFilesMiddleware {
            static_files: Arc::clone(&static_files),
        };

        let service = middleware.layer(tower::service_fn(|_req| async {
            Ok::<_, std::convert::Infallible>(Response::new(Body::empty()))
        }));

        let url = static_files.path_for("test.txt").unwrap();
        assert!(url.starts_with("/assets/test.txt?v="));

        let request = Request::builder().uri(url).body(Body::empty()).unwrap();
        let response = service.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()["content-type"], "text/plain");
        assert_eq!(
            response.into_body().into_bytes().await.unwrap(),
            Bytes::from("This is a test file")
        );
    }

    #[cot::test]
    async fn static_files_middleware_not_found() {
        let static_files = Arc::new(create_static_files());
        let middleware = StaticFilesMiddleware {
            static_files: Arc::clone(&static_files),
        };
        let service = middleware.layer(tower::service_fn(|_req| async {
            Ok::<_, std::convert::Infallible>(Response::new(Body::fixed("test")))
        }));

        let request = Request::builder()
            .uri("/static/nonexistent.txt")
            .body(Body::empty())
            .unwrap();

        let response = service.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.into_body().into_bytes().await.unwrap(),
            Bytes::from("test") // Inner service response
        );
    }

    #[cot::test]
    #[cfg_attr(
        miri,
        ignore = "unsupported operation: can't call foreign function `sqlite3_open_v2`"
    )]
    async fn static_files_middleware_from_context() {
        struct App1;
        impl App for App1 {
            fn name(&self) -> &'static str {
                "app1"
            }

            fn static_files(&self) -> Vec<StaticFile> {
                static_files!("test/test.txt")
            }
        }

        struct App2;
        impl App for App2 {
            fn name(&self) -> &'static str {
                "app2"
            }

            fn static_files(&self) -> Vec<StaticFile> {
                vec![StaticFile::new("app2/test.js", "test")]
            }
        }

        struct TestProject;
        impl Project for TestProject {
            fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
                apps.register(App1);
                apps.register(App2);
            }
        }

        let bootstrapper = Bootstrapper::new(TestProject)
            .with_config(ProjectConfig::default())
            .with_apps()
            .with_database()
            .await
            .unwrap();
        let middleware = StaticFilesMiddleware::from_context(bootstrapper.context());
        let static_files = middleware.static_files;

        let file = static_files.get_file("test/test.txt").unwrap();
        assert_eq!(file.mime_type, mime_guess::mime::TEXT_PLAIN);
        assert_eq!(
            file.content,
            Bytes::from_static(include_bytes!("../static/test/test.txt"))
        );

        let file = static_files.get_file("app2/test.js").unwrap();
        assert_eq!(file.content, Bytes::from("test"));
    }

    #[test]
    fn collect_into() {
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_path = temp_dir.path().to_path_buf();

        let mut static_files = StaticFiles::new(&StaticFilesConfig::default());
        static_files.add_file(StaticFile::new("test.txt", "This is a test file"));
        static_files.add_file(StaticFile::new(
            "nested/test2.txt",
            "This is another test file",
        ));

        static_files.collect_into(&temp_path).unwrap();

        let file_path = temp_path.join("test.txt");
        let nested_file_path = temp_path.join("nested/test2.txt");

        assert!(file_path.exists());
        assert_eq!(
            fs::read_to_string(file_path).unwrap(),
            "This is a test file"
        );

        assert!(nested_file_path.exists());
        assert_eq!(
            fs::read_to_string(nested_file_path).unwrap(),
            "This is another test file"
        );
    }

    #[test]
    fn collect_into_empty() {
        let temp_dir = tempfile::tempdir().unwrap();
        let temp_path = temp_dir.path().to_path_buf();

        let static_files = StaticFiles::new(&StaticFilesConfig::default());
        static_files.collect_into(&temp_path).unwrap();

        assert!(fs::read_dir(&temp_path).unwrap().next().is_none());
    }

    #[test]
    fn static_files_macro() {
        let static_files = static_files!("test/test.txt");

        assert_eq!(static_files.len(), 1);
        assert_eq!(static_files[0].path, "test/test.txt");
        assert_eq!(
            static_files[0].content,
            Bytes::from_static(include_bytes!("../static/test/test.txt"))
        );
    }

    #[test]
    fn static_files_macro_trailing_comma() {
        let static_files = static_files!("test/test.txt",);

        assert_eq!(static_files.len(), 1);
    }

    #[test]
    fn static_file_mime_type_detection() {
        let file = StaticFile::new("style.css", "body { color: red; }");
        assert_eq!(file.mime_type, mime_guess::mime::TEXT_CSS);

        let file = StaticFile::new("script.js", "console.log('test');");
        assert_eq!(file.mime_type, mime_guess::mime::TEXT_JAVASCRIPT);

        let file = StaticFile::new("image.png", "fake image data");
        assert_eq!(file.mime_type, mime_guess::mime::IMAGE_PNG);

        let file = StaticFile::new("unknown", "some content");
        assert_eq!(file.mime_type, mime_guess::mime::APPLICATION_OCTET_STREAM);
    }

    #[test]
    fn static_files_url_rewriting() {
        let mut static_files = StaticFiles::new(&StaticFilesConfig {
            url: "/static/".to_string(),
            rewrite: StaticFilesPathRewriteMode::None,
            cache_timeout: None,
        });

        let file = StaticFile::new("test.txt", "test content");
        static_files.add_file(file);

        // Test None rewrite mode
        let url = static_files.path_for("test.txt").unwrap();
        assert_eq!(url, "/static/test.txt");

        // Test QueryParam rewrite mode
        let mut static_files = StaticFiles::new(&StaticFilesConfig {
            url: "/static/".to_string(),
            rewrite: StaticFilesPathRewriteMode::QueryParam,
            cache_timeout: None,
        });

        let file = StaticFile::new("test.txt", "test content");
        static_files.add_file(file);

        let url = static_files.path_for("test.txt").unwrap();
        assert!(url.starts_with("/static/test.txt?v="));
        assert_eq!(url.len(), "/static/test.txt?v=".len() + 12); // 6 bytes of hash in hex = 12 chars
    }

    #[test]
    fn static_files_url_rewriting_with_different_prefix() {
        let mut static_files = StaticFiles::new(&StaticFilesConfig {
            url: "/assets/".to_string(),
            rewrite: StaticFilesPathRewriteMode::QueryParam,
            cache_timeout: None,
        });

        let file = StaticFile::new("images/logo.png", "fake image data");
        static_files.add_file(file);

        let url = static_files.path_for("images/logo.png").unwrap();
        assert!(url.starts_with("/assets/images/logo.png?v="));
    }

    #[test]
    fn static_files_hash_consistency() {
        let mut static_files = StaticFiles::new(&StaticFilesConfig {
            url: "/static/".to_string(),
            rewrite: StaticFilesPathRewriteMode::QueryParam,
            cache_timeout: None,
        });

        let file = StaticFile::new("test.txt", "test content");
        static_files.add_file(file);

        // Get the URL twice and verify the hash is consistent
        let url1 = static_files.path_for("test.txt").unwrap().to_owned();
        let url2 = static_files.path_for("test.txt").unwrap().to_owned();
        assert_eq!(url1, url2);

        // Add the same file again and verify the hash is still consistent
        let file = StaticFile::new("test.txt", "test content");
        static_files.add_file(file);
        let url3 = static_files.path_for("test.txt").unwrap();
        assert_eq!(url1, url3);
    }

    #[test]
    fn static_files_hash_changes_with_content() {
        let mut static_files = StaticFiles::new(&StaticFilesConfig {
            url: "/static/".to_string(),
            rewrite: StaticFilesPathRewriteMode::QueryParam,
            cache_timeout: None,
        });

        let file1 = StaticFile::new("test.txt", "content 1");
        static_files.add_file(file1);
        let url1 = static_files.path_for("test.txt").unwrap().to_owned();

        let file2 = StaticFile::new("test.txt", "content 2");
        static_files.add_file(file2);
        let url2 = static_files.path_for("test.txt").unwrap();

        assert_ne!(url1, url2);
    }
}
