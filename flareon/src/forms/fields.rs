use std::borrow::Cow;
use std::num::{
    NonZeroI128, NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI8, NonZeroIsize, NonZeroU128,
    NonZeroU16, NonZeroU32, NonZeroU64, NonZeroU8, NonZeroUsize,
};

use crate::forms::{AsFormField, FormField, FormFieldOptions, FormFieldValidationError};
use crate::{Html, Render};

macro_rules! impl_form_field {
    ($field_type_name:ident, $field_options_type_name:ident, $purpose:literal $(, $generic_param:ident $(: $generic_param_bound:ident $(+ $generic_param_bound_more:ident)*)? )?) => {
        #[derive(Debug)]
        #[doc = concat!("A form field for ", $purpose, ".")]
        pub struct $field_type_name $(<$generic_param>)? {
            options: FormFieldOptions,
            custom_options: $field_options_type_name $(<$generic_param>)?,
            value: Option<String>,
        }

        impl $(<$generic_param $(: $generic_param_bound $(+ $generic_param_bound_more)* )?>)? FormField for $field_type_name $(<$generic_param>)? {
            type CustomOptions = $field_options_type_name $(<$generic_param>)?;

            fn with_options(
                options: FormFieldOptions,
                custom_options: Self::CustomOptions,
            ) -> Self {
                Self {
                    options,
                    custom_options,
                    value: None,
                }
            }

            fn options(&self) -> &FormFieldOptions {
                &self.options
            }

            fn value(&self) -> Option<&str> {
                self.value.as_deref()
            }

            fn set_value(&mut self, value: Cow<str>) {
                self.value = Some(value.into_owned());
            }
        }
    };
}

impl_form_field!(StringField, StringFieldOptions, "a string");

/// Custom options for a `CharField`.
#[derive(Debug, Default, Copy, Clone)]
pub struct StringFieldOptions {
    /// The maximum length of the field. Used to set the `maxlength` attribute
    /// in the HTML input element.
    pub max_length: Option<u32>,
}

impl Render for StringField {
    fn render(&self) -> Html {
        let mut tag = HtmlTag::input("text");
        tag.attr("name", self.id());
        if self.options.required {
            tag.bool_attr("required");
        }
        if let Some(max_length) = self.custom_options.max_length {
            tag.attr("maxlength", &max_length.to_string());
        }
        tag.render()
    }
}

impl AsFormField for String {
    type Type = StringField;

    fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError> {
        let value = check_required(field)?;

        if let Some(max_length) = field.custom_options.max_length {
            if value.len() as u32 > max_length {
                return Err(FormFieldValidationError::maximum_length_exceeded(
                    max_length,
                ));
            }
        }
        Ok(value.to_owned())
    }
}

impl_form_field!(IntegerField, IntegerFieldOptions, "an integer", T: Integer);

/// Custom options for a `IntegerField`.
#[derive(Debug, Copy, Clone)]
pub struct IntegerFieldOptions<T> {
    pub min: Option<T>,
    pub max: Option<T>,
}

impl<T: Integer> Default for IntegerFieldOptions<T> {
    fn default() -> Self {
        Self {
            min: T::MIN,
            max: T::MAX,
        }
    }
}

impl<T: Integer> Render for IntegerField<T> {
    fn render(&self) -> Html {
        let mut tag = HtmlTag::input("number");
        tag.attr("name", self.id());
        if self.options.required {
            tag.bool_attr("required");
        }
        if let Some(min) = &self.custom_options.min {
            tag.attr("min", &min.to_string());
        }
        if let Some(max) = &self.custom_options.max {
            tag.attr("max", &max.to_string());
        }
        tag.render()
    }
}

/// A trait for numerical types that optionally have minimum and maximum values.
pub trait Integer: Sized + ToString {
    const MIN: Option<Self>;
    const MAX: Option<Self>;
}

macro_rules! impl_integer {
    ($type:ty) => {
        impl Integer for $type {
            const MAX: Option<Self> = Some(Self::MAX);
            const MIN: Option<Self> = Some(Self::MIN);
        }
    };
}

impl_integer!(i8);
impl_integer!(i16);
impl_integer!(i32);
impl_integer!(i64);
impl_integer!(i128);
impl_integer!(isize);
impl_integer!(u8);
impl_integer!(u16);
impl_integer!(u32);
impl_integer!(u64);
impl_integer!(u128);
impl_integer!(usize);
impl_integer!(NonZeroI8);
impl_integer!(NonZeroI16);
impl_integer!(NonZeroI32);
impl_integer!(NonZeroI64);
impl_integer!(NonZeroI128);
impl_integer!(NonZeroIsize);
impl_integer!(NonZeroU8);
impl_integer!(NonZeroU16);
impl_integer!(NonZeroU32);
impl_integer!(NonZeroU64);
impl_integer!(NonZeroU128);
impl_integer!(NonZeroUsize);

macro_rules! impl_integer_as_form_field {
    ($type:ty) => {
        impl AsFormField for $type {
            type Type = IntegerField<$type>;

            fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError> {
                if let Some(value) = &field.value {
                    Ok(value
                        .parse()
                        .map_err(|_| FormFieldValidationError::invalid_value(value))?)
                } else {
                    Err(FormFieldValidationError::Required)
                }
            }
        }
    };
}

impl_integer_as_form_field!(i8);
impl_integer_as_form_field!(i16);
impl_integer_as_form_field!(i32);
impl_integer_as_form_field!(i64);
impl_integer_as_form_field!(i128);
impl_integer_as_form_field!(isize);
impl_integer_as_form_field!(u8);
impl_integer_as_form_field!(u16);
impl_integer_as_form_field!(u32);
impl_integer_as_form_field!(u64);
impl_integer_as_form_field!(u128);
impl_integer_as_form_field!(usize);
impl_integer_as_form_field!(NonZeroI8);
impl_integer_as_form_field!(NonZeroI16);
impl_integer_as_form_field!(NonZeroI32);
impl_integer_as_form_field!(NonZeroI64);
impl_integer_as_form_field!(NonZeroI128);
impl_integer_as_form_field!(NonZeroIsize);
impl_integer_as_form_field!(NonZeroU8);
impl_integer_as_form_field!(NonZeroU16);
impl_integer_as_form_field!(NonZeroU32);
impl_integer_as_form_field!(NonZeroU64);
impl_integer_as_form_field!(NonZeroU128);
impl_integer_as_form_field!(NonZeroUsize);

impl_form_field!(BoolField, BoolFieldOptions, "a boolean");

/// Custom options for a `BoolField`.
#[derive(Debug, Default, Copy, Clone)]
pub struct BoolFieldOptions {
    /// The maximum length of the field. Used to set the `maxlength` attribute
    /// in the HTML input element.
    pub must_be_true: Option<bool>,
}

impl Render for BoolField {
    fn render(&self) -> Html {
        // Web browsers don't send anything when a checkbox is unchecked, so we
        // need to add a hidden input to send a "false" value.
        let mut tag = HtmlTag::input("hidden");
        tag.attr("name", self.id());
        tag.attr("value", "0");
        let hidden = tag.render();

        let mut tag = HtmlTag::input("checkbox");
        tag.attr("name", self.id());
        tag.attr("value", "1");
        let checkbox = tag.render();

        format!("{}{}", hidden.as_str(), checkbox.as_str()).into()
    }
}

/// Implementation of `AsFormField` for `bool`.
///
/// This implementation converts the string values "true", "on", and "1" to
/// `true`, and "false", "  off", and "0" to `false`. It returns an error if the
/// value is not one of these strings. If the field is required to be `true` by
/// the field's options, it will return an error if the value is `false`.
impl AsFormField for bool {
    type Type = BoolField;

    fn new_field(
        mut options: FormFieldOptions,
        custom_options: <Self::Type as FormField>::CustomOptions,
    ) -> Self::Type {
        options.required = false;
        Self::Type::with_options(options, custom_options)
    }

    fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError> {
        let value = check_required(field)?;
        let value = if ["true", "on", "1"].contains(&value) {
            true
        } else if ["false", "off", "0"].contains(&value) {
            false
        } else {
            return Err(FormFieldValidationError::invalid_value(value));
        };

        if field.custom_options.must_be_true.unwrap_or(false) && !value {
            return Err(FormFieldValidationError::BooleanRequiredToBeTrue);
        }
        Ok(value.to_owned())
    }
}

impl<T: AsFormField> AsFormField for Option<T> {
    type Type = T::Type;

    fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError> {
        let value = T::clean_value(field);
        match value {
            Ok(value) => Ok(Some(value)),
            Err(FormFieldValidationError::Required) => Ok(None),
            Err(error) => Err(error),
        }
    }
}

fn check_required<T: FormField>(field: &T) -> Result<&str, FormFieldValidationError> {
    if let Some(value) = field.value() {
        Ok(value)
    } else {
        Err(FormFieldValidationError::Required)
    }
}

/// A helper struct for rendering HTML tags.
#[derive(Debug)]
struct HtmlTag {
    tag: String,
    attributes: Vec<(String, String)>,
    bool_attributes: Vec<String>,
}

impl HtmlTag {
    #[must_use]
    fn new(tag: &str) -> Self {
        Self {
            tag: tag.to_string(),
            attributes: Vec::new(),
            bool_attributes: Vec::new(),
        }
    }

    #[must_use]
    fn input(input_type: &str) -> Self {
        let mut input = Self::new("input");
        input.attr("type", input_type);
        input
    }

    fn attr(&mut self, key: &str, value: &str) -> &mut Self {
        assert!(
            !self.attributes.iter().any(|(k, _)| k == key),
            "Attribute already exists: {key}"
        );
        self.attributes.push((key.to_string(), value.to_string()));
        self
    }

    fn bool_attr(&mut self, key: &str) -> &mut Self {
        self.bool_attributes.push(key.to_string());
        self
    }

    #[must_use]
    fn render(&self) -> Html {
        let mut result = format!("<{}", self.tag);

        for (key, value) in &self.attributes {
            result.push_str(&format!(" {key}=\"{value}\""));
        }
        for key in &self.bool_attributes {
            result.push_str(&format!(" {key}"));
        }

        result.push_str(" />");
        result.into()
    }
}
