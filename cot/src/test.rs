//! Test utilities for Cot projects.

use std::any::Any;
use std::future::poll_fn;
use std::marker::PhantomData;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;

use async_trait::async_trait;
#[cfg(feature = "cache")]
use cot::config::CacheUrl;
#[cfg(feature = "redis")]
use deadpool_redis::Connection;
use derive_more::Debug;
#[cfg(feature = "redis")]
use redis::AsyncCommands;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tower::Service;
use tower_sessions::MemoryStore;

#[cfg(feature = "db")]
use crate::auth::db::DatabaseUserBackend;
use crate::auth::{Auth, AuthBackend, NoAuthBackend, User, UserId};
#[cfg(feature = "cache")]
use crate::cache::Cache;
#[cfg(feature = "cache")]
use crate::cache::store::memory::Memory;
#[cfg(feature = "redis")]
use crate::cache::store::redis::Redis;
use crate::config::ProjectConfig;
#[cfg(feature = "cache")]
use crate::config::Timeout;
#[cfg(feature = "db")]
use crate::db::Database;
#[cfg(feature = "db")]
use crate::db::migrations::{
    DynMigration, MigrationDependency, MigrationEngine, MigrationWrapper, Operation,
};
#[cfg(feature = "redis")]
use crate::error::error_impl::impl_into_cot_error;
use crate::handler::BoxedHandler;
use crate::project::{prepare_request, prepare_request_for_error_handler, run_at_with_shutdown};
use crate::request::Request;
use crate::response::Response;
use crate::router::Router;
use crate::session::Session;
use crate::static_files::{StaticFile, StaticFiles};
use crate::{Body, Bootstrapper, Project, ProjectContext, Result};

/// A test client for making requests to a Cot project.
///
/// Useful for End-to-End testing Cot projects.
#[derive(Debug)]
pub struct Client {
    context: Arc<ProjectContext>,
    handler: BoxedHandler,
    error_handler: BoxedHandler,
}

impl Client {
    /// Create a new test client for a Cot project.
    ///
    /// # Panics
    ///
    /// Panics if the test config could not be loaded.
    /// Panics if the project could not be initialized.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::test::Client;
    /// use cot::Project;
    ///    use cot::config::ProjectConfig;
    ///
    /// struct MyProject;
    /// impl Project for MyProject {
    ///     fn config(&self, config_name: &str) -> cot::Result<ProjectConfig> {
    ///         Ok(ProjectConfig::default())
    ///     }
    /// }
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let mut client = Client::new(MyProject).await;
    /// let response = client.get("/").await?;
    /// assert!(!response.into_body().into_bytes().await?.is_empty());
    /// # Ok(())
    /// }
    /// ```
    #[must_use]
    #[expect(clippy::future_not_send)] // used in the test code
    pub async fn new<P>(project: P) -> Self
    where
        P: Project + 'static,
    {
        let config = project.config("test").expect("Could not get test config");
        let bootstrapper = Bootstrapper::new(project)
            .with_config(config)
            .boot()
            .await
            .expect("Could not boot project");

        let bootstrapped_project = bootstrapper.finish();
        Self {
            context: Arc::new(bootstrapped_project.context),
            handler: bootstrapped_project.handler,
            error_handler: bootstrapped_project.error_handler,
        }
    }

    /// Send a GET request to the given path.
    ///
    /// # Errors
    ///
    /// Propagates any errors that the request handler might return.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::test::Client;
    /// use cot::Project;
    ///    use cot::config::ProjectConfig;
    ///
    /// struct MyProject;
    /// impl Project for MyProject {
    ///     fn config(&self, config_name: &str) -> cot::Result<ProjectConfig> {
    ///         Ok(ProjectConfig::default())
    ///     }
    /// }
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let mut client = Client::new(MyProject).await;
    /// let response = client.get("/").await?;
    /// assert!(!response.into_body().into_bytes().await?.is_empty());
    /// # Ok(())
    /// }
    /// ```
    pub async fn get(&mut self, path: &str) -> Result<Response> {
        self.request(match http::Request::get(path).body(Body::empty()) {
            Ok(request) => request,
            Err(_) => {
                unreachable!("Test request should be valid")
            }
        })
        .await
    }

    /// Send a request to the given path.
    ///
    /// # Errors
    ///
    /// Propagates any errors that the request handler might return.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::test::Client;
    /// use cot::{Body, Project};
    /// use cot::config::ProjectConfig;
    ///
    /// struct MyProject;
    /// impl Project for MyProject {
    ///     fn config(&self, config_name: &str) -> cot::Result<ProjectConfig> {
    ///         Ok(ProjectConfig::default())
    ///     }
    /// }
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let mut client = Client::new(MyProject).await;
    /// let response = client.request(cot::http::Request::get("/").body(Body::empty()).unwrap()).await?;
    /// assert!(!response.into_body().into_bytes().await?.is_empty());
    /// # Ok(())
    /// }
    /// ```
    pub async fn request(&mut self, mut request: Request) -> Result<Response> {
        prepare_request(&mut request, self.context.clone());
        let (head, body) = request.into_parts();
        let mut error_head = head.clone();
        let request = Request::from_parts(head, body);

        poll_fn(|cx| self.handler.poll_ready(cx)).await?;
        match self.handler.call(request).await {
            Ok(result) => Ok(result),
            Err(error) => {
                prepare_request_for_error_handler(&mut error_head, error);
                let request = Request::from_parts(error_head, Body::empty());

                poll_fn(|cx| self.error_handler.poll_ready(cx)).await?;
                self.error_handler.call(request).await
            }
        }
    }
}

/// A builder for creating test requests, typically used for unit testing
/// without having to create a full Cot project and do actual HTTP requests.
///
/// # Examples
///
/// ```
/// use cot::html::Html;
/// use cot::request::Request;
/// use cot::test::TestRequestBuilder;
///
/// # #[tokio::main]
/// # async fn main() -> cot::Result<()> {
/// async fn index(request: Request) -> Html {
///     Html::new("Hello world!")
/// }
///
/// let request = TestRequestBuilder::get("/").build();
///
/// assert_eq!(index(request).await, Html::new("Hello world!"));
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct TestRequestBuilder {
    method: http::Method,
    url: String,
    router: Option<Router>,
    session: Option<Session>,
    config: Option<Arc<ProjectConfig>>,
    auth_backend: Option<AuthBackendWrapper>,
    auth: Option<Auth>,
    #[cfg(feature = "db")]
    database: Option<Database>,
    form_data: Option<Vec<(String, String)>>,
    #[cfg(feature = "json")]
    json_data: Option<String>,
    static_files: Vec<StaticFile>,
    #[cfg(feature = "cache")]
    cache: Option<Cache>,
}

/// A wrapper over an auth backend that is cloneable.
#[derive(Debug, Clone)]
struct AuthBackendWrapper {
    #[debug("..")]
    inner: Arc<dyn AuthBackend>,
}

impl AuthBackendWrapper {
    pub(crate) fn new<AB>(inner: AB) -> Self
    where
        AB: AuthBackend + 'static,
    {
        Self {
            inner: Arc::new(inner),
        }
    }
}

#[async_trait]
impl AuthBackend for AuthBackendWrapper {
    async fn authenticate(
        &self,
        credentials: &(dyn Any + Send + Sync),
    ) -> cot::auth::Result<Option<Box<dyn User + Send + Sync>>> {
        self.inner.authenticate(credentials).await
    }

    async fn get_by_id(
        &self,
        id: UserId,
    ) -> cot::auth::Result<Option<Box<dyn User + Send + Sync>>> {
        self.inner.get_by_id(id).await
    }
}

impl Default for TestRequestBuilder {
    fn default() -> Self {
        Self {
            method: http::Method::GET,
            url: "/".to_string(),
            router: None,
            session: None,
            config: None,
            auth_backend: None,
            auth: None,
            #[cfg(feature = "db")]
            database: None,
            form_data: None,
            #[cfg(feature = "json")]
            json_data: None,
            static_files: Vec::new(),
            #[cfg(feature = "cache")]
            cache: None,
        }
    }
}

impl TestRequestBuilder {
    /// Create a new GET request builder.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::RequestHandler;
    /// use cot::html::Html;
    /// use cot::test::TestRequestBuilder;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// async fn index() -> Html {
    ///     Html::new("Hello world!")
    /// }
    ///
    /// let request = TestRequestBuilder::get("/").build();
    ///
    /// assert_eq!(
    ///     index
    ///         .handle(request)
    ///         .await?
    ///         .into_body()
    ///         .into_bytes()
    ///         .await?,
    ///     "Hello world!"
    /// );
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn get(url: &str) -> Self {
        Self::with_method(url, crate::Method::GET)
    }

    /// Create a new POST request builder.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::RequestHandler;
    /// use cot::html::Html;
    /// use cot::test::TestRequestBuilder;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// async fn index() -> Html {
    ///     Html::new("Hello world!")
    /// }
    ///
    /// let request = TestRequestBuilder::post("/").build();
    ///
    /// assert_eq!(
    ///     index
    ///         .handle(request)
    ///         .await?
    ///         .into_body()
    ///         .into_bytes()
    ///         .await?,
    ///     "Hello world!"
    /// );
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn post(url: &str) -> Self {
        Self::with_method(url, crate::Method::POST)
    }

    /// Create a new request builder with given HTTP method.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::RequestHandler;
    /// use cot::html::Html;
    /// use cot::http::Method;
    /// use cot::test::TestRequestBuilder;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// async fn index() -> Html {
    ///     Html::new("Resource deleted!")
    /// }
    ///
    /// let request = TestRequestBuilder::with_method("/", Method::DELETE).build();
    ///
    /// assert_eq!(
    ///     index
    ///         .handle(request)
    ///         .await?
    ///         .into_body()
    ///         .into_bytes()
    ///         .await?,
    ///     "Resource deleted!"
    /// );
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn with_method(url: &str, method: crate::Method) -> Self {
        Self {
            method,
            url: url.to_string(),
            ..Self::default()
        }
    }

    /// Add a project config instance to the request builder.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::ProjectConfig;
    /// use cot::test::TestRequestBuilder;
    ///
    /// let request = TestRequestBuilder::get("/")
    ///     .config(ProjectConfig::dev_default())
    ///     .build();
    /// ```
    pub fn config(&mut self, config: ProjectConfig) -> &mut Self {
        self.config = Some(Arc::new(config));
        self
    }

    /// Create a new request builder with default configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::html::Html;
    /// use cot::request::Request;
    /// use cot::test::TestRequestBuilder;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// async fn index(request: Request) -> Html {
    ///     Html::new("Hello world!")
    /// }
    ///
    /// let request = TestRequestBuilder::get("/").with_default_config().build();
    ///
    /// assert_eq!(index(request).await, Html::new("Hello world!"));
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_default_config(&mut self) -> &mut Self {
        self.config = Some(Arc::new(ProjectConfig::default()));
        self
    }

    /// Add an authentication backend to the request builder.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::auth::NoAuthBackend;
    /// use cot::test::TestRequestBuilder;
    ///
    /// let request = TestRequestBuilder::get("/")
    ///     .auth_backend(NoAuthBackend)
    ///     .build();
    /// ```
    pub fn auth_backend<T: AuthBackend + 'static>(&mut self, auth_backend: T) -> &mut Self {
        self.auth_backend = Some(AuthBackendWrapper::new(auth_backend));
        self
    }

    /// Add a router to the request builder.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::Request;
    /// use cot::response::Response;
    /// use cot::router::{Route, Router};
    /// use cot::test::TestRequestBuilder;
    ///
    /// async fn index(request: Request) -> cot::Result<Response> {
    ///     unimplemented!()
    /// }
    ///
    /// let router = Router::with_urls([Route::with_handler_and_name("/", index, "index")]);
    /// let request = TestRequestBuilder::get("/").router(router).build();
    /// ```
    pub fn router(&mut self, router: Router) -> &mut Self {
        self.router = Some(router);
        self
    }

    /// Add a session support to the request builder.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::test::TestRequestBuilder;
    ///
    /// let request = TestRequestBuilder::get("/").with_session().build();
    /// ```
    pub fn with_session(&mut self) -> &mut Self {
        let session_store = MemoryStore::default();
        let session_inner = tower_sessions::Session::new(None, Arc::new(session_store), None);
        self.session = Some(Session::new(session_inner));
        self
    }

    /// Add a session support to the request builder with the session copied
    /// over from another [`Request`] object.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::RequestExt;
    /// use cot::session::Session;
    /// use cot::test::TestRequestBuilder;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let mut request = TestRequestBuilder::get("/").with_session().build();
    /// Session::from_request(&request)
    ///     .insert("key", "value")
    ///     .await?;
    ///
    /// let mut request = TestRequestBuilder::get("/")
    ///     .with_session_from(&request)
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_session_from(&mut self, request: &Request) -> &mut Self {
        self.session = Some(Session::from_request(request).clone());
        self
    }

    /// Add a session support to the request builder with the session object
    /// provided as a parameter.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::RequestExt;
    /// use cot::session::Session;
    /// use cot::test::TestRequestBuilder;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let mut request = TestRequestBuilder::get("/").with_session().build();
    /// let session = Session::from_request(&request);
    /// session.insert("key", "value").await?;
    ///
    /// let mut request = TestRequestBuilder::get("/")
    ///     .session(session.clone())
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    pub fn session(&mut self, session: Session) -> &mut Self {
        self.session = Some(session);
        self
    }

    /// Add a database to the request builder.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::RequestHandler;
    /// use cot::db::Database;
    /// use cot::html::Html;
    /// use cot::test::TestRequestBuilder;
    ///
    /// async fn index(db: Database) -> Html {
    ///     // ... do something with db
    ///
    ///     Html::new("Hello world!")
    /// }
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let request = TestRequestBuilder::get("/")
    ///     .database(Database::new("sqlite::memory:").await?)
    ///     .build();
    ///
    /// assert_eq!(
    ///     index
    ///         .handle(request)
    ///         .await?
    ///         .into_body()
    ///         .into_bytes()
    ///         .await?,
    ///     "Hello world!"
    /// );
    /// # Ok(())
    /// }
    /// ```
    #[cfg(feature = "db")]
    pub fn database<DB: Into<Database>>(&mut self, database: DB) -> &mut Self {
        self.database = Some(database.into());
        self
    }

    /// Use database authentication in the test request.
    ///
    /// Note that this calls [`Self::auth_backend`], [`Self::with_session`],
    /// [`Self::database`], possibly overriding any values set by you earlier.
    ///
    /// # Panics
    ///
    /// Panics if the auth object fails to be created.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::config::ProjectConfig;
    /// use cot::test::{TestDatabase, TestRequestBuilder};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let mut test_database = TestDatabase::new_sqlite().await?;
    /// test_database.with_auth().run_migrations().await;
    /// let request = TestRequestBuilder::get("/")
    ///     .with_db_auth(test_database.database())
    ///     .await
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "db")]
    pub async fn with_db_auth(&mut self, db: Database) -> &mut Self {
        self.auth_backend(DatabaseUserBackend::new(db.clone()));
        self.with_session();
        self.database(db);
        self.auth = Some(
            Auth::new(
                self.session.clone().expect("Session was just set"),
                self.auth_backend
                    .clone()
                    .expect("Auth backend was just set")
                    .inner,
                crate::config::SecretKey::from("000000"),
                &[],
            )
            .await
            .expect("Failed to create Auth"),
        );

        self
    }

    /// Add form data to the request builder.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::test::TestRequestBuilder;
    ///
    /// let request = TestRequestBuilder::post("/")
    ///     .form_data(&[("name", "Alice"), ("age", "30")])
    ///     .build();
    /// ```
    pub fn form_data<T: ToString>(&mut self, form_data: &[(T, T)]) -> &mut Self {
        self.form_data = Some(
            form_data
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        );
        self
    }

    /// Add JSON data to the request builder.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::test::TestRequestBuilder;
    ///
    /// #[derive(serde::Serialize)]
    /// struct Data {
    ///     key: String,
    ///     value: i32,
    /// }
    ///
    /// let request = TestRequestBuilder::post("/")
    ///     .json(&Data {
    ///         key: "value".to_string(),
    ///         value: 42,
    ///     })
    ///     .build();
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the JSON serialization fails.
    #[cfg(feature = "json")]
    pub fn json<T: serde::Serialize>(&mut self, data: &T) -> &mut Self {
        self.json_data = Some(serde_json::to_string(data).expect("Failed to serialize JSON"));
        self
    }

    /// Add a static file to the request builder.
    ///
    /// This allows you to add static files that will be available in the
    /// request through the
    /// [`StaticFiles`](crate::request::extractors::StaticFiles) extractor.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::test::TestRequestBuilder;
    ///
    /// let request = TestRequestBuilder::get("/")
    ///     .static_file("css/style.css", "body { color: red; }")
    ///     .build();
    /// ```
    pub fn static_file<Path, Content>(&mut self, path: Path, content: Content) -> &mut Self
    where
        Path: Into<String>,
        Content: Into<bytes::Bytes>,
    {
        self.static_files.push(StaticFile::new(path, content));
        self
    }

    /// Build the request.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::RequestHandler;
    /// use cot::html::Html;
    /// use cot::test::TestRequestBuilder;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// async fn index() -> Html {
    ///     Html::new("Hello world!")
    /// }
    ///
    /// let request = TestRequestBuilder::get("/").build();
    ///
    /// assert_eq!(
    ///     index
    ///         .handle(request)
    ///         .await?
    ///         .into_body()
    ///         .into_bytes()
    ///         .await?,
    ///     "Hello world!"
    /// );
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn build(&mut self) -> http::Request<Body> {
        let Ok(mut request) = http::Request::builder()
            .method(self.method.clone())
            .uri(self.url.clone())
            .body(Body::empty())
        else {
            unreachable!("Test request should be valid");
        };

        let auth_backend = std::mem::take(&mut self.auth_backend);
        #[expect(trivial_casts)]
        let auth_backend = match auth_backend {
            Some(auth_backend) => Arc::new(auth_backend) as Arc<dyn AuthBackend>,
            None => Arc::new(NoAuthBackend),
        };

        let context = ProjectContext::initialized(
            self.config.clone().unwrap_or_default(),
            Vec::new(),
            Arc::new(self.router.clone().unwrap_or_else(Router::empty)),
            auth_backend,
            #[cfg(feature = "db")]
            self.database.clone(),
            #[cfg(feature = "cache")]
            self.cache
                .clone()
                .unwrap_or_else(|| Cache::new(Memory::new(), None, Timeout::default())),
        );
        prepare_request(&mut request, Arc::new(context));

        if let Some(session) = &self.session {
            request.extensions_mut().insert(session.clone());
        }

        if let Some(auth) = &self.auth {
            request.extensions_mut().insert(auth.clone());
        }

        if let Some(form_data) = &self.form_data {
            if self.method != http::Method::POST {
                todo!("Form data can currently only be used with POST requests");
            }

            let mut data = form_urlencoded::Serializer::new(String::new());
            for (key, value) in form_data {
                data.append_pair(key, value);
            }

            *request.body_mut() = Body::fixed(data.finish());
            request.headers_mut().insert(
                http::header::CONTENT_TYPE,
                http::HeaderValue::from_static("application/x-www-form-urlencoded"),
            );
        }

        #[cfg(feature = "json")]
        if let Some(json_data) = &self.json_data {
            *request.body_mut() = Body::fixed(json_data.clone());
            request.headers_mut().insert(
                http::header::CONTENT_TYPE,
                http::HeaderValue::from_static("application/json"),
            );
        }

        if !self.static_files.is_empty() {
            let config = self.config.clone().unwrap_or_default();
            let mut static_files = StaticFiles::new(&config.static_files);
            for file in std::mem::take(&mut self.static_files) {
                static_files.add_file(file);
            }
            request.extensions_mut().insert(Arc::new(static_files));
        }

        request
    }
}

/// A test database.
///
/// This is used to create a separate database for testing and run migrations on
/// it.
///
/// # Examples
///
/// ```
/// use cot::test::{TestDatabase, TestRequestBuilder};
///
/// # #[tokio::main]
/// # async fn main() -> cot::Result<()> {
/// let mut test_database = TestDatabase::new_sqlite().await?;
/// let request = TestRequestBuilder::get("/")
///     .database(test_database.database())
///     .build();
///
/// // do something with the request
///
/// test_database.cleanup().await?;
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "db")]
#[derive(Debug)]
pub struct TestDatabase {
    database: Database,
    kind: TestDatabaseKind,
    migrations: Vec<MigrationWrapper>,
}

#[cfg(feature = "db")]
impl TestDatabase {
    fn new(database: Database, kind: TestDatabaseKind) -> TestDatabase {
        Self {
            database,
            kind,
            migrations: Vec::new(),
        }
    }

    /// Create a new in-memory SQLite database for testing.
    ///
    /// # Errors
    ///
    /// If the database could not have been created.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::test::{TestDatabase, TestRequestBuilder};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let mut test_database = TestDatabase::new_sqlite().await?;
    /// let request = TestRequestBuilder::get("/")
    ///     .database(test_database.database())
    ///     .build();
    ///
    /// // do something with the request
    ///
    /// test_database.cleanup().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new_sqlite() -> Result<Self> {
        let database = Database::new("sqlite::memory:").await?;
        Ok(Self::new(database, TestDatabaseKind::Sqlite))
    }

    /// Create a new PostgreSQL database for testing and connects to it.
    ///
    /// The database URL is read from the `POSTGRES_URL` environment variable.
    /// Note that it shouldn't include the database name — the function will
    /// create a new database for the test by connecting to the `postgres`
    /// database. If no URL is provided, it defaults to
    /// `postgresql://cot:cot@localhost`.
    ///
    /// The database is created with the name `test_cot__{test_name}`.
    /// Make sure that `test_name` is unique for each test so that the databases
    /// don't conflict with each other.
    ///
    /// The database is dropped when `self.cleanup()` is called. Note that this
    /// means that the database will not be dropped if the test panics.
    ///
    /// # Errors
    ///
    /// Returns an error if a database connection (either to the test database,
    /// or postgres maintenance database) could not be established.
    ///
    /// Returns an error if the old test database could not be dropped.
    ///
    /// Returns an error if the new test database could not be created.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cot::test::{TestDatabase, TestRequestBuilder};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let mut test_database = TestDatabase::new_postgres("my_test").await?;
    /// let request = TestRequestBuilder::get("/")
    ///     .database(test_database.database())
    ///     .build();
    ///
    /// // do something with the request
    ///
    /// test_database.cleanup().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new_postgres(test_name: &str) -> Result<Self> {
        let db_url = std::env::var("POSTGRES_URL")
            .unwrap_or_else(|_| "postgresql://cot:cot@localhost".to_string());
        let database = Database::new(format!("{db_url}/postgres")).await?;

        let test_database_name = format!("test_cot__{test_name}");
        database
            .raw(&format!("DROP DATABASE IF EXISTS {test_database_name}"))
            .await?;
        database
            .raw(&format!("CREATE DATABASE {test_database_name}"))
            .await?;
        database.close().await?;

        let database = Database::new(format!("{db_url}/{test_database_name}")).await?;

        Ok(Self::new(
            database,
            TestDatabaseKind::Postgres {
                db_url,
                db_name: test_database_name,
            },
        ))
    }

    /// Create a new MySQL database for testing and connects to it.
    ///
    /// The database URL is read from the `MYSQL_URL` environment variable.
    /// Note that it shouldn't include the database name — the function will
    /// create a new database for the test by connecting to the `mysql`
    /// database. If no URL is provided, it defaults to
    /// `mysql://root:@localhost`.
    ///
    /// The database is created with the name `test_cot__{test_name}`.
    /// Make sure that `test_name` is unique for each test so that the databases
    /// don't conflict with each other.
    ///
    /// The database is dropped when `self.cleanup()` is called. Note that this
    /// means that the database will not be dropped if the test panics.
    ///
    ///
    /// # Errors
    ///
    /// Returns an error if a database connection (either to the test database,
    /// or MySQL maintenance database) could not be established.
    ///
    /// Returns an error if the old test database could not be dropped.
    ///
    /// Returns an error if the new test database could not be created.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cot::test::{TestDatabase, TestRequestBuilder};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let mut test_database = TestDatabase::new_mysql("my_test").await?;
    /// let request = TestRequestBuilder::get("/")
    ///     .database(test_database.database())
    ///     .build();
    ///
    /// // do something with the request
    ///
    /// test_database.cleanup().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new_mysql(test_name: &str) -> Result<Self> {
        let db_url =
            std::env::var("MYSQL_URL").unwrap_or_else(|_| "mysql://root:@localhost".to_string());
        let database = Database::new(format!("{db_url}/mysql")).await?;

        let test_database_name = format!("test_cot__{test_name}");
        database
            .raw(&format!("DROP DATABASE IF EXISTS {test_database_name}"))
            .await?;
        database
            .raw(&format!("CREATE DATABASE {test_database_name}"))
            .await?;
        database.close().await?;

        let database = Database::new(format!("{db_url}/{test_database_name}")).await?;

        Ok(Self::new(
            database,
            TestDatabaseKind::MySql {
                db_url,
                db_name: test_database_name,
            },
        ))
    }

    /// Add the default Cot authentication migrations to the test database.
    ///
    /// This is useful if you want to test something that requires
    /// authentication.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::test::{TestDatabase, TestRequestBuilder};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let mut test_database = TestDatabase::new_sqlite().await?;
    /// test_database.with_auth().run_migrations().await;
    ///
    /// let request = TestRequestBuilder::get("/")
    ///     .with_db_auth(test_database.database())
    ///     .await
    ///     .build();
    ///
    /// // do something with the request
    ///
    /// test_database.cleanup().await?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "db")]
    pub fn with_auth(&mut self) -> &mut Self {
        self.add_migrations(cot::auth::db::migrations::MIGRATIONS.to_vec());
        self
    }

    /// Add migrations to the test database.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::test::{TestDatabase, TestMigration};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let mut test_database = TestDatabase::new_sqlite().await?;
    ///
    /// test_database.add_migrations(vec![TestMigration::new(
    ///     "auth",
    ///     "create_users",
    ///     vec![],
    ///     vec![],
    /// )]);
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_migrations<T: DynMigration + Send + Sync + 'static, V: IntoIterator<Item = T>>(
        &mut self,
        migrations: V,
    ) -> &mut Self {
        self.migrations
            .extend(migrations.into_iter().map(MigrationWrapper::new));
        self
    }

    /// Run the migrations on the test database.
    ///
    /// # Panics
    ///
    /// Panics if the migration engine could not be initialized or if the
    /// migrations could not be run.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::test::{TestDatabase, TestMigration};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let mut test_database = TestDatabase::new_sqlite().await?;
    /// test_database.add_migrations(vec![TestMigration::new(
    ///     "auth",
    ///     "create_users",
    ///     vec![],
    ///     vec![],
    /// )]);
    ///
    /// test_database.run_migrations().await;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn run_migrations(&mut self) -> &mut Self {
        if !self.migrations.is_empty() {
            let engine = MigrationEngine::new(std::mem::take(&mut self.migrations))
                .expect("Failed to initialize the migration engine");
            engine
                .run(&self.database())
                .await
                .expect("Failed to run migrations");
        }
        self
    }

    /// Get the database.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::test::{TestDatabase, TestRequestBuilder};
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let database = TestDatabase::new_sqlite().await?;
    ///
    /// let request = TestRequestBuilder::get("/")
    ///     .database(database.database())
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn database(&self) -> Database {
        self.database.clone()
    }

    /// Cleanup the test database.
    ///
    /// This removes the test database and closes the connection. Note that this
    /// means that the database will not be dropped if the test panics, nor will
    /// it be dropped if you don't call this function.
    ///
    /// # Errors
    ///
    /// Returns an error if the database could not be closed or if the database
    /// could not be dropped.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::test::TestDatabase;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let mut test_database = TestDatabase::new_sqlite().await?;
    /// test_database.cleanup().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn cleanup(&self) -> Result<()> {
        self.database.close().await?;
        match &self.kind {
            TestDatabaseKind::Sqlite => {}
            TestDatabaseKind::Postgres { db_url, db_name } => {
                let database = Database::new(format!("{db_url}/postgres")).await?;

                database
                    .raw(&format!("DROP DATABASE {db_name} WITH (FORCE)"))
                    .await?;
                database.close().await?;
            }
            TestDatabaseKind::MySql { db_url, db_name } => {
                let database = Database::new(format!("{db_url}/mysql")).await?;

                database.raw(&format!("DROP DATABASE {db_name}")).await?;
                database.close().await?;
            }
        }

        Ok(())
    }
}

#[cfg(feature = "db")]
impl std::ops::Deref for TestDatabase {
    type Target = Database;

    fn deref(&self) -> &Self::Target {
        &self.database
    }
}

#[cfg(feature = "db")]
#[derive(Debug, Clone)]
enum TestDatabaseKind {
    Sqlite,
    Postgres { db_url: String, db_name: String },
    MySql { db_url: String, db_name: String },
}

/// A test migration.
///
/// This can be used if you need a dynamically created migration for testing.
///
/// # Examples
///
/// ```
/// use cot::db::migrations::{Field, Operation};
/// use cot::db::{ColumnType, Identifier};
/// use cot::test::{TestDatabase, TestMigration};
///
/// const OPERATION: Operation = Operation::create_model()
///     .table_name(Identifier::new("myapp__users"))
///     .fields(&[Field::new(Identifier::new("id"), ColumnType::Integer)
///         .auto()
///         .primary_key()])
///     .build();
///
/// let migration = TestMigration::new("auth", "create_users", vec![], vec![OPERATION]);
/// ```
#[cfg(feature = "db")]
#[derive(Debug, Clone)]
pub struct TestMigration {
    app_name: &'static str,
    name: &'static str,
    dependencies: Vec<MigrationDependency>,
    operations: Vec<Operation>,
}

#[cfg(feature = "db")]
impl TestMigration {
    /// Create a new test migration.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::db::migrations::{Field, Operation};
    /// use cot::db::{ColumnType, Identifier};
    /// use cot::test::{TestDatabase, TestMigration};
    ///
    /// const OPERATION: Operation = Operation::create_model()
    ///     .table_name(Identifier::new("myapp__users"))
    ///     .fields(&[Field::new(Identifier::new("id"), ColumnType::Integer)
    ///         .auto()
    ///         .primary_key()])
    ///     .build();
    ///
    /// let migration = TestMigration::new("auth", "create_users", vec![], vec![OPERATION]);
    /// ```
    #[must_use]
    pub fn new<D: Into<Vec<MigrationDependency>>, O: Into<Vec<Operation>>>(
        app_name: &'static str,
        name: &'static str,
        dependencies: D,
        operations: O,
    ) -> Self {
        Self {
            app_name,
            name,
            dependencies: dependencies.into(),
            operations: operations.into(),
        }
    }
}

#[cfg(feature = "db")]
impl DynMigration for TestMigration {
    fn app_name(&self) -> &str {
        self.app_name
    }

    fn name(&self) -> &str {
        self.name
    }

    fn dependencies(&self) -> &[MigrationDependency] {
        &self.dependencies
    }

    fn operations(&self) -> &[Operation] {
        &self.operations
    }
}

/// A utility for running entire projects in end-to-end tests.
///
/// This is useful for testing the full stack of a project, including the
/// database, the router, the auth, etc. The server is running in the same
/// process as the test by running a background async task.
///
///  This can be used to test the entire project by sending real requests to the
/// server, possibly using libraries such as
/// - [`reqwest`](https://docs.rs/reqwest/latest/reqwest/) for HTTP requests
/// - [`thirtyfour`](https://docs.rs/thirtyfour/latest/thirtyfour/) or [`fantoccini`](https://docs.rs/fantoccini/latest/fantoccini/)
///   for browser automation
///
/// Note that you need to use [`cot::e2e_test`] to run this, not
/// [`macro@cot::test`]. Remember to call [`TestServer::close`] when
/// you're done with the tests, as the server will not be stopped automatically.
///
/// # Examples
///
/// ```
/// use cot::test::TestServerBuilder;
///
/// struct TestProject;
/// impl cot::Project for TestProject {}
///
/// #[cot::e2e_test] // note this uses "e2e_test"!
/// async fn test_server() -> cot::Result<()> {
///     let server = TestServerBuilder::new(TestProject).start().await;
///
///     let url = server.url();
///     // ...use the server URL to send requests to the server
///
///     server.close().await;
///     Ok(())
/// }
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TestServerBuilder<T> {
    project: T,
}

impl<T: Project + 'static> TestServerBuilder<T> {
    /// Create a new test server.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::test::TestServerBuilder;
    ///
    /// struct TestProject;
    /// impl cot::Project for TestProject {}
    ///
    /// #[cot::e2e_test] // note this uses "e2e_test"!
    /// async fn test_server() -> cot::Result<()> {
    ///     let server = TestServerBuilder::new(TestProject).start().await;
    ///
    ///     let url = server.url();
    ///     // ...use the server URL to send requests to the server
    ///
    ///     server.close().await;
    ///     Ok(())
    /// }
    /// ```
    #[must_use]
    pub fn new(project: T) -> Self {
        Self { project }
    }

    /// Start the test server.
    ///
    /// # Panics
    ///
    /// This function will panic if it fails to bind to a port.
    ///
    /// This function will panic if the server could not be started.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::test::TestServerBuilder;
    ///
    /// struct TestProject;
    /// impl cot::Project for TestProject {}
    ///
    /// #[cot::e2e_test] // note this uses "e2e_test"!
    /// async fn test_server() -> cot::Result<()> {
    ///     let server = TestServerBuilder::new(TestProject).start().await;
    ///
    ///     let url = server.url();
    ///     // ...use the server URL to send requests to the server
    ///
    ///     server.close().await;
    ///     Ok(())
    /// }
    /// ```
    pub async fn start(self) -> TestServer<T> {
        TestServer::start(self.project).await
    }
}

/// A running test server.
///
/// This is returned by [`TestServerBuilder::start`] and can be used to access
/// the server's URL and close the server.
///
/// # Examples
///
/// ```
/// use cot::test::TestServerBuilder;
///
/// struct TestProject;
/// impl cot::Project for TestProject {}
///
/// #[cot::e2e_test] // note this uses "e2e_test"!
/// async fn test_server() -> cot::Result<()> {
///     let server = TestServerBuilder::new(TestProject).start().await;
///
///     let url = server.url();
///     // ...use the server URL to send requests to the server
///
///     server.close().await;
///     Ok(())
/// }
/// ```
#[must_use = "TestServer must be used to close the server"]
#[derive(Debug)]
pub struct TestServer<T> {
    address: SocketAddr,
    channel_send: oneshot::Sender<()>,
    server_handle: tokio::task::JoinHandle<()>,
    project: PhantomData<fn() -> T>,
}

impl<T: Project + 'static> TestServer<T> {
    async fn start(project: T) -> Self {
        let tcp_listener = TcpListener::bind("0.0.0.0:0")
            .await
            .expect("Failed to bind to a port");
        let mut address = tcp_listener
            .local_addr()
            .expect("Failed to get the listening address");
        address.set_ip(IpAddr::V4(Ipv4Addr::LOCALHOST));

        let (send, recv) = oneshot::channel::<()>();

        let server_handle = tokio::task::spawn_local(async move {
            let bootstrapper = Bootstrapper::new(project)
                .with_config_name("test")
                .expect("Failed to get the \"test\" config")
                .boot()
                .await
                .expect("Failed to boot the project");
            run_at_with_shutdown(bootstrapper, tcp_listener, async move {
                recv.await.expect("Failed to receive a shutdown signal");
            })
            .await
            .expect("Failed to run the server");
        });

        Self {
            address,
            channel_send: send,
            server_handle,
            project: PhantomData,
        }
    }

    /// Get the server's address.
    ///
    /// You can use this to get the port that the server is running on. It's,
    /// however, typically more convenient to use [`Self::url`] to get
    /// the server's URL.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::test::TestServerBuilder;
    ///
    /// struct TestProject;
    /// impl cot::Project for TestProject {}
    ///
    /// #[cot::e2e_test] // note this uses "e2e_test"!
    /// async fn test_server() -> cot::Result<()> {
    ///     let server = TestServerBuilder::new(TestProject).start().await;
    ///
    ///     let address = server.address();
    ///     // ...use the server address to send requests to the server
    ///
    ///     server.close().await;
    ///     Ok(())
    /// }
    /// ```
    #[must_use]
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Get the server's URL.
    ///
    /// This is the URL of the server that can be used to send requests to the
    /// server. Note that this will typically return the local address of the
    /// server (127.0.0.1) and not the public address of the machine. This might
    /// be a problem if you are making requests from a different machine or a
    /// Docker container. If you need to override the host returned by this
    /// function, you can set the `COT_TEST_SERVER_HOST` environment variable.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::test::TestServerBuilder;
    ///
    /// struct TestProject;
    /// impl cot::Project for TestProject {}
    ///
    /// #[cot::e2e_test] // note this uses "e2e_test"!
    /// async fn test_server() -> cot::Result<()> {
    ///     let server = TestServerBuilder::new(TestProject).start().await;
    ///
    ///     let url = server.url();
    ///     // ...use the server URL to send requests to the server
    ///
    ///     server.close().await;
    ///     Ok(())
    /// }
    /// ```
    #[must_use]
    pub fn url(&self) -> String {
        if let Ok(host) = std::env::var("COT_TEST_SERVER_HOST") {
            format!("http://{}:{}", host, self.address.port())
        } else {
            format!("http://{}", self.address)
        }
    }

    /// Stop the server.
    ///
    /// Note that this is not automatically called when the `TestServer` is
    /// dropped; you need to call this function explicitly.
    ///
    /// # Panics
    ///
    /// This function will panic if an error occurs while sending the shutdown
    /// signal or if the server task panics.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::test::TestServerBuilder;
    ///
    /// struct TestProject;
    /// impl cot::Project for TestProject {}
    ///
    /// #[cot::e2e_test] // note this uses "e2e_test"!
    /// async fn test_server() -> cot::Result<()> {
    ///     let server = TestServerBuilder::new(TestProject).start().await;
    ///
    ///     server.close().await;
    ///     Ok(())
    /// }
    /// ```
    pub async fn close(self) {
        self.channel_send
            .send(())
            .expect("Failed to send a shutdown signal");
        self.server_handle
            .await
            .expect("Failed to join the server task");
    }
}

/// A guard for running tests serially.
///
/// This is mostly useful for tests that need to modify some global state (e.g.
/// environment variables or current working directory).
#[doc(hidden)] // not part of the public API; used in cot-cli
pub fn serial_guard() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    let lock = LOCK.get_or_init(|| std::sync::Mutex::new(()));
    match lock.lock() {
        Ok(guard) => guard,
        Err(poison_error) => {
            lock.clear_poison();
            // We can ignore poisoned mutexes because we don't store any data inside
            poison_error.into_inner()
        }
    }
}

#[cfg(feature = "redis")]
const POOL_KEY: &str = "cot:test:db_pool";

#[cfg(feature = "redis")]
async fn get_db_num(conn: &mut Connection) -> usize {
    let cfg = redis::cmd("CONFIG")
        .arg("GET")
        .arg("databases")
        .query_async::<Vec<String>>(conn)
        .await
        .expect("Failed to get Redis config");
    cfg.get(1)
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(16)
}

#[cfg(feature = "redis")]
async fn set_current_db(conn: &mut Connection, db_num: usize) {
    redis::cmd("SELECT")
        .arg(db_num)
        .query_async::<()>(conn)
        .await
        .expect("Failed to select Redis database");
}

#[cfg(feature = "redis")]
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
enum RedisDbAllocatorError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("Redis error: {0}")]
    Redis(String),
}

#[cfg(feature = "redis")]
impl_into_cot_error!(RedisDbAllocatorError);

#[cfg(feature = "redis")]
#[derive(Debug, Clone)]
struct RedisDbAllocator {
    alloc_db: usize,
    redis: Redis,
}

#[cfg(feature = "redis")]
type RedisAllocatorResult<T> = std::result::Result<T, RedisDbAllocatorError>;
#[cfg(feature = "redis")]
impl RedisDbAllocator {
    fn new(alloc_db: usize, redis: Redis) -> Self {
        Self { alloc_db, redis }
    }

    async fn get_conn(&self) -> RedisAllocatorResult<Connection> {
        let conn = self
            .redis
            .get_connection()
            .await
            .map_err(|err| RedisDbAllocatorError::Redis(err.to_string()))?;
        Ok(conn)
    }

    /// Initialize the Redis database allocator.
    ///
    /// The goal here is to ensure that DB IDs are initialized once.
    /// Since we run tests using `nextest`, the tests are run per process.
    /// Thus, we run this in a transaction to guarantee a deterministic
    /// behavior.
    ///
    /// On initializing the IDs, we check for the existence of an "init" key in
    /// the DB. If the key does not exist, or if the length of the pool list
    /// does not match the expected count, we  reinitialize the pool by
    /// populating it with database indices from 1 to `alloc_db - 1`.
    async fn init(&self) -> RedisAllocatorResult<Option<String>> {
        const KEY_TIMEOUT_SECS: u64 = 300;
        const INIT_KEY: &str = "cot:test:db_pool:initialized";

        let mut con = self.get_conn().await?;
        let last_eligible_db = self.alloc_db - 1;

        redis::cmd("WATCH")
            .arg(INIT_KEY)
            .query_async::<redis::Value>(&mut con)
            .await
            .map_err(|err| RedisDbAllocatorError::Redis(err.to_string()))?;

        let prev = redis::cmd("GET")
            .arg(INIT_KEY)
            .query_async::<Option<String>>(&mut con)
            .await
            .map_err(|err| RedisDbAllocatorError::Redis(err.to_string()))?;

        if prev.is_some() {
            redis::cmd("UNWATCH")
                .query_async::<redis::Value>(&mut con)
                .await
                .map_err(|err| RedisDbAllocatorError::Redis(err.to_string()))?;
            return Ok(prev);
        }

        // start a transaction so this is atomic across processes
        redis::cmd("MULTI")
            .query_async::<redis::Value>(&mut con)
            .await
            .map_err(|err| RedisDbAllocatorError::Redis(err.to_string()))?;

        let mut set_cmd = redis::cmd("SET");
        set_cmd.arg(INIT_KEY).arg("1");
        set_cmd.arg("EX").arg(KEY_TIMEOUT_SECS);
        set_cmd
            .query_async::<redis::Value>(&mut con)
            .await
            .map_err(|err| RedisDbAllocatorError::Redis(err.to_string()))?;

        // delete and reinit IDs
        redis::cmd("DEL")
            .arg(POOL_KEY)
            .query_async::<redis::Value>(&mut con)
            .await
            .map_err(|err| RedisDbAllocatorError::Redis(err.to_string()))?;

        let vals: Vec<String> = (1..=last_eligible_db).map(|i| i.to_string()).collect();
        redis::cmd("RPUSH")
            .arg(POOL_KEY)
            .arg(vals)
            .query_async::<redis::Value>(&mut con)
            .await
            .map_err(|err| RedisDbAllocatorError::Redis(err.to_string()))?;

        // keys should expire after a short while, a double defense against reuse by
        // subsequent runs
        redis::cmd("EXPIRE")
            .arg(POOL_KEY)
            .arg(KEY_TIMEOUT_SECS)
            .query_async::<redis::Value>(&mut con)
            .await
            .map_err(|err| RedisDbAllocatorError::Redis(err.to_string()))?;

        redis::cmd("EXEC")
            .query_async::<Option<Vec<redis::Value>>>(&mut con)
            .await
            .map_err(|err| RedisDbAllocatorError::Redis(err.to_string()))?;
        Ok(None)
    }

    async fn allocate(&self) -> RedisAllocatorResult<Option<usize>> {
        let mut connection = self.get_conn().await?;

        let db_index: Option<String> = connection
            .lpop(POOL_KEY, None)
            .await
            .map_err(|err| RedisDbAllocatorError::Redis(err.to_string()))?;
        Ok(db_index.and_then(|i| i.parse::<usize>().ok()))
    }
}

#[cfg(feature = "cache")]
#[derive(Debug, Clone)]
enum CacheKind {
    Memory,
    #[cfg(feature = "redis")]
    Redis {
        #[expect(unused)]
        allocator: RedisDbAllocator,
    },
}

/// A test cache.
///
/// This is used to create a separate cache for testing.
///
/// # Examples
///
/// ```
/// use cot::test::TestCache;
///
/// # #[tokio::main]
/// # async fn main() -> cot::Result<()> {
/// let test_cache = TestCache::new_memory();
/// let cache = test_cache.cache();
///
/// // do something with the cache
///
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "cache")]
#[derive(Debug, Clone)]
pub struct TestCache {
    cache: Cache,
    kind: CacheKind,
}

#[cfg(feature = "cache")]
impl TestCache {
    fn new(cache: Cache, kind: CacheKind) -> Self {
        Self { cache, kind }
    }

    /// Create a new in-memory test cache.
    ///
    /// # Errors
    ///
    /// Returns an error if the cache could not be created.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::test::TestCache;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let test_cache = TestCache::new_memory();
    /// let cache = test_cache.cache();
    ///
    /// // do something with the cache
    ///
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn new_memory() -> Self {
        let cache = Cache::new(Memory::new(), None, Timeout::default());
        Self::new(cache, CacheKind::Memory)
    }

    /// Create a new Redis test cache.
    ///
    /// The Redis URL is read from the `REDIS_URL` environment variable. If not
    /// provided, it defaults to `redis://localhost`.
    ///
    /// Running with redis makes use of an internal allocator that selects what
    /// DB a test will run. Every test requires its own database to avoid
    /// conflicts. The allocator, by design, will reserve the last database
    /// number for allocation purposes, so make sure your Redis instance is
    /// configured with at least 2 databases. For example if your redis
    /// instance has 16 logical databases, database 15 will be used for
    /// allocations, and databases 0-14 will be used for tests.
    ///
    /// # Errors
    ///
    /// Returns an error if the Redis cache could not be created.
    ///
    /// # Panics
    ///
    /// Panics if Redis is not configured with at least 2 databases.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cot::test::TestCache;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let test_cache = TestCache::new_redis().await?;
    /// let cache = test_cache.cache();
    ///
    /// // do something with the cache
    ///
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "redis")]
    pub async fn new_redis() -> Result<Self> {
        let url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost".to_string());
        let mut url = CacheUrl::from(url);

        let redis = Redis::new(&url, crate::config::DEFAULT_REDIS_POOL_SIZE)?;
        let mut conn = redis.get_connection().await?;
        // get the total number of DBs
        let db_num = get_db_num(&mut conn).await;
        assert!(
            db_num > 1,
            "Redis must be configured with at least 2 databases for testing"
        );

        let alloc_db = db_num - 1;

        // switch to the allocation DB to perform initialization
        set_current_db(&mut conn, db_num - 1).await;

        let allocator = RedisDbAllocator::new(alloc_db, redis);
        allocator.init().await?;
        // get the db number for the current test
        let current_db = allocator
            .allocate()
            .await?
            .expect("Failed to allocate a Redis database for testing");

        // create a new connection to the correct DB
        url.inner_mut().set_path(current_db.to_string().as_str());
        let redis = Redis::new(&url, crate::config::DEFAULT_REDIS_POOL_SIZE)?;
        let cache = Cache::new(redis, Some("test_harness".to_string()), Timeout::default());

        let this = Self::new(cache, CacheKind::Redis { allocator });

        Ok(this)
    }

    /// Get the cache.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::test::TestCache;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let test_cache = TestCache::new_memory();
    /// let cache = test_cache.cache();
    ///
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn cache(&self) -> Cache {
        self.cache.clone()
    }

    /// Cleanup the test cache.
    ///
    /// This will clear the cache and deallocate any resources used by the test
    /// cache.
    ///
    /// # Errors
    ///
    /// Returns an error if the cache could not be cleared.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use cot::test::TestCache;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let test_cache = TestCache::new_redis().await?;
    ///
    /// // do something with the cache
    ///
    /// test_cache.cleanup().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn cleanup(&self) -> Result<()> {
        #[cfg(feature = "redis")]
        if let CacheKind::Redis { allocator: _ } = &self.kind {
            self.cache.clear().await?;
        }
        Ok(())
    }
}
