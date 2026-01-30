//! Forms and form fields for handling user input.
//!
//! This module provides a way to define forms and form fields for handling user
//! input in a web application. It provides a way to create forms from requests,
//! validate the form data, and render the form fields in an HTML form.
//!
//! # `Form` derive macro
//!
//! The easiest way to work with forms in Cot is to use the
//! [`Form`](derive@Form) derive macro. Just define a structure that will hold
//! all the form data you need, and derive the [`Form`] trait for it.
//!
//! ```
//! use cot::form::Form;
//!
//! #[derive(Form)]
//! struct MyForm {
//!     #[form(opts(max_length = 100))]
//!     name: String,
//! }
//! ```

mod field_value;
/// Built-in form fields that can be used in a form.
pub mod fields;

use std::borrow::Cow;
use std::fmt::Display;

use async_trait::async_trait;
use bytes::Bytes;
use chrono::NaiveDateTime;
use chrono_tz::Tz;
use cot_core::error::impl_into_cot_error;
use cot_core::headers::{MULTIPART_FORM_CONTENT_TYPE, URLENCODED_FORM_CONTENT_TYPE};
/// Derive the [`Form`] trait for a struct and create a [`FormContext`] for it.
///
/// This macro will generate an implementation of the [`Form`] trait for the
/// given named struct. Note that all the fields of the struct **must**
/// implement the [`AsFormField`] trait.
///
/// # Rendering
///
/// In order for the [`FormContext`] to be renderable in templates, all the form
/// fields (i.e. [`AsFormField::Type`]) must implement the [`Display`] and
/// [`askama::filters::HtmlSafe`] traits. If you are implementing your own form
/// field types, you should make sure they implement these traits (and you have
/// to make sure the types are safe to render as HTML, possibly escaping user
/// input if needed).
///
/// Note that even if the form is not rendered in a template, you will still be
/// able to render the fields individually.
///
/// # Safety
///
/// The implementation of [`Display`] for the form context that this derive
/// macro generates depends on the implementation of [`Display`] for the form
/// fields. If the form fields are not safe to render as HTML, the form context
/// will not be safe to render as HTML either.
pub use cot_macros::Form;
use derive_more::with_trait::Debug;
pub use field_value::{FormFieldValue, FormFieldValueError};
use http_body_util::BodyExt;
use thiserror::Error;

use crate::request::{Request, RequestExt};

const ERROR_PREFIX: &str = "failed to process a form:";
/// Error occurred while processing a form.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FormError {
    /// An error occurred while processing the request, before validating the
    /// form data.
    #[error("{ERROR_PREFIX} request error: {error}")]
    #[non_exhaustive]
    RequestError {
        /// The underlying error that occurred during request processing.
        #[from]
        error: Box<crate::Error>,
    },
    /// The underlying error that occurred during multipart form processing.
    #[error("{ERROR_PREFIX} multipart error: {error}")]
    #[non_exhaustive]
    MultipartError {
        /// The underlying error that occurred during multipart form processing.
        #[from]
        error: FormFieldValueError,
    },
}
impl_into_cot_error!(FormError, BAD_REQUEST);

/// The result of validating a form.
///
/// This enum is used to represent the result of validating a form. In the case
/// of a successful validation, the `Ok` variant contains the form object. In
/// the case of a failed validation, the `ValidationError` variant contains the
/// context object with the validation errors, as well as the user's input.
#[must_use]
#[derive(Debug, Clone)]
pub enum FormResult<T: Form> {
    /// The form validation passed.
    Ok(T),
    /// The form validation failed.
    ValidationError(T::Context),
}

impl<T: Form> FormResult<T> {
    /// Unwraps the form result, panicking if the form validation failed.
    ///
    /// This should only be used in tests or when the form validation
    /// is guaranteed to pass.
    ///
    /// # Panics
    ///
    /// Panics if the form validation failed.
    #[track_caller]
    pub fn unwrap(self) -> T {
        match self {
            Self::Ok(form) => form,
            Self::ValidationError(context) => panic!("Form validation failed: {context:?}"),
        }
    }
}

/// An error that can occur when validating a form field.
#[derive(Debug, Error, PartialEq, Eq)]
#[non_exhaustive]
#[error("{message}")]
pub enum FormFieldValidationError {
    /// The field is required.
    #[error("This field is required.")]
    Required,
    /// The field value is too long.
    #[error("This exceeds the maximum length of {max_length}.")]
    MaximumLengthExceeded {
        /// The maximum length of the field.
        max_length: u32,
    },

    /// The field value is too short.
    #[error("This is below the minimum length of {min_length}.")]
    MinimumLengthNotMet {
        /// The minimum length of the field.
        min_length: u32,
    },

    /// The field value is below the permitted minimum.
    #[error("This is below the minimum value of {min_value}.")]
    MinimumValueNotMet {
        /// The minimum permitted value.
        min_value: String,
    },

    /// The field value exceeds the permitted maximum.
    #[error("This exceeds the maximum value of {max_value}.")]
    MaximumValueExceeded {
        /// The maximum permitted value.
        max_value: String,
    },
    /// The field value is an ambiguous datetime.
    #[error("The datetime value `{datetime}` is ambiguous.")]
    AmbiguousDateTime {
        /// The ambiguous datetime value.
        datetime: NaiveDateTime,
    },
    /// The field value is a non-existent local datetime.
    #[error("Local datetime {datetime} does not exist for the specified timezone {timezone}.")]
    NonExistentLocalDateTime {
        /// The non-existent local datetime value.
        datetime: NaiveDateTime,
        /// The timezone in which the datetime was specified.
        timezone: Tz,
    },
    /// The field value is required to be true.
    #[error("This field must be checked.")]
    BooleanRequiredToBeTrue,
    /// The field value is invalid.
    #[error("Value is not valid for this field.")]
    InvalidValue(String),
    /// An error occurred while getting the field value.
    #[error("Error getting field value: {0}")]
    FormFieldValueError(#[from] FormFieldValueError),
    /// Custom error with a given message.
    #[error("{0}")]
    Custom(Cow<'static, str>),
}

impl FormFieldValidationError {
    /// Creates a new `FormFieldValidationError` for an invalid value of a
    /// field.
    #[must_use]
    pub fn invalid_value<T: Into<String>>(value: T) -> Self {
        Self::InvalidValue(value.into())
    }

    /// Creates a new `FormFieldValidationError` for a field value that is too
    /// long.
    #[must_use]
    pub fn maximum_length_exceeded(max_length: u32) -> Self {
        Self::MaximumLengthExceeded { max_length }
    }

    /// Creates a new `FormFieldValidationError` for a field value that is too
    /// short.
    #[must_use]
    pub fn minimum_length_not_met(min_length: u32) -> Self {
        FormFieldValidationError::MinimumLengthNotMet { min_length }
    }

    /// Creates a new `FormFieldValidatorError`for a field value below the
    /// permitted minimum value.
    #[must_use]
    pub fn minimum_value_not_met<T: Display>(min_value: T) -> Self {
        FormFieldValidationError::MinimumValueNotMet {
            min_value: min_value.to_string(),
        }
    }

    /// Creates a new `FormFieldValidationError` for a field value that exceeds
    /// the permitted maximum value
    #[must_use]
    pub fn maximum_value_exceeded<T: Display>(max_value: T) -> Self {
        FormFieldValidationError::MaximumValueExceeded {
            max_value: max_value.to_string(),
        }
    }

    /// Creates a new `FormFieldValidationError` for an ambiguous datetime.
    #[must_use]
    pub fn ambiguous_datetime(datetime: NaiveDateTime) -> Self {
        FormFieldValidationError::AmbiguousDateTime { datetime }
    }

    /// Creates a new `FormFieldValidationError` for a non-existent local
    /// datetime.
    #[must_use]
    pub fn non_existent_local_datetime(datetime: NaiveDateTime, timezone: Tz) -> Self {
        FormFieldValidationError::NonExistentLocalDateTime { datetime, timezone }
    }

    /// Creates a new `FormFieldValidationError` from a `String`.
    #[must_use]
    pub const fn from_string(message: String) -> Self {
        Self::Custom(Cow::Owned(message))
    }

    /// Creates a new `FormFieldValidationError` from a static string.
    #[must_use]
    pub const fn from_static(message: &'static str) -> Self {
        Self::Custom(Cow::Borrowed(message))
    }
}

/// An enum indicating the target of a form validation error.
#[derive(Debug)]
pub enum FormErrorTarget<'a> {
    /// An error targeting a single field.
    Field(&'a str),
    /// An error targeting the entire form.
    Form,
}

/// A trait for types that can be used as forms.
///
/// This trait is used to define a type that can be used as a form. It provides
/// a way to create a form from a request, build a context from the request, and
/// validate the form.
///
/// # Deriving
///
/// This trait can, and should be derived using the [`Form`](derive@Form) derive
/// macro. This macro generates the implementation of the trait for the type,
/// including the implementation of the [`FormContext`] trait for the context
/// type.
///
/// ```
/// use cot::form::Form;
///
/// #[derive(Form)]
/// struct MyForm {
///     #[form(opts(max_length = 100))]
///     name: String,
/// }
/// ```
#[async_trait]
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not implement the `Form` trait",
    label = "`{Self}` is not a form",
    note = "add #[derive(cot::form::Form)] to the struct to automatically derive the trait"
)]
pub trait Form: Sized {
    /// The context type associated with the form.
    type Context: FormContext + Send;

    /// Creates a form struct from a request.
    ///
    /// # Errors
    ///
    /// This method should return an error if the form data could not be read
    /// from the request.
    async fn from_request(request: &mut Request) -> Result<FormResult<Self>, FormError>;

    /// Creates the context for the form from `self`.
    ///
    /// This is useful for pre-populating forms with objects created in the code
    /// or obtained externally, such as from a database.
    async fn to_context(&self) -> Self::Context;

    /// Builds the context for the form from a request.
    ///
    /// Note that this doesn't try to convert the values from the form fields
    /// into the final types, so this context object may not include all the
    /// errors. The conversion is done in the [`Self::from_request`] method.
    ///
    /// # Errors
    ///
    /// This method should return an error if the form data could not be read
    /// from the request.
    async fn build_context(request: &mut Request) -> Result<Self::Context, FormError> {
        let mut context = Self::Context::new();

        let mut form_data = form_data(request).await?;

        while let Some((field_id, value)) = form_data.next_value().await? {
            if let Err(err) = context.set_value(&field_id, value).await {
                context.add_error(FormErrorTarget::Field(&field_id), err);
            }
        }

        Ok(context)
    }
}

async fn form_data(request: &mut Request) -> Result<FormData<'_>, FormError> {
    let form_data = if content_type_str(request).starts_with(MULTIPART_FORM_CONTENT_TYPE) {
        let multipart = multipart_form_data(request)?;

        FormData::Multipart { inner: multipart }
    } else {
        let form_data_bytes = urlencoded_form_data(request).await?;

        FormData::new_urlencoded(form_data_bytes)
    };
    Ok(form_data)
}

fn multipart_form_data(request: &mut Request) -> Result<multer::Multipart<'_>, FormError> {
    let content_type = content_type_str(request);

    let boundary =
        multer::parse_boundary(content_type).map_err(FormFieldValueError::from_multer)?;
    let body = std::mem::take(request.body_mut());
    let multipart = multer::Multipart::new(body.into_data_stream(), boundary);

    Ok(multipart)
}

async fn urlencoded_form_data(request: &mut Request) -> Result<Bytes, FormError> {
    let result = if request.method() == http::Method::GET || request.method() == http::Method::HEAD
    {
        if let Some(query) = request.uri().query() {
            Bytes::copy_from_slice(query.as_bytes())
        } else {
            Bytes::new()
        }
    } else if content_type_str(request) == URLENCODED_FORM_CONTENT_TYPE {
        let body = std::mem::take(request.body_mut());

        body.into_bytes()
            .await
            .map_err(|e| FormError::RequestError { error: Box::new(e) })?
    } else {
        return Err(FormError::RequestError {
            error: Box::new(crate::Error::from(ExpectedForm)),
        });
    };

    Ok(result)
}

#[derive(Debug, Error)]
#[error(
    "request does not contain a form (expected a POST request with \
    the `application/x-www-form-urlencoded` or `multipart/form-data` content type, \
    or a GET or HEAD request)"
)]
struct ExpectedForm;
impl_into_cot_error!(ExpectedForm, BAD_REQUEST);

fn content_type_str(request: &mut Request) -> String {
    request
        .content_type()
        .map_or("".into(), |value| String::from_utf8_lossy(value.as_bytes()))
        .into_owned()
}

#[derive(Debug)]
enum FormData<'a> {
    Form {
        #[debug("..")]
        inner: form_urlencoded::Parse<'a>,
        // needed to store the data used by the parser in `inner`
        // must be declared after `inner` to avoid dropping it first
        // see `new_urlencoded` for details
        _data: Bytes,
    },
    Multipart {
        inner: multer::Multipart<'a>,
    },
}

impl<'a> FormData<'a> {
    fn new_urlencoded(data: Bytes) -> Self {
        #[expect(unsafe_code)]
        let slice = unsafe {
            // SAFETY:
            // * `Bytes` guarantees that `data` is non-null, valid for reads for
            //   `data.len()` bytes
            // * `data` is not mutated inside `Bytes`
            // * data inside `slice` will not get deallocated as long as the underlying
            //   `Bytes` object is alive
            // * struct fields are dropped in the order of declaration, so `data` will be
            //   dropped after `inner`
            std::slice::from_raw_parts(data.as_ptr(), data.len())
        };

        FormData::Form {
            inner: form_urlencoded::parse(slice),
            _data: data,
        }
    }

    async fn next_value(
        &mut self,
    ) -> Result<Option<(String, FormFieldValue<'a>)>, FormFieldValueError> {
        match self {
            FormData::Form { inner, .. } => Ok(inner
                .next()
                .map(|(key, value)| (key.into_owned(), FormFieldValue::new_text(value)))),
            FormData::Multipart { inner } => {
                let next_field = inner.next_field().await;

                match next_field {
                    Ok(Some(field)) => {
                        let name = field
                            .name()
                            .ok_or_else(FormFieldValueError::no_name)?
                            .to_owned();
                        let value = FormFieldValue::new_multipart(field);

                        Ok(Some((name, value)))
                    }
                    Ok(None) => Ok(None),
                    Err(err) => Err(FormFieldValueError::from_multer(err)),
                }
            }
        }
    }
}

/// A trait for form contexts.
///
/// A form context is used to store the state of a form, such as the values of
/// the fields and any errors that occur during validation. This trait is used
/// to define the interface for a form context, which is used to interact with
/// the form fields and errors.
///
/// This trait is typically not implemented directly; instead, its
/// implementations are generated automatically through the
/// [`Form`](derive@Form) derive macro.
#[async_trait]
pub trait FormContext: Debug {
    /// Creates a new form context without any initial form data.
    fn new() -> Self
    where
        Self: Sized;

    /// Returns an iterator over the fields in the form.
    fn fields(&self) -> Box<dyn DoubleEndedIterator<Item = &dyn DynFormField> + '_>;

    /// Sets the value of a form field.
    ///
    /// # Errors
    ///
    /// This method should return an error if the value is invalid.
    async fn set_value(
        &mut self,
        field_id: &str,
        value: FormFieldValue<'_>,
    ) -> Result<(), FormFieldValidationError>;

    /// Adds a validation error to the form context.
    fn add_error(&mut self, target: FormErrorTarget<'_>, error: FormFieldValidationError) {
        self.errors_for_mut(target).push(error);
    }

    /// Returns the validation errors for a target in the form context.
    fn errors_for(&self, target: FormErrorTarget<'_>) -> &[FormFieldValidationError];

    /// Returns a mutable reference to the validation errors for a target in the
    /// form context.
    fn errors_for_mut(&mut self, target: FormErrorTarget<'_>)
    -> &mut Vec<FormFieldValidationError>;

    /// Returns whether the form context has any validation errors.
    fn has_errors(&self) -> bool;
}

/// Generic options valid for all types of form fields.
#[derive(Debug)]
pub struct FormFieldOptions {
    /// The HTML ID of the form field.
    pub id: String,
    /// Display name of the form field.
    pub name: String,
    /// Whether the field is required. Note that this really only adds the
    /// "required" attribute to the HTML input element, since by default all
    /// fields are required. If you want to make a field optional, just use
    /// [`Option`] in the struct definition.
    pub required: bool,
}

/// A form field.
///
/// This trait is used to define a type of field that can be used in a form. It
/// is used to render the field in an HTML form, set the value of the field, and
/// validate it. Typically, the implementors of this trait are used indirectly
/// through the [`Form`] trait and field types that implement [`AsFormField`].
pub trait FormField: Display {
    /// Custom options for the form field, unique for each field type.
    type CustomOptions: Default;

    /// Creates a new form field with the given options.
    fn with_options(options: FormFieldOptions, custom_options: Self::CustomOptions) -> Self
    where
        Self: Sized;

    /// Returns the generic options for the form field.
    fn options(&self) -> &FormFieldOptions;

    /// Returns the ID of the form field.
    fn id(&self) -> &str {
        &self.options().id
    }

    /// Returns the display name of the form field.
    fn name(&self) -> &str {
        &self.options().name
    }

    /// Returns the string value of the form field.
    fn value(&self) -> Option<&str>;

    /// Sets the value of the form field.
    ///
    /// This method should convert the value to the appropriate type for the
    /// field, such as a number for a number field.
    ///
    /// Note that this method might be called multiple times. This will happen
    /// when the field has appeared in the form data multiple times, such as
    /// in the case of a `<select multiple>` HTML element. If the field
    /// does not support storing multiple values, it should overwrite the
    /// previous value.
    fn set_value(
        &mut self,
        field: FormFieldValue<'_>,
    ) -> impl Future<Output = Result<(), FormFieldValueError>> + Send;
}

/// A version of [`FormField`] that can be used in a dynamic context.
///
/// This trait is used to allow a form field to be used in a dynamic context,
/// such as when using Form field iterator. It provides access to the field's
/// options, value, and rendering, among others.
///
/// This trait is implemented for all types that implement [`FormField`].
#[async_trait]
pub trait DynFormField: Display {
    /// Returns the generic options for the form field.
    fn dyn_options(&self) -> &FormFieldOptions;

    /// Returns the HTML ID of the form field.
    fn dyn_id(&self) -> &str;

    /// Returns the string value of the form field if any has been set (and
    /// is applicable to the field type).
    fn dyn_value(&self) -> Option<&str>;

    /// Sets the value of the form field.
    async fn dyn_set_value(&mut self, field: FormFieldValue<'_>)
    -> Result<(), FormFieldValueError>;
}

#[async_trait]
impl<T: FormField + Send> DynFormField for T {
    fn dyn_options(&self) -> &FormFieldOptions {
        FormField::options(self)
    }

    fn dyn_id(&self) -> &str {
        FormField::id(self)
    }

    fn dyn_value(&self) -> Option<&str> {
        FormField::value(self)
    }

    async fn dyn_set_value(
        &mut self,
        field: FormFieldValue<'_>,
    ) -> Result<(), FormFieldValueError> {
        FormField::set_value(self, field).await
    }
}

/// A trait for types that can be used as form fields.
///
/// This trait uses [`FormField`] to define a type that can be used as a form
/// field. It provides a way to clean the value of the field, which is used to
/// validate the field's value before converting to the final type.
pub trait AsFormField {
    /// The form field type associated with the field.
    type Type: FormField;

    /// Creates a new form field with the provided generic and type-specific
    /// options.
    ///
    /// This method is used to create a new instance of the form field with the
    /// given options and custom options. The options are used to set the
    /// properties of the field, such as the ID and whether the field is
    /// required.
    ///
    /// The custom options are unique to each field type and are used to set
    /// additional properties of the field.
    fn new_field(
        options: FormFieldOptions,
        custom_options: <Self::Type as FormField>::CustomOptions,
    ) -> Self::Type {
        Self::Type::with_options(options, custom_options)
    }

    /// Validates the value of the field and converts it to the final type. This
    /// method should return an error if the value is invalid.
    ///
    /// # Errors
    ///
    /// Returns an error if the value fails to validate or convert to the final
    /// type
    fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError>
    where
        Self: Sized;

    /// Returns `self` as a value that can be set with [`FormField::set_value`].
    fn to_field_value(&self) -> String;
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use cot_core::headers::{MULTIPART_FORM_CONTENT_TYPE, URLENCODED_FORM_CONTENT_TYPE};

    use super::*;
    use crate::Body;

    #[cot::test]
    async fn urlencoded_form_data_extract_get_empty() {
        let mut request = http::Request::builder()
            .method(http::Method::GET)
            .uri("https://example.com")
            .body(Body::empty())
            .unwrap();

        let bytes = urlencoded_form_data(&mut request).await.unwrap();
        assert_eq!(bytes, Bytes::from_static(b""));
    }

    #[cot::test]
    async fn urlencoded_form_data_extract_get() {
        let mut request = http::Request::builder()
            .method(http::Method::GET)
            .uri("https://example.com/?hello=world")
            .body(Body::empty())
            .unwrap();

        let bytes = urlencoded_form_data(&mut request).await.unwrap();
        assert_eq!(bytes, Bytes::from_static(b"hello=world"));
    }

    #[cot::test]
    async fn urlencoded_form_data_extract_head() {
        let mut request = http::Request::builder()
            .method(http::Method::HEAD)
            .uri("https://example.com/?hello=world")
            .body(Body::empty())
            .unwrap();

        let bytes = urlencoded_form_data(&mut request).await.unwrap();
        assert_eq!(bytes, Bytes::from_static(b"hello=world"));
    }

    #[cot::test]
    async fn urlencoded_form_data_extract_urlencoded() {
        let mut request = http::Request::builder()
            .method(http::Method::POST)
            .header(http::header::CONTENT_TYPE, URLENCODED_FORM_CONTENT_TYPE)
            .body(Body::fixed("hello=world"))
            .unwrap();

        let result = urlencoded_form_data(&mut request).await.unwrap();
        assert_eq!(result, Bytes::from_static(b"hello=world"));
    }

    #[cot::test]
    async fn form_data_extract_multipart() {
        let boundary = "boundary";
        let body = format!(
            "--{boundary}\r\n\
            Content-Disposition: form-data; name=\"hello\"\r\n\
            \r\n\
            world\r\n\
            --{boundary}\r\n\
            Content-Disposition: form-data; name=\"test\"\r\n\
            \r\n\
            123\r\n\
            --{boundary}--\r\n"
        );

        let mut request = http::Request::builder()
            .method(http::Method::POST)
            .header(
                http::header::CONTENT_TYPE,
                format!("{MULTIPART_FORM_CONTENT_TYPE}; boundary={boundary}"),
            )
            .body(Body::fixed(body))
            .unwrap();

        let mut form_data = form_data(&mut request).await.unwrap();

        let mut values = Vec::new();
        while let Some((field_id, value)) = form_data.next_value().await.unwrap() {
            values.push((field_id, value.into_text().await.unwrap()));
        }
        assert_eq!(values.len(), 2);
        assert_eq!(values[0].0, "hello");
        assert_eq!(values[0].1, "world");
        assert_eq!(values[1].0, "test");
        assert_eq!(values[1].1, "123");
    }

    #[cot::test]
    async fn form_data_extract_multipart_with_file() {
        let boundary = "boundary";
        let body = format!(
            "--{boundary}\r\n\
            Content-Disposition: form-data; name=\"hello\"\r\n\
            \r\n\
            world\r\n\
            --{boundary}\r\n\
            Content-Disposition: form-data; name=\"file\"; filename=\"test.txt\"\r\n\
            Content-Type: text/plain\r\n\
            \r\n\
            file content\r\n\
            --{boundary}--\r\n"
        );

        let mut request = http::Request::builder()
            .method(http::Method::POST)
            .header(
                http::header::CONTENT_TYPE,
                format!("{MULTIPART_FORM_CONTENT_TYPE}; boundary={boundary}"),
            )
            .body(Body::fixed(body))
            .unwrap();

        let mut form_data = form_data(&mut request).await.unwrap();

        let mut values = Vec::new();
        while let Some((field_id, value)) = form_data.next_value().await.unwrap() {
            assert!(value.is_multipart());
            values.push((
                field_id,
                value.filename().map(ToOwned::to_owned),
                value.content_type().map(ToOwned::to_owned),
                value.into_text().await.unwrap(),
            ));
        }

        assert_eq!(values.len(), 2);
        assert_eq!(values[0].0, "hello");
        assert_eq!(values[0].1, None);
        assert_eq!(values[0].2, None);
        assert_eq!(values[0].3, "world");
        assert_eq!(values[1].0, "file");
        assert_eq!(values[1].1, Some("test.txt".to_owned()));
        assert_eq!(values[1].2, Some("text/plain".to_owned()));
        assert_eq!(values[1].3, "file content");
    }

    #[cot::test]
    async fn form_data_extract_invalid_content_type() {
        let mut request = http::Request::builder()
            .method(http::Method::POST)
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(Body::fixed("{}"))
            .unwrap();

        let result = form_data(&mut request).await;

        assert!(result.is_err());
        if let Err(FormError::RequestError { error }) = result {
            assert!(
                error
                    .to_string()
                    .contains("request does not contain a form"),
                "{}",
                error
            );
        } else {
            panic!("Expected RequestError");
        }
    }
}
