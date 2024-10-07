use std::marker::PhantomData;

use askama::Template;
use async_trait::async_trait;
use derive_more::Debug;

use crate::auth::db::DatabaseUserCredentials;
use crate::auth::AuthRequestExt;
use crate::forms::fields::Password;
use crate::forms::{
    Form, FormContext, FormErrorTarget, FormField, FormFieldValidationError, FormResult,
};
use crate::request::{Request, RequestExt};
use crate::response::{Response, ResponseExt};
use crate::router::Router;
use crate::{reverse, Body, FlareonApp, Render, StatusCode};

#[derive(Debug, Form)]
struct LoginForm {
    username: String,
    password: Password,
}

#[derive(Debug, Template)]
#[template(path = "admin/login.html")]
struct LoginTemplate<'a> {
    request: &'a Request,
    form: <LoginForm as Form>::Context,
}

#[derive(Debug, Template)]
#[template(path = "admin/model_list.html")]
struct ModelListTemplate<'a> {
    request: &'a Request,
    #[debug("...")]
    model_managers: Vec<Box<dyn AdminModelManager>>,
}

#[derive(Debug, Template)]
#[template(path = "admin/model.html")]
struct ModelTemplate<'a> {
    request: &'a Request,
    #[debug("...")]
    objects: Vec<Box<dyn AdminModel>>,
}

async fn index(mut request: Request) -> flareon::Result<Response> {
    if request.user().await?.is_authenticated() {
        let template = ModelListTemplate {
            request: &request,
            model_managers: admin_model_managers(&request),
        };
        Ok(Response::new_html(
            StatusCode::OK,
            Body::fixed(template.render()?),
        ))
    } else {
        Ok(reverse!(request, "login"))
    }
}

async fn login(mut request: Request) -> flareon::Result<Response> {
    let login_form_context = if request.method() == http::Method::GET {
        LoginForm::build_context(&mut request).await?
    } else if request.method() == http::Method::POST {
        let login_form = LoginForm::from_request(&mut request).await?;
        match login_form {
            FormResult::Ok(login_form) => {
                if authenticate(&mut request, login_form).await? {
                    return Ok(reverse!(request, "index"));
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

async fn authenticate(request: &mut Request, login_form: LoginForm) -> flareon::Result<bool> {
    let user = request
        .authenticate(&DatabaseUserCredentials::new(
            login_form.username,
            // TODO unify auth::Password and forms::fields::Password
            flareon::auth::Password::new(login_form.password.into_string()),
        ))
        .await?;

    if let Some(user) = user {
        request.login(user).await?;
        Ok(true)
    } else {
        Ok(false)
    }
}

async fn view_model(mut request: Request) -> flareon::Result<Response> {
    if request.user().await?.is_authenticated() {
        // TODO use a nice URL parser instead of unwrap
        let model_name = request.path_params().get("model_name").unwrap();

        let model_managers = admin_model_managers(&request);
        let manager = model_managers
            .iter()
            .find(|manager| manager.url_name() == model_name)
            .unwrap(); // TODO throw 404

        let template = ModelTemplate {
            request: &request,
            objects: manager.get_objects(&request).await?,
        };
        Ok(Response::new_html(
            StatusCode::OK,
            Body::fixed(template.render()?),
        ))
    } else {
        Ok(reverse!(request, "login"))
    }
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

#[async_trait]
pub trait AdminModelManager: Send + Sync {
    fn name(&self) -> &str;

    fn url_name(&self) -> &str;

    async fn get_objects(&self, request: &Request) -> flareon::Result<Vec<Box<dyn AdminModel>>>;
}

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

    async fn get_objects(&self, request: &Request) -> flareon::Result<Vec<Box<dyn AdminModel>>> {
        #[allow(trivial_casts)] // Upcast to the correct Box type
        T::get_objects(request).await.map(|objects| {
            objects
                .into_iter()
                .map(|object| Box::new(object) as Box<dyn AdminModel>)
                .collect()
        })
    }
}

#[async_trait]
pub trait AdminModel {
    async fn get_objects(request: &Request) -> flareon::Result<Vec<Self>>
    where
        Self: Sized;

    fn name() -> &'static str
    where
        Self: Sized;

    fn url_name() -> &'static str
    where
        Self: Sized;

    fn display(&self) -> String;
}

#[derive(Debug, Copy, Clone)]
pub struct AdminApp;

impl Default for AdminApp {
    fn default() -> Self {
        Self::new()
    }
}

impl AdminApp {
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }
}

impl FlareonApp for AdminApp {
    fn name(&self) -> &str {
        "flareon_admin"
    }

    fn router(&self) -> Router {
        Router::with_urls([
            crate::Route::with_handler_and_name("/", index, "index"),
            crate::Route::with_handler_and_name("/login", login, "login"),
            crate::Route::with_handler_and_name("/:model_name", view_model, "view_model"),
        ])
    }
}
