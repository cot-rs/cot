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

#[repr(transparent)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AppName(pub String);
