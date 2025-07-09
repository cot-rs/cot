//! HTTP request type and helper methods.
//!
//! Cot uses the [`Request`](http::Request) type from the [`http`] crate
//! to represent incoming HTTP requests. However, it also provides a
//! [`RequestExt`] trait that contain various helper methods for working with
//! HTTP requests. These methods are used to access the application context,
//! project configuration, path parameters, and more. You probably want to have
//! a `use` statement for [`RequestExt`] in your code most of the time to be
//! able to use these functions:
//!
//! ```
//! use cot::request::RequestExt;
//! ```

use std::future::Future;
use std::sync::Arc;

use http::Extensions;
use indexmap::IndexMap;

#[cfg(feature = "db")]
use crate::db::Database;
use crate::error::ErrorRepr;
use crate::request::extractors::FromRequestHead;
use crate::router::Router;
use crate::{Body, Result};

pub mod extractors;
mod path_params_deserializer;

/// HTTP request type.
pub type Request = http::Request<Body>;

/// HTTP request head type.
pub type RequestHead = http::request::Parts;

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
    /// use cot::request::extractors::Path;
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::Response;
    ///
    /// async fn my_handler(mut request: Request) -> cot::Result<Response> {
    ///     let path_params = request.extract_from_head::<Path<String>>().await?;
    ///     // ...
    ///     # unimplemented!()
    /// }
    /// ```
    fn extract_from_head<E>(&mut self) -> impl Future<Output = Result<E>> + Send
    where
        E: FromRequestHead + 'static;

    /// Get the application context.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::Response;
    ///
    /// async fn my_handler(mut request: Request) -> cot::Result<Response> {
    ///     let context = request.context();
    ///     // ... do something with the context
    ///     # unimplemented!()
    /// }
    /// ```
    #[must_use]
    fn context(&self) -> &crate::ProjectContext;

    /// Get the project configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::Response;
    ///
    /// async fn my_handler(mut request: Request) -> cot::Result<Response> {
    ///     let config = request.project_config();
    ///     // ... do something with the config
    ///     # unimplemented!()
    /// }
    /// ```
    #[must_use]
    fn project_config(&self) -> &crate::config::ProjectConfig;

    /// Get the router.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::Response;
    ///
    /// async fn my_handler(mut request: Request) -> cot::Result<Response> {
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
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::Response;
    ///
    /// async fn my_handler(mut request: Request) -> cot::Result<Response> {
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
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::Response;
    ///
    /// async fn my_handler(mut request: Request) -> cot::Result<Response> {
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
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::Response;
    ///
    /// async fn my_handler(mut request: Request) -> cot::Result<Response> {
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
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::Response;
    ///
    /// async fn my_handler(mut request: Request) -> cot::Result<Response> {
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
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::Response;
    ///
    /// async fn my_handler(mut request: Request) -> cot::Result<Response> {
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
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::Response;
    ///
    /// async fn my_handler(mut request: Request) -> cot::Result<Response> {
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
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::Response;
    ///
    /// async fn my_handler(mut request: Request) -> cot::Result<Response> {
    ///     request.expect_content_type("application/json")?;
    ///     // ...
    ///     # unimplemented!()
    /// }
    /// ```
    fn expect_content_type(&mut self, expected: &'static str) -> Result<()> {
        let content_type = self
            .content_type()
            .map_or("".into(), |value| String::from_utf8_lossy(value.as_bytes()));
        if content_type == expected {
            Ok(())
        } else {
            Err(ErrorRepr::InvalidContentType {
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
    async fn extract_from_head<E>(&mut self) -> Result<E>
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
    fn context(&self) -> &crate::ProjectContext {
        self.extensions()
            .get::<Arc<crate::ProjectContext>>()
            .expect("AppContext extension missing")
    }

    fn project_config(&self) -> &crate::config::ProjectConfig {
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
    async fn extract_from_head<E>(&mut self) -> Result<E>
    where
        E: FromRequestHead + 'static,
    {
        E::from_request_head(self).await
    }

    fn context(&self) -> &crate::ProjectContext {
        self.extensions
            .get::<Arc<crate::ProjectContext>>()
            .expect("AppContext extension missing")
    }

    fn project_config(&self) -> &crate::config::ProjectConfig {
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

#[repr(transparent)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct AppName(pub(crate) String);

#[repr(transparent)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct RouteName(pub(crate) String);

/// Path parameters extracted from the request URL, and available as a map of
/// strings.
///
/// This struct is meant to be mainly used using the [`PathParams::parse`]
/// method, which will deserialize the path parameters into a type `T`
/// implementing `serde::DeserializeOwned`. If needed, you can also access the
/// path parameters directly using the [`PathParams::get`] method.
///
/// # Examples
///
/// ```
/// use cot::request::{PathParams, Request, RequestExt};
/// use cot::response::Response;
/// use cot::test::TestRequestBuilder;
///
/// async fn my_handler(mut request: Request) -> cot::Result<Response> {
///     let path_params = request.path_params();
///     let name = path_params.get("name").unwrap();
///
///     // using more ergonomic syntax:
///     let name: String = request.path_params().parse()?;
///
///     let name = println!("Hello, {}!", name);
///     // ...
///     # unimplemented!()
/// }
/// ```
#[derive(Debug, Clone)]
pub struct PathParams {
    params: IndexMap<String, String>,
}

impl Default for PathParams {
    fn default() -> Self {
        Self::new()
    }
}

impl PathParams {
    /// Creates a new [`PathParams`] instance.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::PathParams;
    ///
    /// let mut path_params = PathParams::new();
    /// path_params.insert("name".into(), "world".into());
    /// assert_eq!(path_params.get("name"), Some("world"));
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            params: IndexMap::new(),
        }
    }

    /// Inserts a new path parameter.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::PathParams;
    ///
    /// let mut path_params = PathParams::new();
    /// path_params.insert("name".into(), "world".into());
    /// assert_eq!(path_params.get("name"), Some("world"));
    /// ```
    pub fn insert(&mut self, name: String, value: String) {
        self.params.insert(name, value);
    }

    /// Iterates over the path parameters.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::PathParams;
    ///
    /// let mut path_params = PathParams::new();
    /// path_params.insert("name".into(), "world".into());
    /// for (name, value) in path_params.iter() {
    ///     println!("{}: {}", name, value);
    /// }
    /// ```
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.params
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }

    /// Returns the number of path parameters.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::PathParams;
    ///
    /// let path_params = PathParams::new();
    /// assert_eq!(path_params.len(), 0);
    /// ```
    #[must_use]
    pub fn len(&self) -> usize {
        self.params.len()
    }

    /// Returns `true` if the path parameters are empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::PathParams;
    ///
    /// let path_params = PathParams::new();
    /// assert!(path_params.is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.params.is_empty()
    }

    /// Returns the value of a path parameter.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::PathParams;
    ///
    /// let mut path_params = PathParams::new();
    /// path_params.insert("name".into(), "world".into());
    /// assert_eq!(path_params.get("name"), Some("world"));
    /// ```
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.params.get(name).map(String::as_str)
    }

    /// Returns the value of a path parameter at the given index.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::PathParams;
    ///
    /// let mut path_params = PathParams::new();
    /// path_params.insert("name".into(), "world".into());
    /// assert_eq!(path_params.get_index(0), Some("world"));
    /// ```
    #[must_use]
    pub fn get_index(&self, index: usize) -> Option<&str> {
        self.params
            .get_index(index)
            .map(|(_, value)| value.as_str())
    }

    /// Returns the key of a path parameter at the given index.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::PathParams;
    ///
    /// let mut path_params = PathParams::new();
    /// path_params.insert("name".into(), "world".into());
    /// assert_eq!(path_params.key_at_index(0), Some("name"));
    /// ```
    #[must_use]
    pub fn key_at_index(&self, index: usize) -> Option<&str> {
        self.params.get_index(index).map(|(key, _)| key.as_str())
    }

    /// Deserializes the path parameters into a type `T` implementing
    /// `serde::DeserializeOwned`.
    ///
    /// # Errors
    ///
    /// Throws an error if the path parameters could not be deserialized.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::PathParams;
    ///
    /// # fn main() -> Result<(), cot::Error> {
    /// let mut path_params = PathParams::new();
    /// path_params.insert("hello".into(), "world".into());
    ///
    /// let hello: String = path_params.parse()?;
    /// assert_eq!(hello, "world");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ```
    /// use cot::request::PathParams;
    ///
    /// # fn main() -> Result<(), cot::Error> {
    /// let mut path_params = PathParams::new();
    /// path_params.insert("hello".into(), "world".into());
    /// path_params.insert("name".into(), "john".into());
    ///
    /// let (hello, name): (String, String) = path_params.parse()?;
    /// assert_eq!(hello, "world");
    /// assert_eq!(name, "john");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ```
    /// use cot::request::PathParams;
    /// use serde::Deserialize;
    ///
    /// # fn main() -> Result<(), cot::Error> {
    /// let mut path_params = PathParams::new();
    /// path_params.insert("hello".into(), "world".into());
    /// path_params.insert("name".into(), "john".into());
    ///
    /// #[derive(Deserialize)]
    /// struct Params {
    ///     hello: String,
    ///     name: String,
    /// }
    ///
    /// let params: Params = path_params.parse()?;
    /// assert_eq!(params.hello, "world");
    /// assert_eq!(params.name, "john");
    /// # Ok(())
    /// # }
    /// ```
    pub fn parse<'de, T: serde::Deserialize<'de>>(
        &'de self,
    ) -> std::result::Result<T, PathParamsDeserializerError> {
        let deserializer = path_params_deserializer::PathParamsDeserializer::new(self);
        serde_path_to_error::deserialize(deserializer).map_err(PathParamsDeserializerError)
    }
}

/// An error that occurs when deserializing path parameters.
#[derive(Debug, Clone, thiserror::Error)]
#[error("{0}")]
pub struct PathParamsDeserializerError(
    // A wrapper over the original deserializer error. The exact error reason
    // shouldn't be useful to the user, hence we're not exposing it.
    #[source] serde_path_to_error::Error<path_params_deserializer::PathParamsDeserializerError>,
);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::extractors::Path;
    use crate::response::Response;
    use crate::router::{Route, Router};
    use crate::test::TestRequestBuilder;

    #[test]
    fn path_params() {
        let mut path_params = PathParams::new();
        path_params.insert("name".into(), "world".into());

        assert_eq!(path_params.get("name"), Some("world"));
        assert_eq!(path_params.get("missing"), None);
    }

    #[test]
    fn path_params_parse() {
        #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
        struct Params {
            hello: String,
            foo: String,
        }

        let mut path_params = PathParams::new();
        path_params.insert("hello".into(), "world".into());
        path_params.insert("foo".into(), "bar".into());

        let params: Params = path_params.parse().unwrap();
        assert_eq!(
            params,
            Params {
                hello: "world".to_string(),
                foo: "bar".to_string(),
            }
        );
    }

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

    #[cot::test]
    async fn request_ext_extract_from_head() {
        async fn handler(mut request: Request) -> Result<Response> {
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
        let (mut head, _) = Request::new(Body::empty()).into_parts();
        let mut params = PathParams::new();
        params.insert("id".to_string(), "42".to_string());
        head.extensions.insert(params);

        assert_eq!(head.path_params().get("id"), Some("42"));
    }

    #[test]
    fn parts_ext_mutating_path_params() {
        let (mut head, _) = Request::new(Body::empty()).into_parts();
        head.path_params_mut()
            .insert("page".to_string(), "1".to_string());

        assert_eq!(head.path_params().get("page"), Some("1"));
    }

    #[test]
    fn parts_ext_app_name() {
        let (mut head, _) = Request::new(Body::empty()).into_parts();
        head.extensions.insert(AppName("test_app".to_string()));

        assert_eq!(head.app_name(), Some("test_app"));
    }

    #[test]
    fn parts_ext_route_name() {
        let (mut head, _) = Request::new(Body::empty()).into_parts();
        head.extensions.insert(RouteName("test_route".to_string()));

        assert_eq!(head.route_name(), Some("test_route"));
    }

    #[test]
    fn parts_ext_content_type() {
        let (mut head, _) = Request::new(Body::empty()).into_parts();
        head.headers.insert(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("text/plain"),
        );

        assert_eq!(
            head.content_type(),
            Some(&http::HeaderValue::from_static("text/plain"))
        );
    }

    #[cot::test]
    async fn path_extract_from_head() {
        let (mut head, _) = Request::new(Body::empty()).into_parts();

        let mut params = PathParams::new();
        params.insert("id".to_string(), "42".to_string());
        head.extensions.insert(params);

        let Path(id): Path<String> = head.extract_from_head().await.unwrap();
        assert_eq!(id, "42");
    }
}
