//! The admin panel app and its views.

use std::marker::PhantomData;

use bytes::Bytes;
use derive_more::Debug;
use serde::Deserialize;

use super::{AdminModel, AdminModelManager, Pagination};
use crate::auth::Auth;
use crate::common_types::Password;
use crate::error::NotFound;
use crate::form::{
    Form, FormContext, FormErrorTarget, FormField, FormFieldValidationError, FormResult,
};
use crate::html::Html;
use crate::request::extractors::{FromRequestHead, Path, StaticFiles, UrlQuery};
use crate::request::{Request, RequestExt, RequestHead};
use crate::response::{IntoResponse, Response};
use crate::router::{Router, Urls};
use crate::static_files::StaticFile;
use crate::{App, Error, Method, RequestHandler, Template, reverse_redirect};

struct AdminAuthenticated<T, H: Send + Sync>(H, PhantomData<fn() -> T>);

impl<T, H: RequestHandler<T> + Send + Sync> AdminAuthenticated<T, H> {
    #[must_use]
    fn new(handler: H) -> Self {
        Self(handler, PhantomData)
    }
}

impl<T, H: RequestHandler<T> + Send + Sync> RequestHandler<T> for AdminAuthenticated<T, H> {
    async fn handle(&self, mut request: Request) -> crate::Result<Response> {
        let auth: Auth = request.extract_from_head().await?;
        if !auth.user().is_authenticated() {
            return Ok(reverse_redirect!(request, "login")?);
        }

        self.0.handle(request).await
    }
}

#[derive(Debug, FromRequestHead)]
struct BaseContext {
    urls: Urls,
    static_files: StaticFiles,
}

async fn index(
    base_context: BaseContext,
    AdminModelManagers(managers): AdminModelManagers,
) -> crate::Result<Html> {
    #[derive(Debug, Template)]
    #[template(path = "admin/model_list.html")]
    struct ModelListTemplate<'a> {
        ctx: &'a BaseContext,
        #[debug("..")]
        model_managers: Vec<Box<dyn AdminModelManager>>,
    }

    let template = ModelListTemplate {
        ctx: &base_context,
        model_managers: managers,
    };
    Ok(Html::new(template.render()?))
}

#[derive(Debug, Form)]
struct LoginForm {
    username: String,
    password: Password,
}

async fn login(
    base_context: BaseContext,
    auth: Auth,
    mut request: Request,
) -> crate::Result<Response> {
    #[derive(Debug, Template)]
    #[template(path = "admin/login.html")]
    struct LoginTemplate<'a> {
        ctx: &'a BaseContext,
        form: <LoginForm as Form>::Context,
    }

    let login_form_context = if request.method() == Method::GET {
        LoginForm::build_context(&mut request).await?
    } else if request.method() == Method::POST {
        let login_form = LoginForm::from_request(&mut request).await?;
        match login_form {
            FormResult::Ok(login_form) => {
                if authenticate(&auth, login_form).await? {
                    return Ok(reverse_redirect!(base_context.urls, "index")?);
                }

                let mut context = LoginForm::build_context(&mut request).await?;
                context.add_error(
                    FormErrorTarget::Form,
                    FormFieldValidationError::from_static("Invalid username or password."),
                );
                context
            }
            FormResult::ValidationError(context) => context,
        }
    } else {
        panic!("Unexpected request method");
    };

    let template = LoginTemplate {
        ctx: &base_context,
        form: login_form_context,
    };
    Html::new(template.render()?).into_response()
}

async fn authenticate(auth: &Auth, login_form: LoginForm) -> crate::Result<bool> {
    #[cfg(feature = "db")]
    let user = auth
        .authenticate(&crate::auth::user_pass::DatabaseUserCredentials::new(
            login_form.username,
            Password::new(login_form.password.into_string()),
        ))
        .await?;

    #[cfg(not(feature = "db"))]
    let user: Option<Box<dyn crate::auth::User + Send + Sync>> = None;

    if let Some(user) = user {
        auth.login(user).await?;
        Ok(true)
    } else {
        Ok(false)
    }
}

impl Pagination {
    fn new(limit: u64, page: u64) -> Self {
        assert!(page > 0, "Page number must be greater than 0");

        Self {
            limit,
            offset: (page - 1) * limit,
        }
    }
}

#[derive(Debug, Deserialize)]
struct PaginationParams {
    page: Option<u64>,
    page_size: Option<u64>,
}

async fn view_model(
    base_context: BaseContext,
    managers: AdminModelManagers,
    Path(model_name): Path<String>,
    UrlQuery(pagination_params): UrlQuery<PaginationParams>,
    request: Request,
) -> crate::Result<Response> {
    #[derive(Debug, Template)]
    #[template(path = "admin/model.html")]
    struct ModelTemplate<'a> {
        ctx: &'a BaseContext,
        #[debug("..")]
        model: &'a dyn AdminModelManager,
        #[debug("..")]
        objects: Vec<Box<dyn AdminModel>>,
        page: u64,
        page_size: &'a u64,
        total_object_counts: u64,
        total_pages: u64,
    }

    const DEFAULT_PAGE_SIZE: u64 = 10;

    let manager = get_manager(managers, &model_name)?;

    let page = pagination_params.page.unwrap_or(1);
    let page_size = pagination_params.page_size.unwrap_or(DEFAULT_PAGE_SIZE);

    let total_object_counts = manager.get_total_object_counts(&request).await?;
    let total_pages = total_object_counts.div_ceil(page_size);

    if (page == 0 || page > total_pages) && total_pages > 0 {
        return Err(Error::from(NotFound::with_message(format!(
            "page {page} not found"
        ))));
    }

    let pagination = Pagination::new(page_size, page);

    let objects = manager.get_objects(&request, pagination).await?;

    let template = ModelTemplate {
        ctx: &base_context,
        model: &*manager,
        objects,
        page,
        page_size: &page_size,
        total_object_counts,
        total_pages,
    };

    Html::new(template.render()?).into_response()
}

async fn create_model_instance(
    base_context: BaseContext,
    managers: AdminModelManagers,
    Path(model_name): Path<String>,
    request: Request,
) -> cot::Result<Response> {
    edit_model_instance_impl(base_context, managers, request, &model_name, None).await
}

async fn edit_model_instance(
    base_context: BaseContext,
    managers: AdminModelManagers,
    Path((model_name, object_id)): Path<(String, String)>,
    request: Request,
) -> cot::Result<Response> {
    edit_model_instance_impl(
        base_context,
        managers,
        request,
        &model_name,
        Some(&object_id),
    )
    .await
}

async fn edit_model_instance_impl(
    base_context: BaseContext,
    managers: AdminModelManagers,
    mut request: Request,
    model_name: &str,
    object_id: Option<&str>,
) -> cot::Result<Response> {
    #[derive(Debug, Template)]
    #[template(path = "admin/model_edit.html")]
    struct ModelEditTemplate<'a> {
        ctx: &'a BaseContext,
        #[debug("..")]
        model: &'a dyn AdminModelManager,
        form_context: Box<dyn FormContext>,
        is_edit: bool,
    }

    let manager = get_manager(managers, model_name)?;

    let form_context = if request.method() == Method::POST {
        let form_context = manager.save_from_request(&mut request, object_id).await?;

        if let Some(form_context) = form_context {
            form_context
        } else {
            return Ok(reverse_redirect!(
                base_context.urls,
                "view_model",
                model_name = manager.url_name()
            )?);
        }
    } else if let Some(object_id) = object_id {
        let object = get_object(&mut request, &*manager, object_id).await?;

        manager.form_context_from_object(object).await
    } else {
        manager.form_context()
    };

    let template = ModelEditTemplate {
        ctx: &base_context,
        model: &*manager,
        form_context,
        is_edit: object_id.is_some(),
    };

    Html::new(template.render()?).into_response()
}

async fn remove_model_instance(
    base_context: BaseContext,
    managers: AdminModelManagers,
    Path((model_name, object_id)): Path<(String, String)>,
    mut request: Request,
) -> cot::Result<Response> {
    #[derive(Debug, Template)]
    #[template(path = "admin/model_remove.html")]
    struct ModelRemoveTemplate<'a> {
        ctx: &'a BaseContext,
        #[debug("..")]
        model: &'a dyn AdminModelManager,
        #[debug("..")]
        object: &'a dyn AdminModel,
    }

    let manager = get_manager(managers, &model_name)?;
    let object = get_object(&mut request, &*manager, &object_id).await?;

    if request.method() == Method::POST {
        manager.remove_by_id(&mut request, &object_id).await?;

        Ok(reverse_redirect!(
            base_context.urls,
            "view_model",
            model_name = manager.url_name()
        )?)
    } else {
        let template = ModelRemoveTemplate {
            ctx: &base_context,
            model: &*manager,
            object: &*object,
        };

        Html::new(template.render()?).into_response()
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
            Error::from(NotFound::with_message(format!(
                "Object with ID `{}` not found in model `{}`",
                object_id,
                manager.name()
            )))
        })
}

fn get_manager(
    AdminModelManagers(model_managers): AdminModelManagers,
    model_name: &str,
) -> cot::Result<Box<dyn AdminModelManager>> {
    model_managers
        .into_iter()
        .find(|manager| manager.url_name() == model_name)
        .ok_or_else(|| {
            Error::from(NotFound::with_message(format!(
                "Model `{model_name}` not found"
            )))
        })
}

#[repr(transparent)]
struct AdminModelManagers(Vec<Box<dyn AdminModelManager>>);

impl FromRequestHead for AdminModelManagers {
    #[expect(clippy::unused_async_trait_impl)]
    async fn from_request_head(head: &RequestHead) -> cot::Result<Self> {
        let managers = head
            .context()
            .apps()
            .iter()
            .flat_map(|app| app.admin_model_managers())
            .collect();
        Ok(Self(managers))
    }
}

/// The admin app.
///
/// # Examples
///
/// ```
/// use cot::admin::AdminApp;
/// use cot::project::RegisterAppsContext;
/// use cot::{AppBuilder, Project};
///
/// struct MyProject;
/// impl Project for MyProject {
///     fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
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
    /// use cot::project::RegisterAppsContext;
    /// use cot::{AppBuilder, Project};
    ///
    /// struct MyProject;
    /// impl Project for MyProject {
    ///     fn register_apps(&self, apps: &mut AppBuilder, _context: &RegisterAppsContext) {
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

    fn static_files(&self) -> Vec<StaticFile> {
        vec![StaticFile::new(
            "admin/admin.css",
            Bytes::from_static(include_bytes!(concat!(
                env!("OUT_DIR"),
                "/static/admin/admin.css"
            ))),
        )]
    }
}
