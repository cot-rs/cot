//! The OpenAPI (Swagger) UI router.
//!
//! This module provides a [`cot::App`] which serves the OpenAPI (Swagger) UI.

use std::borrow::Cow;
use std::sync::{Arc, OnceLock};

use bytes::Bytes;
use swagger_ui_redist::SwaggerUiStaticFile;

use crate::App;
use crate::html::Html;
use crate::json::Json;
use crate::request::extractors::StaticFiles;
use crate::request::{Request, RequestExt};
use crate::router::{Route, Router};
use crate::static_files::StaticFile;

/// A wrapper around the Swagger UI functionality.
///
/// This struct serves the Swagger UI interface for OpenAPI documentation.
/// It can be registered with a Cot application to provide interactive API
/// documentation at a specified URL path.
///
/// # Example
///
/// ```
/// use cot::openapi::swagger_ui::SwaggerUi;
/// use cot::project::{AppBuilder, RegisterAppsContext};
///
/// fn register_apps(apps: &mut AppBuilder, _context: &RegisterAppsContext) {
///     // Register SwaggerUI at the "/swagger" path
///     apps.register_with_views(SwaggerUi::new(), "/swagger");
/// }
/// ```
#[derive(Debug, Clone)]
pub struct SwaggerUi {
    inner: Arc<OnceLock<swagger_ui_redist::SwaggerUi>>,
    openapi_path: Arc<Cow<'static, str>>,
    serve_openapi: bool,
}

async fn openapi_json(request: Request) -> Json<aide::openapi::OpenApi> {
    Json(request.router().as_api())
}

impl Default for SwaggerUi {
    fn default() -> Self {
        Self::new()
    }
}

impl SwaggerUi {
    /// Creates a new [`SwaggerUi`] that serves the OpenAPI JSON at the default
    /// path.
    #[must_use]
    pub fn new() -> Self {
        Self::new_impl(Cow::Borrowed("openapi.json"), true)
    }

    /// Creates a new [`SwaggerUi`] that serves the OpenAPI JSON at the
    /// specified path.
    #[must_use]
    pub fn with_api_at<P: Into<Cow<'static, str>>>(openapi_path: P) -> Self {
        Self::new_impl(openapi_path.into(), false)
    }

    fn new_impl(openapi_path: Cow<'static, str>, serve_openapi: bool) -> Self {
        Self {
            inner: Arc::new(OnceLock::new()),
            openapi_path: Arc::new(openapi_path),
            serve_openapi,
        }
    }

    fn build_swagger_ui(
        openapi_path: Cow<'static, str>,
        static_files: &StaticFiles,
    ) -> crate::Result<swagger_ui_redist::SwaggerUi> {
        let mut swagger_ui = swagger_ui_redist::SwaggerUi::new();
        swagger_ui.config().urls([openapi_path]);
        for static_file in SwaggerUiStaticFile::all() {
            let file_path = static_files.url_for(&Self::static_file_path(*static_file))?;
            swagger_ui.override_file_path(*static_file, file_path.to_owned());
        }

        Ok(swagger_ui)
    }

    fn static_file_path(static_file: SwaggerUiStaticFile) -> String {
        format!("swagger/{}", static_file.file_name())
    }
}

impl App for SwaggerUi {
    fn name(&self) -> &'static str {
        "swagger-ui"
    }

    fn router(&self) -> Router {
        let swagger_ui = Arc::clone(&self.inner);
        let openapi_path = Arc::clone(&self.openapi_path);

        let swagger_handler = async move |static_files: StaticFiles| {
            let swagger_ui = swagger_ui.get_or_init(move || {
                // TODO return an error when feature(once_cell_get_mut) is stable:
                // https://github.com/rust-lang/rust/issues/121641
                Self::build_swagger_ui((*openapi_path).clone(), &static_files)
                    .expect("could not build swagger UI")
            });
            let swagger = swagger_ui.serve().map_err(cot::Error::internal)?;
            Ok::<_, crate::Error>(Html::new(swagger))
        };

        let mut urls = vec![Route::with_handler("/", swagger_handler)];
        if self.serve_openapi {
            urls.push(Route::with_handler("/openapi.json", openapi_json));
        }
        Router::with_urls(urls)
    }

    fn static_files(&self) -> Vec<StaticFile> {
        swagger_ui_redist::SwaggerUi::static_files()
            .iter()
            .map(|(static_file_id, data)| {
                let path = Self::static_file_path(*static_file_id);
                let bytes = Bytes::from_static(data);
                StaticFile::new(path, bytes)
            })
            .collect()
    }
}
