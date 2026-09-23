//! Extractors for request data.
//!
//! An extractor is a function that extracts data from a request. The main
//! benefit of using an extractor is that it can be used directly as a parameter
//! in a route handler.
//!
//! An extractor implements either [`FromRequest`] or [`FromRequestHead`].
//! There are two variants because the request body can only be read once, so it
//! needs to be read in the [`FromRequest`] implementation. Therefore, there can
//! only be one extractor that implements [`FromRequest`] per route handler.
//!
//! # Examples
//!
//! For example, the [`Path`] extractor is used to extract path parameters:
//!
//! ```
//! use cot::html::Html;
//! use cot::request::extractors::{FromRequest, Path};
//! use cot::request::{Request, RequestExt};
//! use cot::router::{Route, Router};
//! use cot::test::TestRequestBuilder;
//!
//! async fn my_handler(Path(my_param): Path<String>) -> Html {
//!     Html::new(format!("Hello {my_param}!"))
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> cot::Result<()> {
//! let router = Router::with_urls([Route::with_handler_and_name(
//!     "/{my_param}/",
//!     my_handler,
//!     "home",
//! )]);
//! let request = TestRequestBuilder::get("/world/")
//!     .router(router.clone())
//!     .build();
//!
//! assert_eq!(
//!     router
//!         .handle(request)
//!         .await?
//!         .into_body()
//!         .into_bytes()
//!         .await?,
//!     "Hello world!"
//! );
//! # Ok(())
//! # }
//! ```

use std::net::IpAddr;
use std::sync::Arc;

use axum::extract::connect_info::Connected;
use axum::serve::IncomingStream;
use cot_core::error::impl_into_cot_error;
use cot_core::remote_addr::extract_cf_connecting_ip;
use cot_core::remote_addr::extract_forwarded;
use cot_core::remote_addr::extract_x_forwarded_for;
use cot_core::remote_addr::extract_x_real_ip;
/// Trait for extractors that consume the request body.
///
/// Extractors implementing this trait are used in route handlers that consume
/// the request body and therefore can only be used once per request.
///
/// See [`crate::request::extractors`] documentation for more information about
/// extractors.
pub use cot_core::request::extractors::FromRequest;
/// Trait for extractors that don't consume the request body.
///
/// Extractors implementing this trait are used in route handlers that don't
/// consume the request and therefore can be used multiple times per request.
///
/// If you need to consume the body of the request, use [`FromRequest`] instead.
///
/// See [`crate::request::extractors`] documentation for more information about
pub use cot_core::request::extractors::FromRequestHead;
#[doc(inline)]
pub use cot_core::request::extractors::{Path, UrlQuery};

use crate::Body;
use crate::auth::Auth;
use crate::form::{Form, FormResult};
use crate::request::{Request, RequestExt, RequestHead};
use crate::router::Urls;
use crate::session::Session;

impl FromRequestHead for Urls {
    #[expect(clippy::unused_async_trait_impl)]
    async fn from_request_head(head: &RequestHead) -> cot::Result<Self> {
        Ok(Self::from_parts(head))
    }
}

/// An extractor that gets the request body as form data and deserializes it
/// into a type `F` implementing [`Form`].
///
/// The content type of the request must be `application/x-www-form-urlencoded`.
///
/// # Errors
///
/// Throws an error if the content type is not
/// `application/x-www-form-urlencoded`. Throws an error if the request body
/// could not be read. Throws an error if the request body could not be
/// deserialized - either because the form data is invalid or because the
/// deserialization to the target structure failed.
///
/// # Example
///
/// ```
/// use cot::form::{Form, FormResult};
/// use cot::html::Html;
/// use cot::request::extractors::RequestForm;
/// use cot::test::TestRequestBuilder;
///
/// #[derive(Form)]
/// struct MyForm {
///     hello: String,
/// }
///
/// async fn my_handler(RequestForm(form): RequestForm<MyForm>) -> Html {
///     let form = match form {
///         FormResult::Ok(form) => form,
///         FormResult::ValidationError(error) => {
///             panic!("Form validation error!")
///         }
///     };
///
///     Html::new(format!("Hello {}!", form.hello))
/// }
///
/// # #[tokio::main]
/// # async fn main() -> cot::Result<()> {
/// # use cot::RequestHandler;
/// # let request = TestRequestBuilder::post("/").form_data(&[("hello", "world")]).build();
/// # my_handler.handle(request).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug)]
pub struct RequestForm<F: Form>(pub FormResult<F>);

impl<F: Form> FromRequest for RequestForm<F> {
    async fn from_request(head: &RequestHead, body: Body) -> cot::Result<Self> {
        let mut request = Request::from_parts(head.clone(), body);
        Ok(Self(F::from_request(&mut request).await?))
    }
}

#[cfg(feature = "db")]
impl FromRequestHead for crate::db::Database {
    #[expect(clippy::unused_async_trait_impl)]
    async fn from_request_head(head: &RequestHead) -> cot::Result<Self> {
        Ok(head.context().database().clone())
    }
}

#[cfg(feature = "cache")]
impl FromRequestHead for crate::cache::Cache {
    #[expect(clippy::unused_async_trait_impl)]
    async fn from_request_head(head: &RequestHead) -> cot::Result<Self> {
        Ok(head.context().cache().clone())
    }
}

#[cfg(feature = "email")]
impl FromRequestHead for crate::email::Email {
    #[expect(clippy::unused_async_trait_impl)]
    async fn from_request_head(head: &RequestHead) -> cot::Result<Self> {
        Ok(head.context().email().clone())
    }
}

/// An extractor that allows you to access static files metadata (e.g., their
/// URLs).
///
/// # Examples
///
/// ```
/// use cot::html::Html;
/// use cot::request::Request;
/// use cot::request::extractors::StaticFiles;
/// use cot::test::TestRequestBuilder;
///
/// async fn my_handler(static_files: StaticFiles) -> cot::Result<Html> {
///     let url = static_files.url_for("css/main.css")?;
///
///     Ok(Html::new(format!(
///         "<html><head><link rel=\"stylesheet\" href=\"{url}\"></head></html>"
///     )))
/// }
///
/// # #[tokio::main]
/// # async fn main() -> cot::Result<()> {
/// # use cot::RequestHandler;
/// # let request = TestRequestBuilder::get("/")
/// #     .static_file("css/main.css", "body { color: red; }")
/// #     .build();
/// # my_handler.handle(request).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticFiles {
    inner: Arc<crate::static_files::StaticFiles>,
}

impl StaticFiles {
    /// Gets the URL for a static file.
    ///
    /// This method returns the URL that can be used to access the static file.
    /// The URL is constructed based on the static files configuration, which
    /// may include a URL prefix or be suffixed by a content hash.
    ///
    /// # Errors
    ///
    /// Returns a [`StaticFilesGetError::NotFound`] error if the file doesn't
    /// exist.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::html::Html;
    /// use cot::request::extractors::StaticFiles;
    /// use cot::test::TestRequestBuilder;
    ///
    /// async fn my_handler(static_files: StaticFiles) -> cot::Result<Html> {
    ///     let url = static_files.url_for("css/main.css")?;
    ///
    ///     Ok(Html::new(format!(
    ///         "<html><head><link rel=\"stylesheet\" href=\"{url}\"></head></html>"
    ///     )))
    /// }
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// # use cot::RequestHandler;
    /// # let request = TestRequestBuilder::get("/")
    /// #     .static_file("css/main.css", "body { color: red; }")
    /// #     .build();
    /// # my_handler.handle(request).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn url_for(&self, path: &str) -> Result<&str, StaticFilesGetError> {
        self.inner
            .path_for(path)
            .ok_or_else(|| StaticFilesGetError::NotFound {
                path: path.to_owned(),
            })
    }
}

const ERROR_PREFIX: &str = "could not get URL for a static file:";
/// Errors that can occur when trying to get a static file.
///
/// This enum represents errors that can occur when attempting to
/// access a static file through the [`StaticFiles`] extractor.
#[derive(Debug, Clone, PartialEq, Eq, Hash, thiserror::Error)]
#[non_exhaustive]
pub enum StaticFilesGetError {
    /// The requested static file was not found.
    #[error("{ERROR_PREFIX} static file `{path}` not found")]
    #[non_exhaustive]
    NotFound {
        /// The path of the static file that was not found.
        path: String,
    },
}
impl_into_cot_error!(StaticFilesGetError);

impl FromRequestHead for StaticFiles {
    #[expect(clippy::unused_async_trait_impl)]
    async fn from_request_head(head: &RequestHead) -> cot::Result<Self> {
        Ok(StaticFiles {
            inner: head
                .extensions
                .get::<Arc<crate::static_files::StaticFiles>>()
                .cloned()
                .expect("StaticFilesMiddleware not enabled for the route/project"),
        })
    }
}

impl FromRequestHead for Session {
    #[expect(clippy::unused_async_trait_impl)]
    async fn from_request_head(head: &RequestHead) -> cot::Result<Self> {
        Ok(Session::from_extensions(&head.extensions).clone())
    }
}

impl FromRequestHead for Auth {
    #[expect(clippy::unused_async_trait_impl)]
    async fn from_request_head(head: &RequestHead) -> cot::Result<Self> {
        let auth = head
            .extensions
            .get::<Auth>()
            .expect("AuthMiddleware not enabled for the route/project")
            .clone();

        Ok(auth)
    }
}

#[derive(Clone, Copy, Debug)]
/// An extractor that extracts the IP address of the remote.
/// This automatically checks for proxy IP headers and contains their IP if one such is specified.
///
/// # Examples
/// ```rust
/// use cot::request::extractors::RemoteAddr;
/// use cot_core::html::Html;
///
/// pub async fn example_handler(ip: RemoteAddr) -> cot::Result<Html> {
///     dbg!(ip.ip()); // Prints the IP as a debug statement
///     dbg!(ip.direct_peer_ip()); // Prints the closest IP as a debug statement
///     todo!()
/// }
/// ```
pub struct RemoteAddr {
    /// IP of the closest peer
    direct: IpAddr,
    /// IP behind the proxy if such exist
    proxied: Option<IpAddr>,
}

impl RemoteAddr {
    #[must_use]
    /// Get the IP address of the peer.
    /// This automatically handles proxy IP headers.
    pub fn ip(&self) -> IpAddr {
        self.proxied.unwrap_or(self.direct)
    }

    #[must_use]
    /// Get the IP address of the peer closest to this server.
    /// This does **not** handle proxies.
    ///
    /// In most use-cases [`Self::ip`] is more appropriate.
    pub fn direct_peer_ip(&self) -> IpAddr {
        self.direct
    }
}

impl<'a> Connected<IncomingStream<'a, tokio::net::TcpListener>> for RemoteAddr {
    fn connect_info(stream: IncomingStream<'a, tokio::net::TcpListener>) -> Self {
        let closest_ip = stream.remote_addr().ip();
        RemoteAddr {
            direct: closest_ip,
            proxied: None,
        }
    }
}

impl FromRequestHead for RemoteAddr {
    async fn from_request_head(head: &RequestHead) -> crate::Result<Self> {
        let addr = head
            .extensions
            .get::<RemoteAddr>()
            .expect("Missing RemoteAddr extension");

        let closest = addr.direct;

        let config = head.project_config().clone().client_ip;
        let trusted_proxies = config.get_proxies();
        let trusted_headers = config.get_trusted_headers();

        let mut proxy: Option<IpAddr> = None;

        if trusted_proxies.iter().any(|proxy| proxy.contains(&closest)) {
            for h in trusted_headers {
                if let Some(v) = head.headers.get(&h.to_string())
                    && proxy.is_none()
                {
                    match h {
                        crate::config::ClientIpHeader::Forwarded => {
                            proxy = extract_forwarded(v)?;
                        }
                        crate::config::ClientIpHeader::XForwardedFor => {
                            proxy = extract_x_forwarded_for(v)?;
                        }
                        crate::config::ClientIpHeader::CfConnectingIp
                        | crate::config::ClientIpHeader::TrueClientIp => {
                            proxy = Some(extract_cf_connecting_ip(v)?);
                        }
                        crate::config::ClientIpHeader::XRealIp => {
                            proxy = Some(extract_x_real_ip(v)?);
                        }
                    }
                }
            }
        }

        Ok(RemoteAddr {
            direct: addr.direct,
            proxied: proxy,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr};
    use std::str::FromStr;

    use cot_core::Method;
    use cot_core::html::Html;
    use http::{HeaderName, HeaderValue};

    use super::*;
    use crate::config::{ClientIpConfig, ClientIpHeader, ProjectConfig};
    use crate::request::extractors::FromRequest;
    use crate::reverse;
    use crate::router::{Route, Router};
    use crate::test::TestRequestBuilder;

    #[cot::test]
    async fn urls_extraction() {
        async fn handler() -> Html {
            Html::new("")
        }

        let router = Router::with_urls([Route::with_handler_and_name(
            "/test/",
            handler,
            "test_route",
        )]);

        let mut request = TestRequestBuilder::get("/test/").router(router).build();

        let urls: Urls = request.extract_from_head().await.unwrap();

        assert!(reverse!(urls, "test_route").is_ok());
    }

    #[cot::test]
    async fn method_extraction() {
        let mut request = TestRequestBuilder::get("/test/").build();

        let method: Method = request.extract_from_head().await.unwrap();

        assert_eq!(method, Method::GET);
    }
    #[cot::test]
    async fn request_form() {
        #[derive(Debug, PartialEq, Eq, Form)]
        struct MyForm {
            hello: String,
            foo: String,
        }

        let request = TestRequestBuilder::post("/")
            .form_data(&[("hello", "world"), ("foo", "bar")])
            .build();

        let (head, body) = request.into_parts();
        let RequestForm(form_result): RequestForm<MyForm> =
            RequestForm::from_request(&head, body).await.unwrap();

        assert_eq!(
            form_result.unwrap(),
            MyForm {
                hello: "world".to_string(),
                foo: "bar".to_string(),
            }
        );
    }

    #[cfg(feature = "db")]
    #[cot::test]
    #[cfg_attr(
        miri,
        ignore = "unsupported operation: can't call foreign function `sqlite3_open_v2` on OS `linux`"
    )]
    async fn request_db() {
        let db = crate::test::TestDatabase::new_sqlite().await.unwrap();
        let mut test_request = TestRequestBuilder::get("/").database(db.database()).build();

        let extracted_db: crate::db::Database = test_request.extract_from_head().await.unwrap();

        // check that we have a connection to the database
        extracted_db.close().await.unwrap();
    }

    #[cfg(feature = "cache")]
    #[cot::test]
    async fn request_cache() {
        let mut request_builder = TestRequestBuilder::get("/");
        let mut request = request_builder.build();

        let extracted_cache = request.extract_from_head::<crate::cache::Cache>().await;
        assert!(extracted_cache.is_ok());
    }

    #[cfg(feature = "email")]
    #[cot::test]
    async fn request_email() {
        let mut request_builder = TestRequestBuilder::get("/");
        let mut request = request_builder.build();

        let email_service = request.extract_from_head::<crate::email::Email>().await;
        assert!(email_service.is_ok());
    }

    #[cot::test]
    async fn remote_addr() {
        const IP: IpAddr = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));

        let mut request = TestRequestBuilder::get("/").with_default_config().build();
        request.extensions_mut().insert(RemoteAddr {
            direct: IP,
            proxied: None,
        });
        let RemoteAddr { direct, proxied: _ } = request.extract_from_head().await.unwrap();

        assert_eq!(direct, IP);
    }

    #[cot::test]
    async fn remote_addr_v6() {
        const IP: IpAddr = IpAddr::V6(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8));

        let mut request = TestRequestBuilder::get("/").with_default_config().build();
        request.extensions_mut().insert(RemoteAddr {
            direct: IP,
            proxied: None,
        });
        let RemoteAddr { direct, proxied: _ } = request.extract_from_head().await.unwrap();

        assert_eq!(direct, IP);
    }

    #[cot::test]
    async fn remote_addr_proxied_unconfigured() {
        const IP: IpAddr = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));

        let mut request = TestRequestBuilder::get("/").with_default_config().build();
        request.extensions_mut().insert(RemoteAddr {
            direct: IP,
            proxied: None,
        });
        let RemoteAddr { direct: _, proxied } = request.extract_from_head().await.unwrap();

        assert_eq!(proxied, None);
    }

    #[cot::test]
    async fn remote_addr_proxied_configured() {
        const IP: IpAddr = IpAddr::V6(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8));
        const IP_PROXY: IpAddr = IpAddr::V6(Ipv6Addr::new(21, 32, 43, 54, 65, 76, 87, 98));

        let mut config = ProjectConfig::dev_default();

        config.client_ip = ClientIpConfig::new(
            vec![ipnet::IpNet::new(IP, 64).unwrap()],
            vec![ClientIpHeader::Forwarded],
        );

        let mut request = TestRequestBuilder::get("/").config(config).build();
        request.extensions_mut().insert(RemoteAddr {
            direct: IP,
            proxied: None,
        });
        request.headers_mut().insert(
            HeaderName::from_str("Forwarded").unwrap(),
            HeaderValue::from_str(&format!("for={IP_PROXY}")).unwrap(),
        );

        let ip = request
            .extract_from_head::<RemoteAddr>()
            .await
            .unwrap()
            .ip();

        assert_eq!(ip, IP_PROXY);
    }

    #[cot::test]
    async fn remote_addr_proxied_configured_b() {
        const IP: IpAddr = IpAddr::V6(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8));
        const IP_PROXY: IpAddr = IpAddr::V6(Ipv6Addr::new(21, 32, 43, 54, 65, 76, 87, 98));

        let mut config = ProjectConfig::dev_default();

        config.client_ip = ClientIpConfig::new(
            vec![ipnet::IpNet::new(IP, 128).unwrap()],
            vec![ClientIpHeader::XForwardedFor],
        );

        let mut request = TestRequestBuilder::get("/").config(config).build();
        request.extensions_mut().insert(RemoteAddr {
            direct: IP,
            proxied: None,
        });
        request.headers_mut().insert(
            HeaderName::from_str("Forwarded").unwrap(),
            HeaderValue::from_str(&format!("for={IP_PROXY}")).unwrap(),
        );

        let ip = request
            .extract_from_head::<RemoteAddr>()
            .await
            .unwrap()
            .ip();

        assert_eq!(ip, IP);
    }

    #[cot::test]
    async fn remote_addr_proxied_configured_c() {
        const IP: IpAddr = IpAddr::V6(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8));
        const IP_WRONG: IpAddr = IpAddr::V6(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 9));
        const IP_PROXY: IpAddr = IpAddr::V6(Ipv6Addr::new(21, 32, 43, 54, 65, 76, 87, 98));

        let mut config = ProjectConfig::dev_default();

        config.client_ip = ClientIpConfig::new(
            vec![ipnet::IpNet::new(IP, 128).unwrap()],
            vec![ClientIpHeader::Forwarded],
        );

        let mut request = TestRequestBuilder::get("/").config(config).build();
        request.extensions_mut().insert(RemoteAddr {
            direct: IP_WRONG,
            proxied: None,
        });
        request.headers_mut().insert(
            HeaderName::from_str("Forwarded").unwrap(),
            HeaderValue::from_str(&format!("for={IP_PROXY}")).unwrap(),
        );

        let ip = request
            .extract_from_head::<RemoteAddr>()
            .await
            .unwrap()
            .ip();

        assert_eq!(ip, IP_WRONG);
    }

    #[cot::test]
    async fn remote_addr_proxied_configured_c_in_range() {
        const IP: IpAddr = IpAddr::V6(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8));
        const IP_WRONG: IpAddr = IpAddr::V6(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 9));
        const IP_PROXY: IpAddr = IpAddr::V6(Ipv6Addr::new(21, 32, 43, 54, 65, 76, 87, 98));

        let mut config = ProjectConfig::dev_default();

        config.client_ip = ClientIpConfig::new(
            vec![ipnet::IpNet::new(IP, 64).unwrap()],
            vec![ClientIpHeader::Forwarded],
        );

        let mut request = TestRequestBuilder::get("/").config(config).build();
        request.extensions_mut().insert(RemoteAddr {
            direct: IP_WRONG,
            proxied: None,
        });
        request.headers_mut().insert(
            HeaderName::from_str("Forwarded").unwrap(),
            HeaderValue::from_str(&format!("for={IP_PROXY}")).unwrap(),
        );

        let ip = request
            .extract_from_head::<RemoteAddr>()
            .await
            .unwrap()
            .ip();

        assert_eq!(ip, IP_PROXY);
    }

    #[cot::test]
    async fn remote_addr_proxied_configured_d() {
        const IP: IpAddr = IpAddr::V6(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8));
        const IP_PROXY: IpAddr = IpAddr::V6(Ipv6Addr::new(21, 32, 43, 54, 65, 76, 87, 98));

        let mut config = ProjectConfig::dev_default();

        config.client_ip = ClientIpConfig::new(
            vec![ipnet::IpNet::new(IP, 128).unwrap()],
            vec![ClientIpHeader::Forwarded],
        );

        let mut request = TestRequestBuilder::get("/").config(config).build();
        request.extensions_mut().insert(RemoteAddr {
            direct: IP,
            proxied: None,
        });
        request.headers_mut().insert(
            HeaderName::from_str("X-Forwarded-For").unwrap(),
            HeaderValue::from_str(&format!("{IP_PROXY}")).unwrap(),
        );

        let ip = request
            .extract_from_head::<RemoteAddr>()
            .await
            .unwrap()
            .ip();

        assert_eq!(ip, IP);
    }

    #[cot::test]
    async fn remote_addr_proxied_configured_e() {
        const IP: IpAddr = IpAddr::V6(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8));
        const IP_PROXY: IpAddr = IpAddr::V6(Ipv6Addr::new(21, 32, 43, 54, 65, 76, 87, 98));

        let mut config = ProjectConfig::dev_default();

        config.client_ip = ClientIpConfig::new(
            vec![ipnet::IpNet::new(IP, 128).unwrap()],
            vec![ClientIpHeader::XForwardedFor],
        );

        let mut request = TestRequestBuilder::get("/").config(config).build();
        request.extensions_mut().insert(RemoteAddr {
            direct: IP,
            proxied: None,
        });
        request.headers_mut().insert(
            HeaderName::from_str("X-Forwarded-For").unwrap(),
            HeaderValue::from_str(&format!("{IP_PROXY}")).unwrap(),
        );

        let ip = request
            .extract_from_head::<RemoteAddr>()
            .await
            .unwrap()
            .ip();

        assert_eq!(ip, IP_PROXY);
    }

    #[cot::test]
    async fn remote_addr_proxied_configured_f() {
        const IP: IpAddr = IpAddr::V6(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8));
        const IP_PROXY: IpAddr = IpAddr::V6(Ipv6Addr::new(21, 32, 43, 54, 65, 76, 87, 98));

        let mut config = ProjectConfig::dev_default();

        config.client_ip = ClientIpConfig::new(
            vec![ipnet::IpNet::new(IP, 128).unwrap()],
            vec![
                ClientIpHeader::XForwardedFor,
                ClientIpHeader::CfConnectingIp,
            ],
        );

        let mut request = TestRequestBuilder::get("/").config(config).build();
        request.extensions_mut().insert(RemoteAddr {
            direct: IP,
            proxied: None,
        });
        request.headers_mut().insert(
            HeaderName::from_str("CF-Connecting-IP").unwrap(),
            HeaderValue::from_str(&format!("{IP_PROXY}")).unwrap(),
        );

        let ip = request
            .extract_from_head::<RemoteAddr>()
            .await
            .unwrap()
            .ip();

        assert_eq!(ip, IP_PROXY);
    }

    #[cot::test]
    async fn remote_addr_proxied_configured_malformed() {
        const IP: IpAddr = IpAddr::V6(Ipv6Addr::new(1, 2, 3, 4, 5, 6, 7, 8));
        const IP_PROXY: IpAddr = IpAddr::V6(Ipv6Addr::new(21, 32, 43, 54, 65, 76, 87, 98));

        let mut config = ProjectConfig::dev_default();

        config.client_ip = ClientIpConfig::new(
            vec![ipnet::IpNet::new(IP, 128).unwrap()],
            vec![
                ClientIpHeader::XForwardedFor,
                ClientIpHeader::CfConnectingIp,
            ],
        );

        let mut request = TestRequestBuilder::get("/").config(config).build();
        request.extensions_mut().insert(RemoteAddr {
            direct: IP,
            proxied: None,
        });
        request.headers_mut().insert(
            HeaderName::from_str("CF-Connecting-IP").unwrap(),
            HeaderValue::from_str(&format!("AAA{IP_PROXY}")).unwrap(),
        );

        let ip = request.extract_from_head::<RemoteAddr>().await;

        assert!(ip.is_err());
    }
}
