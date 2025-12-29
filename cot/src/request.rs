use std::sync::Arc;

use cot::db::Database;
use cot::request::extractors::InvalidContentType;
use cot_core::request::extractors::FromRequestHead;
use cot_core::request::{PathParams, Request, RequestHead, RouteName};
use cot_core::router::Router;
use http::Extensions;

pub mod extractors;

mod private {
    pub trait Sealed {}
}

/// Extension trait for [`http::Request`] that provides helper methods for
/// working with HTTP requests.
///
/// # Sealed
///
/// This trait is sealed since it doesn't make sense to be implemented for types
/// outside the context of Cot.
pub trait RequestExt: private::Sealed {
    /// Runs an extractor implementing [`FromRequestHead`] on the request.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::response::Response;
    /// use cot_core::request::extractors::Path;
    /// use cot_core::request::{Request, RequestExt};
    ///
    /// async fn my_handler(mut request: Request) -> cot_core::Result<Response> {
    ///     let path_params = request.extract_from_head::<Path<String>>().await?;
    ///     // ...
    ///     # unimplemented!()
    /// }
    /// ```
    fn extract_from_head<E>(&mut self) -> impl Future<Output = cot_core::Result<E>> + Send
    where
        E: FromRequestHead + 'static;

    /// Get the application context.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::response::Response;
    /// use cot_core::request::{Request, RequestExt};
    ///
    /// async fn my_handler(mut request: Request) -> cot_core::Result<Response> {
    ///     let context = request.context();
    ///     // ... do something with the context
    ///     # unimplemented!()
    /// }
    /// ```
    #[must_use]
    fn context(&self) -> &cot::project::ProjectContext;

    /// Get the project configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::response::Response;
    /// use cot_core::request::{Request, RequestExt};
    ///
    /// async fn my_handler(mut request: Request) -> cot_core::Result<Response> {
    ///     let config = request.project_config();
    ///     // ... do something with the config
    ///     # unimplemented!()
    /// }
    /// ```
    #[must_use]
    fn project_config(&self) -> &cot::config::ProjectConfig;

    /// Get the router.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::response::Response;
    /// use cot_core::request::{Request, RequestExt};
    ///
    /// async fn my_handler(mut request: Request) -> cot_core::Result<Response> {
    ///     let router = request.router();
    ///     // ... do something with the router
    ///     # unimplemented!()
    /// }
    /// ```
    #[must_use]
    fn router(&self) -> &Arc<Router>;

    /// Get the app name the current route belongs to, or [`None`] if the
    /// request is not routed.
    ///
    /// This is mainly useful for providing context to reverse redirects, where
    /// you want to redirect to a route in the same app.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::response::Response;
    /// use cot_core::request::{Request, RequestExt};
    ///
    /// async fn my_handler(mut request: Request) -> cot_core::Result<Response> {
    ///     let app_name = request.app_name();
    ///     // ... do something with the app name
    ///     # unimplemented!()
    /// }
    /// ```
    fn app_name(&self) -> Option<&str>;

    /// Get the route name, or [`None`] if the request is not routed or doesn't
    /// have a route name.
    ///
    /// This is mainly useful for use in templates, where you want to know which
    /// route is being rendered, for instance to mark the active tab.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::response::Response;
    /// use cot_core::request::{Request, RequestExt};
    ///
    /// async fn my_handler(mut request: Request) -> cot_core::Result<Response> {
    ///     let route_name = request.route_name();
    ///     // ... do something with the route name
    ///     # unimplemented!()
    /// }
    /// ```
    #[must_use]
    fn route_name(&self) -> Option<&str>;

    /// Get the path parameters.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::response::Response;
    /// use cot_core::request::{Request, RequestExt};
    ///
    /// async fn my_handler(mut request: Request) -> cot_core::Result<Response> {
    ///     let path_params = request.path_params();
    ///     // ... do something with the path params
    ///     # unimplemented!()
    /// }
    /// ```
    #[must_use]
    fn path_params(&self) -> &PathParams;

    /// Get the path parameters mutably.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::response::Response;
    /// use cot_core::request::{Request, RequestExt};
    ///
    /// async fn my_handler(mut request: Request) -> cot_core::Result<Response> {
    ///     let path_params = request.path_params_mut();
    ///     // ... do something with the path params
    ///     # unimplemented!()
    /// }
    /// ```
    #[must_use]
    fn path_params_mut(&mut self) -> &mut PathParams;

    /// Get the database.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::response::Response;
    /// use cot_core::request::{Request, RequestExt};
    ///
    /// async fn my_handler(mut request: Request) -> cot_core::Result<Response> {
    ///     let db = request.db();
    ///     // ... do something with the database
    ///     # unimplemented!()
    /// }
    /// ```
    #[cfg(feature = "db")]
    #[must_use]
    fn db(&self) -> &Arc<Database>;

    /// Get the content type of the request.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::response::Response;
    /// use cot_core::request::{Request, RequestExt};
    ///
    /// async fn my_handler(mut request: Request) -> cot_core::Result<Response> {
    ///     let content_type = request.content_type();
    ///     // ... do something with the content type
    ///     # unimplemented!()
    /// }
    /// ```
    #[must_use]
    fn content_type(&self) -> Option<&http::HeaderValue>;

    /// Expect the content type of the request to be the given value.
    ///
    /// # Errors
    ///
    /// Throws an error if the content type is not the expected value.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::response::Response;
    /// use cot_core::request::{Request, RequestExt};
    ///
    /// async fn my_handler(mut request: Request) -> cot_core::Result<Response> {
    ///     request.expect_content_type("application/json")?;
    ///     // ...
    ///     # unimplemented!()
    /// }
    /// ```
    fn expect_content_type(&mut self, expected: &'static str) -> cot_core::Result<()> {
        let content_type = self
            .content_type()
            .map_or("".into(), |value| String::from_utf8_lossy(value.as_bytes()));
        if content_type == expected {
            Ok(())
        } else {
            Err(InvalidContentType {
                expected,
                actual: content_type.into_owned(),
            }
            .into())
        }
    }

    #[doc(hidden)]
    fn extensions(&self) -> &Extensions;
}

impl private::Sealed for Request {}

impl RequestExt for Request {
    async fn extract_from_head<E>(&mut self) -> cot_core::Result<E>
    where
        E: FromRequestHead + 'static,
    {
        let request = std::mem::take(self);

        let (head, body) = request.into_parts();
        let result = E::from_request_head(&head).await;

        *self = Request::from_parts(head, body);
        result
    }

    #[track_caller]
    fn context(&self) -> &cot::project::ProjectContext {
        self.extensions()
            .get::<Arc<cot::project::ProjectContext>>()
            .expect("AppContext extension missing")
    }

    fn project_config(&self) -> &cot::config::ProjectConfig {
        self.context().config()
    }

    fn router(&self) -> &Arc<Router> {
        self.context().router()
    }

    fn app_name(&self) -> Option<&str> {
        self.extensions()
            .get::<AppName>()
            .map(|AppName(name)| name.as_str())
    }

    fn route_name(&self) -> Option<&str> {
        self.extensions()
            .get::<RouteName>()
            .map(|RouteName(name)| name.as_str())
    }

    #[track_caller]
    fn path_params(&self) -> &PathParams {
        self.extensions()
            .get::<PathParams>()
            .expect("PathParams extension missing")
    }

    fn path_params_mut(&mut self) -> &mut PathParams {
        self.extensions_mut().get_or_insert_default::<PathParams>()
    }

    #[cfg(feature = "db")]
    fn db(&self) -> &Arc<Database> {
        self.context().database()
    }

    fn content_type(&self) -> Option<&http::HeaderValue> {
        self.headers().get(http::header::CONTENT_TYPE)
    }

    fn extensions(&self) -> &Extensions {
        self.extensions()
    }
}

impl private::Sealed for RequestHead {}

impl RequestExt for RequestHead {
    async fn extract_from_head<E>(&mut self) -> cot_core::Result<E>
    where
        E: FromRequestHead + 'static,
    {
        E::from_request_head(self).await
    }

    fn context(&self) -> &cot::project::ProjectContext {
        self.extensions
            .get::<Arc<cot::project::ProjectContext>>()
            .expect("AppContext extension missing")
    }

    fn project_config(&self) -> &cot::config::ProjectConfig {
        self.context().config()
    }

    fn router(&self) -> &Arc<Router> {
        self.context().router()
    }

    fn app_name(&self) -> Option<&str> {
        self.extensions
            .get::<AppName>()
            .map(|AppName(name)| name.as_str())
    }

    fn route_name(&self) -> Option<&str> {
        self.extensions
            .get::<RouteName>()
            .map(|RouteName(name)| name.as_str())
    }

    fn path_params(&self) -> &PathParams {
        self.extensions
            .get::<PathParams>()
            .expect("PathParams extension missing")
    }

    fn path_params_mut(&mut self) -> &mut PathParams {
        self.extensions.get_or_insert_default::<PathParams>()
    }

    #[cfg(feature = "db")]
    fn db(&self) -> &Arc<Database> {
        self.context().database()
    }

    fn content_type(&self) -> Option<&http::HeaderValue> {
        self.headers.get(http::header::CONTENT_TYPE)
    }

    fn extensions(&self) -> &Extensions {
        &self.extensions
    }
}

#[cfg(test)]
mod tests {
    use cot::test::TestRequestBuilder;
    use cot_core::Body;
    use cot_core::request::extractors::Path;
    use cot_core::response::Response;
    use cot_core::router::Route;

    use super::*;

    #[test]
    fn request_ext_app_name() {
        let mut request = TestRequestBuilder::get("/").build();
        assert_eq!(request.app_name(), None);

        request
            .extensions_mut()
            .insert(AppName("test_app".to_string()));
        assert_eq!(request.app_name(), Some("test_app"));
    }

    #[test]
    fn request_ext_route_name() {
        let mut request = TestRequestBuilder::get("/").build();
        assert_eq!(request.route_name(), None);

        request
            .extensions_mut()
            .insert(RouteName("test_route".to_string()));
        assert_eq!(request.route_name(), Some("test_route"));
    }

    #[test]
    fn request_ext_parts_route_name() {
        let request = TestRequestBuilder::get("/").build();
        let (mut head, _body) = request.into_parts();
        assert_eq!(head.route_name(), None);

        head.extensions.insert(RouteName("test_route".to_string()));
        assert_eq!(head.route_name(), Some("test_route"));
    }

    #[test]
    fn request_ext_path_params() {
        let mut request = TestRequestBuilder::get("/").build();

        let mut params = PathParams::new();
        params.insert("id".to_string(), "42".to_string());
        request.extensions_mut().insert(params);

        assert_eq!(request.path_params().get("id"), Some("42"));
    }

    #[test]
    fn request_ext_path_params_mut() {
        let mut request = TestRequestBuilder::get("/").build();

        request
            .path_params_mut()
            .insert("id".to_string(), "42".to_string());

        assert_eq!(request.path_params().get("id"), Some("42"));
    }

    #[test]
    fn request_ext_content_type() {
        let mut request = TestRequestBuilder::get("/").build();
        assert_eq!(request.content_type(), None);

        request.headers_mut().insert(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("text/plain"),
        );

        assert_eq!(
            request.content_type(),
            Some(&http::HeaderValue::from_static("text/plain"))
        );
    }

    #[test]
    fn request_ext_expect_content_type() {
        let mut request = TestRequestBuilder::get("/").build();

        // Should fail with no content type
        assert!(request.expect_content_type("text/plain").is_err());

        request.headers_mut().insert(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("text/plain"),
        );

        // Should succeed with matching content type
        assert!(request.expect_content_type("text/plain").is_ok());

        // Should fail with non-matching content type
        assert!(request.expect_content_type("application/json").is_err());
    }

    #[cot_macros::test]
    async fn request_ext_extract_from_head() {
        async fn handler(mut request: Request) -> cot_core::Result<Response> {
            let Path(id): Path<String> = request.extract_from_head().await?;
            assert_eq!(id, "42");

            Ok(Response::new(Body::empty()))
        }

        let router = Router::with_urls([Route::with_handler("/{id}/", handler)]);

        let request = TestRequestBuilder::get("/42/")
            .router(router.clone())
            .build();

        router.handle(request).await.unwrap();
    }

    #[test]
    fn parts_ext_path_params() {
        let (mut head, _) = Request::new(crate::Body::empty()).into_parts();
        let mut params = PathParams::new();
        params.insert("id".to_string(), "42".to_string());
        head.extensions.insert(params);

        assert_eq!(head.path_params().get("id"), Some("42"));
    }

    #[test]
    fn parts_ext_mutating_path_params() {
        let (mut head, _) = Request::new(crate::Body::empty()).into_parts();
        head.path_params_mut()
            .insert("page".to_string(), "1".to_string());

        assert_eq!(head.path_params().get("page"), Some("1"));
    }

    #[test]
    fn parts_ext_app_name() {
        let (mut head, _) = Request::new(crate::Body::empty()).into_parts();
        head.extensions.insert(AppName("test_app".to_string()));

        assert_eq!(head.app_name(), Some("test_app"));
    }

    #[test]
    fn parts_ext_route_name() {
        let (mut head, _) = Request::new(crate::Body::empty()).into_parts();
        head.extensions.insert(RouteName("test_route".to_string()));

        assert_eq!(head.route_name(), Some("test_route"));
    }

    #[test]
    fn parts_ext_content_type() {
        let (mut head, _) = Request::new(crate::Body::empty()).into_parts();
        head.headers.insert(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("text/plain"),
        );

        assert_eq!(
            head.content_type(),
            Some(&http::HeaderValue::from_static("text/plain"))
        );
    }
}
