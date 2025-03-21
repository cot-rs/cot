//! Administration panel.
//!
//! This module provides an administration panel for managing models
//! registered in the application, straight from the web interface.

use std::any::Any;
use std::collections::HashMap;
use std::marker::PhantomData;

use async_trait::async_trait;
use bytes::Bytes;
use cot::Error;
/// Implements the [`AdminModel`] trait for a struct.
///
/// This is a simple method for adding a database model to the admin panel.
/// Note that in order for this derive macro to work, the structure
/// **must** implement [`Model`](crate::db::Model) and
/// [`Form`] traits. These can also be derived using the `#[model]` and
/// `#[derive(Form)]` attributes.
pub use cot_macros::AdminModel;
use derive_more::Debug;
use rinja::Template;

use crate::auth::{AuthRequestExt, Password};
use crate::form::{
    Form, FormContext, FormErrorTarget, FormField, FormFieldValidationError, FormResult,
};
use crate::request::{Request, RequestExt, query_pairs};
use crate::response::{Response, ResponseExt};
use crate::router::Router;
use crate::{App, Body, Method, RequestHandler, StatusCode, reverse_redirect, static_files};

struct AdminAuthenticated<T: Send + Sync>(T);

impl<T: RequestHandler + Send + Sync> AdminAuthenticated<T> {
    #[must_use]
    fn new(handler: T) -> Self {
        Self(handler)
    }
}

#[async_trait]
impl<T: RequestHandler + Send + Sync> RequestHandler for AdminAuthenticated<T> {
    async fn handle(&self, mut request: Request) -> crate::Result<Response> {
        if !request.user().await?.is_authenticated() {
            return Ok(reverse_redirect!(request, "login")?);
        }

        self.0.handle(request).await
    }
}

async fn index(request: Request) -> crate::Result<Response> {
    #[derive(Debug, Template)]
    #[template(path = "admin/model_list.html")]
    struct ModelListTemplate<'a> {
        request: &'a Request,
        #[debug("..")]
        model_managers: Vec<Box<dyn AdminModelManager>>,
    }

    let template = ModelListTemplate {
        request: &request,
        model_managers: admin_model_managers(&request),
    };
    Ok(Response::new_html(
        StatusCode::OK,
        Body::fixed(template.render()?),
    ))
}

#[derive(Debug, Form)]
struct LoginForm {
    username: String,
    password: Password,
}

async fn login(mut request: Request) -> cot::Result<Response> {
    #[derive(Debug, Template)]
    #[template(path = "admin/login.html")]
    struct LoginTemplate<'a> {
        request: &'a Request,
        form: <LoginForm as Form>::Context,
    }

    let login_form_context = if request.method() == Method::GET {
        LoginForm::build_context(&mut request).await?
    } else if request.method() == Method::POST {
        let login_form = LoginForm::from_request(&mut request).await?;
        match login_form {
            FormResult::Ok(login_form) => {
                if authenticate(&mut request, login_form).await? {
                    return Ok(reverse_redirect!(request, "index")?);
                }

                let mut context = LoginForm::build_context(&mut request).await?;
                context.add_error(
                    FormErrorTarget::Form,
                    FormFieldValidationError::from_static("Invalid username or password"),
                );
                context
            }
            FormResult::ValidationError(context) => context,
        }
    } else {
        panic!("Unexpected request method");
    };

    let template = LoginTemplate {
        request: &request,
        form: login_form_context,
    };
    Ok(Response::new_html(
        StatusCode::OK,
        Body::fixed(template.render()?),
    ))
}

async fn authenticate(request: &mut Request, login_form: LoginForm) -> cot::Result<bool> {
    #[cfg(feature = "db")]
    let user = request
        .authenticate(&crate::auth::db::DatabaseUserCredentials::new(
            login_form.username,
            Password::new(login_form.password.into_string()),
        ))
        .await?;

    #[cfg(not(feature = "db"))]
    let user: Option<Box<dyn crate::auth::User + Send + Sync>> = None;

    if let Some(user) = user {
        request.login(user).await?;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Struct representing the pagination of objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pagination {
    limit: u64,
    offset: u64,
}

impl Pagination {
    fn new(limit: u64, page: u64) -> Self {
        assert!(page > 0, "Page number must be greater than 0");

        Self {
            limit,
            offset: (page - 1) * limit,
        }
    }

    /// Returns the limit of objects per page.
    #[must_use]
    pub fn limit(&self) -> u64 {
        self.limit
    }

    /// Returns the offset of objects.
    #[must_use]
    pub fn offset(&self) -> u64 {
        self.offset
    }
}

async fn view_model(request: Request) -> cot::Result<Response> {
    #[derive(Debug, Template)]
    #[template(path = "admin/model.html")]
    struct ModelTemplate<'a> {
        request: &'a Request,
        #[debug("..")]
        model: &'a dyn AdminModelManager,
        #[debug("..")]
        objects: Vec<Box<dyn AdminModel>>,
        page: u64,
        page_size: &'a u64,
        total_object_counts: u64,
        total_pages: u64,
    }

    let model_name: String = request.path_params().parse()?;
    let manager = get_manager(&request, &model_name)?;

    let query_params: HashMap<String, String> = request
        .uri()
        .query()
        .map(|q| {
            query_pairs(&Bytes::copy_from_slice(q.as_bytes()))
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let page: u64 = query_params
        .get("page")
        .map_or(1, |p| p.parse().unwrap_or(1));

    let limit = query_params
        .get("page_size")
        .map_or(10, |p| p.parse().unwrap_or(10));

    let total_object_counts = manager.get_total_object_counts(&request).await?;
    let total_pages = total_object_counts.div_ceil(limit);

    if page == 0 || page > total_pages {
        return Err(Error::not_found_message(format!("page {page} not found")));
    }

    let pagination = Pagination::new(limit, page);

    let objects = manager.get_objects(&request, pagination).await?;

    let template = ModelTemplate {
        request: &request,
        model: &*manager,
        objects,
        page,
        page_size: &limit,
        total_object_counts,
        total_pages,
    };

    Ok(Response::new_html(
        StatusCode::OK,
        Body::fixed(template.render()?),
    ))
}

async fn create_model_instance(request: Request) -> cot::Result<Response> {
    let model_name: String = request.path_params().parse()?;

    edit_model_instance_impl(request, &model_name, None).await
}

async fn edit_model_instance(request: Request) -> cot::Result<Response> {
    let (model_name, object_id): (String, String) = request.path_params().parse()?;

    edit_model_instance_impl(request, &model_name, Some(&object_id)).await
}

async fn edit_model_instance_impl(
    mut request: Request,
    model_name: &str,
    object_id: Option<&str>,
) -> cot::Result<Response> {
    #[derive(Debug, Template)]
    #[template(path = "admin/model_edit.html")]
    struct ModelEditTemplate<'a> {
        request: &'a Request,
        #[debug("..")]
        model: &'a dyn AdminModelManager,
        form_context: Box<dyn FormContext>,
        is_edit: bool,
    }

    let manager = get_manager(&request, model_name)?;

    let form_context = if request.method() == Method::POST {
        let form_context = manager.save_from_request(&mut request, object_id).await?;

        if let Some(form_context) = form_context {
            form_context
        } else {
            return Ok(reverse_redirect!(
                request,
                "view_model",
                model_name = manager.url_name()
            )?);
        }
    } else if let Some(object_id) = object_id {
        let object = get_object(&mut request, &*manager, object_id).await?;

        manager.form_context_from_object(object)
    } else {
        manager.form_context()
    };

    let template = ModelEditTemplate {
        request: &request,
        model: &*manager,
        form_context,
        is_edit: object_id.is_some(),
    };

    Ok(Response::new_html(
        StatusCode::OK,
        Body::fixed(template.render()?),
    ))
}

async fn remove_model_instance(mut request: Request) -> cot::Result<Response> {
    #[derive(Debug, Template)]
    #[template(path = "admin/model_remove.html")]
    struct ModelRemoveTemplate<'a> {
        request: &'a Request,
        #[debug("..")]
        model: &'a dyn AdminModelManager,
        #[debug("..")]
        object: &'a dyn AdminModel,
    }

    let (model_name, object_id): (String, String) = request.path_params().parse()?;

    let manager = get_manager(&request, &model_name)?;
    let object = get_object(&mut request, &*manager, &object_id).await?;

    if request.method() == Method::POST {
        manager.remove_by_id(&mut request, &object_id).await?;

        Ok(reverse_redirect!(
            request,
            "view_model",
            model_name = manager.url_name()
        )?)
    } else {
        let template = ModelRemoveTemplate {
            request: &request,
            model: &*manager,
            object: &*object,
        };

        Ok(Response::new_html(
            StatusCode::OK,
            Body::fixed(template.render()?),
        ))
    }
}

async fn get_object(
    request: &mut Request,
    manager: &dyn AdminModelManager,
    object_id: &str,
) -> Result<Box<dyn AdminModel>, Error> {
    manager
        .get_object_by_id(request, object_id)
        .await?
        .ok_or_else(|| {
            Error::not_found_message(format!(
                "Object with ID `{}` not found in model `{}`",
                object_id,
                manager.name()
            ))
        })
}

fn get_manager(request: &Request, model_name: &str) -> cot::Result<Box<dyn AdminModelManager>> {
    let model_managers = admin_model_managers(request);

    model_managers
        .into_iter()
        .find(|manager| manager.url_name() == model_name)
        .ok_or_else(|| Error::not_found_message(format!("Model `{model_name}` not found")))
}

#[must_use]
fn admin_model_managers(request: &Request) -> Vec<Box<dyn AdminModelManager>> {
    request
        .context()
        .apps()
        .iter()
        .flat_map(|app| app.admin_model_managers())
        .collect()
}

/// A trait for adding admin models to the app.
///
/// This exposes an API over [`AdminModel`] that is dyn-compatible and
/// hence can be dynamically added to the project.
///
/// See [`DefaultAdminModelManager`] for an automatic implementation of this
/// trait.
#[async_trait]
pub trait AdminModelManager: Send + Sync {
    /// Returns the display name of the model.
    fn name(&self) -> &str;

    /// Returns the URL slug for the model.
    fn url_name(&self) -> &str;

    /// Returns the list of objects of this model.
    async fn get_objects(
        &self,
        request: &Request,
        pagination: Pagination,
    ) -> cot::Result<Vec<Box<dyn AdminModel>>>;

    /// Returns the total count of objects of this model.
    async fn get_total_object_counts(&self, request: &Request) -> cot::Result<u64>;

    /// Returns the object with the given ID.
    async fn get_object_by_id(
        &self,
        request: &Request,
        id: &str,
    ) -> cot::Result<Option<Box<dyn AdminModel>>>;

    /// Returns an empty form context for this model.
    fn form_context(&self) -> Box<dyn FormContext>;

    /// Returns a form context pre-filled with the data from given object.
    ///
    /// It is guaranteed that `object` parameter is an object returned by either
    /// [`Self::get_objects`] or [`Self::get_object_by_id`] methods. This means
    /// that if you always return the same object type from these methods,
    /// you can safely downcast the object to the same type in this method
    /// as well.
    fn form_context_from_object(&self, object: Box<dyn AdminModel>) -> Box<dyn FormContext>;

    /// Saves the object by using the form data from given request.
    ///
    /// # Errors
    ///
    /// Returns an error if the object could not be saved, for instance
    /// due to a database error.
    async fn save_from_request(
        &self,
        request: &mut Request,
        object_id: Option<&str>,
    ) -> cot::Result<Option<Box<dyn FormContext>>>;

    /// Removes the object with the given ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the object with the given ID does not exist.
    ///
    /// Returns an error if the object could not be removed, for instance
    /// due to a database error.
    async fn remove_by_id(&self, request: &mut Request, object_id: &str) -> cot::Result<()>;
}

/// A default implementation of [`AdminModelManager`] for an [`AdminModel`].
#[derive(Debug)]
pub struct DefaultAdminModelManager<T> {
    phantom_data: PhantomData<T>,
}

impl<T> Default for DefaultAdminModelManager<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> DefaultAdminModelManager<T> {
    /// Creates a new instance of the default admin model manager.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            phantom_data: PhantomData,
        }
    }
}

#[async_trait]
impl<T: AdminModel + Send + Sync + 'static> AdminModelManager for DefaultAdminModelManager<T> {
    fn name(&self) -> &str {
        T::name()
    }

    fn url_name(&self) -> &str {
        T::url_name()
    }

    async fn get_total_object_counts(&self, request: &Request) -> cot::Result<u64> {
        T::get_total_object_counts(request).await
    }

    async fn get_objects(
        &self,
        request: &Request,
        pagination: Pagination,
    ) -> cot::Result<Vec<Box<dyn AdminModel>>> {
        #[allow(trivial_casts)] // Upcast to the correct Box type
        T::get_objects(request, pagination).await.map(|objects| {
            objects
                .into_iter()
                .map(|object| Box::new(object) as Box<dyn AdminModel>)
                .collect()
        })
    }

    async fn get_object_by_id(
        &self,
        request: &Request,
        id: &str,
    ) -> cot::Result<Option<Box<dyn AdminModel>>> {
        #[allow(trivial_casts)] // Upcast to the correct Box type
        T::get_object_by_id(request, id)
            .await
            .map(|object| object.map(|object| Box::new(object) as Box<dyn AdminModel>))
    }

    fn form_context(&self) -> Box<dyn FormContext> {
        T::form_context()
    }

    fn form_context_from_object(&self, object: Box<dyn AdminModel>) -> Box<dyn FormContext> {
        let object_casted = object
            .as_any()
            .downcast_ref::<T>()
            .expect("Invalid object type");

        T::form_context_from_self(object_casted)
    }

    async fn save_from_request(
        &self,
        request: &mut Request,
        object_id: Option<&str>,
    ) -> cot::Result<Option<Box<dyn FormContext>>> {
        T::save_from_request(request, object_id).await
    }

    async fn remove_by_id(&self, request: &mut Request, object_id: &str) -> cot::Result<()> {
        T::remove_by_id(request, object_id).await
    }
}

/// A model that can be managed by the admin panel.
#[async_trait]
pub trait AdminModel: Any + Send + 'static {
    /// Returns the object as an `Any` trait object.
    // TODO: consider removing this when Rust trait_upcasting is stabilized and we
    // bump the MSRV (lands in Rust 1.86)
    fn as_any(&self) -> &dyn Any;

    /// Get the objects of this model.
    async fn get_objects(request: &Request, pagination: Pagination) -> cot::Result<Vec<Self>>
    where
        Self: Sized;

    /// Get the total count of objects of this model.
    async fn get_total_object_counts(request: &Request) -> cot::Result<u64>
    where
        Self: Sized;

    /// Returns the object with the given ID.
    async fn get_object_by_id(request: &Request, id: &str) -> cot::Result<Option<Self>>
    where
        Self: Sized;

    /// Get the display name of this model.
    fn name() -> &'static str
    where
        Self: Sized;

    /// Get the URL slug for this model.
    fn url_name() -> &'static str
    where
        Self: Sized;

    /// Get the ID of this model instance as a [`String`].
    fn id(&self) -> String;

    /// Get the display text of this model instance.
    fn display(&self) -> String;

    /// Get the form context for this model.
    fn form_context() -> Box<dyn FormContext>
    where
        Self: Sized;

    /// Get the form context with the data pre-filled from this model instance.
    fn form_context_from_self(&self) -> Box<dyn FormContext>;

    /// Save the model instance from the form data in the request.
    ///
    /// # Errors
    ///
    /// Returns an error if the object could not be saved, for instance
    /// due to a database error.
    async fn save_from_request(
        request: &mut Request,
        object_id: Option<&str>,
    ) -> cot::Result<Option<Box<dyn FormContext>>>
    where
        Self: Sized;

    /// Remove the model instance with the given ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the object with the given ID does not exist.
    ///
    /// Returns an error if the object could not be removed, for instance
    /// due to a database error.
    async fn remove_by_id(request: &mut Request, object_id: &str) -> cot::Result<()>
    where
        Self: Sized;
}

/// The admin app.
///
/// # Examples
///
/// ```
/// use cot::admin::AdminApp;
/// use cot::project::WithConfig;
/// use cot::{AppBuilder, Project, ProjectContext};
///
/// struct MyProject;
/// impl Project for MyProject {
///     fn register_apps(&self, apps: &mut AppBuilder, _context: &ProjectContext<WithConfig>) {
///         apps.register_with_views(AdminApp::new(), "/admin");
///     }
/// }
/// ```
#[derive(Debug, Copy, Clone)]
pub struct AdminApp;

impl Default for AdminApp {
    fn default() -> Self {
        Self::new()
    }
}

impl AdminApp {
    /// Creates an admin app instance.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::admin::AdminApp;
    /// use cot::project::WithConfig;
    /// use cot::{AppBuilder, Project, ProjectContext};
    ///
    /// struct MyProject;
    /// impl Project for MyProject {
    ///     fn register_apps(&self, apps: &mut AppBuilder, _context: &ProjectContext<WithConfig>) {
    ///         apps.register_with_views(AdminApp::new(), "/admin");
    ///     }
    /// }
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }
}

impl App for AdminApp {
    fn name(&self) -> &'static str {
        "cot_admin"
    }

    fn router(&self) -> Router {
        Router::with_urls([
            crate::router::Route::with_handler_and_name(
                "/",
                AdminAuthenticated::new(index),
                "index",
            ),
            crate::router::Route::with_handler_and_name("/login/", login, "login"),
            crate::router::Route::with_handler_and_name(
                "/{model_name}/",
                AdminAuthenticated::new(view_model),
                "view_model",
            ),
            crate::router::Route::with_handler_and_name(
                "/{model_name}/create/",
                AdminAuthenticated::new(create_model_instance),
                "create_model_instance",
            ),
            crate::router::Route::with_handler_and_name(
                "/{model_name}/{pk}/edit/",
                AdminAuthenticated::new(edit_model_instance),
                "edit_model_instance",
            ),
            crate::router::Route::with_handler_and_name(
                "/{model_name}/{pk}/remove/",
                AdminAuthenticated::new(remove_model_instance),
                "remove_model_instance",
            ),
        ])
    }

    fn static_files(&self) -> Vec<(String, Bytes)> {
        static_files!("admin/admin.css")
    }
}
