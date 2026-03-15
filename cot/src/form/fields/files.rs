use std::fmt::{Display, Formatter};

use askama::filters::HtmlSafe;
use bytes::Bytes;
use cot::form::{AsFormField, FormFieldValidationError};
use cot::html::HtmlTag;

use crate::form::{FormField, FormFieldOptions, FormFieldValue, FormFieldValueError};

#[derive(Debug)]
/// A form field for a file.
pub struct FileField {
    options: FormFieldOptions,
    custom_options: FileFieldOptions,
    filename: Option<String>,
    content_type: Option<String>,
    data: Option<Bytes>,
}

impl FormField for FileField {
    type CustomOptions = FileFieldOptions;

    fn with_options(options: FormFieldOptions, custom_options: Self::CustomOptions) -> Self {
        Self {
            options,
            custom_options,
            filename: None,
            content_type: None,
            data: None,
        }
    }

    fn options(&self) -> &FormFieldOptions {
        &self.options
    }

    fn value(&self) -> Option<&str> {
        None
    }

    async fn set_value(&mut self, field: FormFieldValue<'_>) -> Result<(), FormFieldValueError> {
        if !field.is_multipart() {
            return Err(FormFieldValueError::multipart_required());
        }

        self.filename = field.filename().map(ToOwned::to_owned);
        self.content_type = field.content_type().map(ToOwned::to_owned);
        self.data = Some(field.into_bytes().await?);
        Ok(())
    }
}

/// Custom options for a [`FileField`].
#[derive(Debug, Default, Clone)]
pub struct FileFieldOptions {
    /// The accepted file types. Used to set the [`accept` attribute] in the
    /// HTML input element. Each string in the vector represents a file type
    /// or extension that the browser should accept.
    ///
    /// Examples:
    /// - `"image/*"` - Accepts all image types
    /// - `".pdf"` - Accepts PDF files
    /// - `"application/pdf"` - Accepts PDF files by MIME type
    ///
    /// [`accept` attribute]: https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/input/file#limiting_accepted_file_types
    pub accept: Option<Vec<String>>,
}

impl Display for FileField {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut tag = HtmlTag::input("file");
        tag.attr("name", self.id());
        tag.attr("id", self.id());
        if self.options.required {
            tag.bool_attr("required");
        }
        if let Some(accept) = &self.custom_options.accept {
            tag.attr("accept", accept.join(","));
        }

        write!(f, "{}", tag.render())
    }
}

impl HtmlSafe for FileField {}

/// A representation of an uploaded file stored in memory.
///
/// This struct holds the contents of an uploaded file along with its metadata
/// (filename and content type). It's used to store file uploads that are
/// processed through a form.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InMemoryUploadedFile {
    filename: Option<String>,
    content_type: Option<String>,
    content: Bytes,
}

impl AsFormField for InMemoryUploadedFile {
    type Type = FileField;

    fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError> {
        let data = if let Some(value) = &field.data {
            if value.is_empty() {
                Err(FormFieldValidationError::Required)
            } else {
                Ok(value)
            }
        } else {
            Err(FormFieldValidationError::Required)
        }?;

        Ok(Self {
            filename: field.filename.clone(),
            content_type: field.content_type.clone(),
            content: data.clone(),
        })
    }

    fn to_field_value(&self) -> String {
        String::new()
    }
}

impl InMemoryUploadedFile {
    /// Get the filename of the uploaded file.
    #[must_use]
    pub fn filename(&self) -> Option<&str> {
        self.filename.as_deref()
    }

    /// Get the content (MIME) type of the uploaded file.
    #[must_use]
    pub fn content_type(&self) -> Option<&str> {
        self.content_type.as_deref()
    }

    /// Get the content of the uploaded file.
    #[must_use]
    pub fn content(&self) -> &Bytes {
        &self.content
    }
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use futures_util::stream::once;
    use multer::Multipart;

    use super::*;
    use crate::form::{FormField, FormFieldOptions, FormFieldValue};

    #[test]
    fn file_field_render() {
        let field = FileField::with_options(
            FormFieldOptions {
                id: "test".to_owned(),
                name: "test".to_owned(),
                required: true,
            },
            FileFieldOptions {
                accept: Some(vec!["image/*".to_string(), ".pdf".to_string()]),
            },
        );

        let html = field.to_string();

        assert!(html.contains("type=\"file\""));
        assert!(html.contains("required"));
        assert!(html.contains("accept=\"image/*,.pdf\""));
    }

    #[test]
    fn file_field_render_no_accept() {
        let field = FileField::with_options(
            FormFieldOptions {
                id: "test".to_owned(),
                name: "test".to_owned(),
                required: true,
            },
            FileFieldOptions { accept: None },
        );

        let html = field.to_string();

        assert!(html.contains("type=\"file\""));
        assert!(html.contains("required"));
        assert!(!html.contains("accept="));
    }

    #[cot::test]
    async fn file_field_clean_value() {
        let mut field = FileField::with_options(
            FormFieldOptions {
                id: "test".to_owned(),
                name: "test".to_owned(),
                required: true,
            },
            FileFieldOptions { accept: None },
        );

        let boundary = "boundary";
        let body = format!(
            "--{boundary}\r\n\
            Content-Disposition: form-data; name=\"test\"; filename=\"test.txt\"\r\n\
            Content-Type: text/plain\r\n\
            \r\n\
            test content\r\n\
            --{boundary}--\r\n"
        );

        let stream = once(async move { Ok::<_, std::io::Error>(Bytes::from(body)) });
        let mut multipart = Multipart::new(stream, boundary);

        let field_value = multipart.next_field().await.unwrap().unwrap();
        let multipart = FormFieldValue::new_multipart(field_value);

        field.set_value(multipart).await.unwrap();
        let value = InMemoryUploadedFile::clean_value(&field).unwrap();

        assert_eq!(value.filename(), Some("test.txt"));
        assert_eq!(value.content_type(), Some("text/plain"));
        assert_eq!(value.content(), &bytes::Bytes::from("test content"));
    }

    #[cot::test]
    async fn file_field_clean_required() {
        let field = FileField::with_options(
            FormFieldOptions {
                id: "test".to_owned(),
                name: "test".to_owned(),
                required: true,
            },
            FileFieldOptions { accept: None },
        );

        let value = InMemoryUploadedFile::clean_value(&field);
        assert_eq!(value, Err(FormFieldValidationError::Required));
    }

    #[test]
    fn in_memory_uploaded_file() {
        let file = InMemoryUploadedFile {
            filename: Some("test.txt".to_string()),
            content_type: Some("text/plain".to_string()),
            content: bytes::Bytes::from("test content"),
        };

        assert_eq!(file.filename(), Some("test.txt"));
        assert_eq!(file.content_type(), Some("text/plain"));
        assert_eq!(file.content(), &bytes::Bytes::from("test content"));
    }

    #[test]
    fn in_memory_uploaded_file_no_metadata() {
        let file = InMemoryUploadedFile {
            filename: None,
            content_type: None,
            content: bytes::Bytes::from("test content"),
        };

        assert_eq!(file.filename(), None);
        assert_eq!(file.content_type(), None);
        assert_eq!(file.content(), &bytes::Bytes::from("test content"));
    }
}
