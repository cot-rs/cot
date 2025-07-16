use std::error::Error as StdError;
use std::fmt::Display;

use bytes::Bytes;
use thiserror::Error;

use crate::error::error_impl::impl_into_cot_error;

/// A value from a form field.
///
/// This type represents a value from a form field, which can be either a text
/// value or a multipart field (like a file upload). It provides methods to
/// access the field's metadata (name, filename, content type) and to convert
/// the value into different formats.
#[derive(Debug)]
pub struct FormFieldValue<'a> {
    inner: FormFieldValueImpl<'a>,
}

impl<'a> FormFieldValue<'a> {
    /// Creates a new text field value.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::form::FormFieldValue;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let value = FormFieldValue::new_text("hello");
    /// assert_eq!(value.is_multipart(), false);
    /// assert_eq!(value.filename(), None);
    /// assert_eq!(value.content_type(), None);
    /// assert_eq!(value.into_text().await?, "hello");
    /// # Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn new_text<T: Into<String>>(text: T) -> Self {
        Self {
            inner: FormFieldValueImpl::Text(text.into()),
        }
    }

    /// Creates a new multipart field value.
    #[must_use]
    pub(crate) fn new_multipart(field: multer::Field<'a>) -> Self {
        Self {
            inner: FormFieldValueImpl::Multipart(Box::new(MultipartField { inner: field })),
        }
    }

    /// Returns the filename of the field, if it has one.
    ///
    /// Only multipart fields can have filenames. Text fields always return
    /// `None`.
    #[must_use]
    pub fn filename(&self) -> Option<&str> {
        match &self.inner {
            FormFieldValueImpl::Text(_) => None,
            FormFieldValueImpl::Multipart(multipart) => multipart.inner.file_name(),
        }
    }

    /// Returns the content type of the field, if it has one.
    ///
    /// Only multipart fields can have content types. Text fields always return
    /// `None`.
    #[must_use]
    pub fn content_type(&self) -> Option<&str> {
        match &self.inner {
            FormFieldValueImpl::Text(_) => None,
            FormFieldValueImpl::Multipart(multipart) => {
                multipart.inner.content_type().map(AsRef::as_ref)
            }
        }
    }

    /// Converts the field value into bytes.
    ///
    /// For text fields, this converts the text into bytes. For multipart
    /// fields, this reads the field's content as bytes.
    ///
    /// # Errors
    ///
    /// This method can return an error if the field is a multipart field and
    /// the content cannot be read, for example, because of an I/O error.
    ///
    /// # Examples
    ///
    /// ```
    /// use bytes::Bytes;
    /// use cot::form::FormFieldValue;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let text_value = FormFieldValue::new_text("hello");
    /// assert_eq!(text_value.into_bytes().await?, Bytes::from("hello"));
    /// # Ok(())
    /// # }
    /// ```
    pub async fn into_bytes(self) -> Result<Bytes, FormFieldValueError> {
        match self.inner {
            FormFieldValueImpl::Text(text) => Ok(Bytes::from(text)),
            FormFieldValueImpl::Multipart(multipart) => multipart
                .inner
                .bytes()
                .await
                .map_err(FormFieldValueError::from_multer),
        }
    }

    /// Converts the field value into text.
    ///
    /// For text fields, this returns the text directly. For multipart fields,
    /// this reads the field's content as text.
    ///
    /// # Errors
    ///
    /// This method can return an error if the field is a multipart field and
    /// the content cannot be read, for example, because of an I/O error.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::form::FormFieldValue;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// let text_value = FormFieldValue::new_text("hello");
    /// assert_eq!(text_value.into_text().await?, "hello");
    /// # Ok(())
    /// # }
    /// ```
    pub async fn into_text(self) -> Result<String, FormFieldValueError> {
        match self.inner {
            FormFieldValueImpl::Text(text) => Ok(text),
            FormFieldValueImpl::Multipart(multipart) => multipart
                .inner
                .text()
                .await
                .map_err(FormFieldValueError::from_multer),
        }
    }

    /// Returns whether this is a multipart field.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::form::FormFieldValue;
    ///
    /// let text_value = FormFieldValue::new_text("hello");
    /// assert!(!text_value.is_multipart());
    /// ```
    #[must_use]
    pub fn is_multipart(&self) -> bool {
        matches!(self.inner, FormFieldValueImpl::Multipart(_))
    }
}

#[derive(Debug)]
enum FormFieldValueImpl<'a> {
    Text(String),
    Multipart(Box<MultipartField<'a>>),
}

#[derive(Debug)]
struct MultipartField<'a> {
    inner: multer::Field<'a>,
}

/// An error that can occur when processing a form field value.
///
/// This type represents errors that can occur when processing form field
/// values, such as errors from the multipart parser or validation errors.
#[derive(Debug, PartialEq, Eq)]
pub struct FormFieldValueError {
    inner: FormFieldValueErrorImpl,
}
impl_into_cot_error!(FormFieldValueError, BAD_REQUEST);

impl Display for FormFieldValueError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "failed to retrieve the value of a form field: {}",
            self.inner
        )
    }
}

impl StdError for FormFieldValueError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.inner.source()
    }
}

#[derive(Debug, PartialEq, Eq, Error)]
enum FormFieldValueErrorImpl {
    #[error(transparent)]
    Multer(multer::Error),
    #[error("multipart field does not have a name")]
    NoName,
    #[error("file field requires the form to be sent as `multipart/form-data`")]
    MultipartRequired,
}

impl FormFieldValueError {
    pub(crate) fn from_multer(multer: multer::Error) -> Self {
        Self {
            inner: FormFieldValueErrorImpl::Multer(multer),
        }
    }

    pub(crate) fn no_name() -> Self {
        Self {
            inner: FormFieldValueErrorImpl::NoName,
        }
    }

    pub(crate) fn multipart_required() -> Self {
        Self {
            inner: FormFieldValueErrorImpl::MultipartRequired,
        }
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use futures_util::stream::once;
    use multer::Multipart;

    use super::*;

    #[test]
    fn form_field_value_error_display() {
        let error = FormFieldValueError::no_name();
        assert_eq!(
            error.to_string(),
            "failed to retrieve the value of a form field: multipart field does not have a name"
        );

        let error = FormFieldValueError::from_multer(multer::Error::IncompleteStream);
        assert_eq!(
            error.to_string(),
            "failed to retrieve the value of a form field: incomplete multipart stream"
        );
    }

    #[test]
    fn form_field_value_error_source() {
        let error = FormFieldValueError::no_name();
        assert!(error.source().is_none());

        let error = FormFieldValueError::from_multer(multer::Error::DecodeHeaderName {
            name: "test".to_string(),
            cause: Box::new(std::io::Error::other("oh no!")),
        });
        assert!(error.source().is_some());
        assert_eq!(error.source().unwrap().to_string(), "oh no!");
    }

    #[cot::test]
    async fn text_field_value() {
        let value = FormFieldValue::new_text("hello");

        assert!(!value.is_multipart());
        assert_eq!(value.filename(), None);
        assert_eq!(value.content_type(), None);
        assert_eq!(value.into_text().await.unwrap(), "hello");

        let value = FormFieldValue::new_text("hello");

        assert_eq!(value.into_bytes().await.unwrap(), Bytes::from("hello"));
    }

    #[cot::test]
    async fn multipart_field_value() {
        let boundary = "boundary";
        let body = format!(
            "--{boundary}\r\n\
            Content-Disposition: form-data; name=\"file\"; filename=\"test.txt\"\r\n\
            Content-Type: text/plain\r\n\
            \r\n\
            file content\r\n\
            --{boundary}--\r\n"
        );

        let value = test_multipart_value(boundary, body.clone()).await;
        assert!(value.is_multipart());
        assert_eq!(value.filename(), Some("test.txt"));
        assert_eq!(value.content_type(), Some("text/plain"));
        assert_eq!(value.into_text().await.unwrap(), "file content");

        let value = test_multipart_value(boundary, body).await;
        assert_eq!(value.into_bytes().await.unwrap(), "file content");
    }

    async fn test_multipart_value(boundary: &str, body: String) -> FormFieldValue<'static> {
        let stream = once(async move { Ok::<_, std::io::Error>(Bytes::from(body)) });
        let mut multipart = Multipart::new(stream, boundary);

        let field = multipart.next_field().await.unwrap().unwrap();
        FormFieldValue::new_multipart(field)
    }
}
