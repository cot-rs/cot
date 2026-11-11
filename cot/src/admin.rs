//! Administration panel.
//!
//! This module provides an administration panel for managing models
//! registered in the application, straight from the web interface.

mod app;

use std::any::Any;
use std::marker::PhantomData;

pub use app::AdminApp;
use async_trait::async_trait;
/// Implements the [`AdminModel`] trait for a struct.
///
/// This is a simple method for adding a database model to the admin panel.
/// Note that in order for this derive macro to work, the structure
/// **must** implement [`Model`](crate::db::Model) and
/// [`Form`](crate::form::Form) traits. These can also be derived using the
/// `#[model]` and `#[derive(Form)]` attributes.
pub use cot_macros::AdminModel;
use derive_more::Debug;

use crate::form::FormContext;
use crate::request::Request;

/// Struct representing the pagination of objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Pagination {
    limit: u64,
    offset: u64,
}

impl Pagination {
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
    async fn form_context_from_object(&self, object: Box<dyn AdminModel>) -> Box<dyn FormContext>;

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
    /// Returns an error if the object could not be removed, for example,
    /// a database error.
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
        #[expect(trivial_casts)] // Upcast to the correct Box type
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
        #[expect(trivial_casts)] // Upcast to the correct Box type
        T::get_object_by_id(request, id)
            .await
            .map(|object| object.map(|object| Box::new(object) as Box<dyn AdminModel>))
    }

    fn form_context(&self) -> Box<dyn FormContext> {
        T::form_context()
    }

    async fn form_context_from_object(&self, object: Box<dyn AdminModel>) -> Box<dyn FormContext> {
        let object_any: &dyn Any = &*object;
        let object_casted = object_any.downcast_ref::<T>().expect("Invalid object type");

        T::form_context_from_self(object_casted).await
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
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not implement the `AdminModel` trait",
    label = "`{Self}` is not an admin model",
    note = "add #[derive(cot::admin::AdminModel)] to the struct to automatically derive the trait"
)]
pub trait AdminModel: Any + Send + 'static {
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
    async fn form_context_from_self(&self) -> Box<dyn FormContext>;

    /// Save the model instance from the form data in the request.
    ///
    /// # Errors
    ///
    /// Returns an error if the object could not be saved, for example,
    /// a database error.
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
    /// Returns an error if the object could not be removed, for example,
    /// a database error.
    async fn remove_by_id(request: &mut Request, object_id: &str) -> cot::Result<()>
    where
        Self: Sized;
}
