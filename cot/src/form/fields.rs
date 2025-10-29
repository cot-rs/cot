mod attrs;
mod chrono;
mod files;
mod select;

use std::fmt::{Debug, Display, Formatter};
use std::num::{
    NonZeroI8, NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI128, NonZeroIsize, NonZeroU8,
    NonZeroU16, NonZeroU32, NonZeroU64, NonZeroU128, NonZeroUsize,
};

use askama::filters::HtmlSafe;
pub use attrs::{AutoCapitalize, AutoComplete, Dir, List, Step};
pub use chrono::{
    DateField, DateFieldOptions, DateTimeField, DateTimeFieldOptions, DateTimeWithTimezoneField,
    DateTimeWithTimezoneFieldOptions, TimeField, TimeFieldOptions,
};
pub use files::{FileField, FileFieldOptions, InMemoryUploadedFile};
pub(crate) use select::check_required_multiple;
pub use select::{
    SelectChoice, SelectField, SelectFieldOptions, SelectMultipleField, SelectMultipleFieldOptions,
};

use crate::auth::PasswordHash;
use crate::common_types::{Email, Password, Url};
#[cfg(feature = "db")]
use crate::db::{Auto, ForeignKey, LimitedString, Model};
use crate::form::{AsFormField, FormField, FormFieldOptions, FormFieldValidationError};
use crate::html::HtmlTag;

macro_rules! impl_form_field {
    ($field_type_name:ident, $field_options_type_name:ident, $purpose:literal $(, $generic_param:ident $(: $generic_param_bound:ident $(+ $generic_param_bound_more:ident)*)?)?) => {
        #[derive(Debug)]
        #[doc = concat!("A form field for ", $purpose, ".")]
        pub struct $field_type_name $(<$generic_param>)? {
            options: $crate::form::FormFieldOptions,
            custom_options: $field_options_type_name $(<$generic_param>)?,
            value: Option<String>,
        }

        impl $(<$generic_param $(: $generic_param_bound $(+ $generic_param_bound_more)* )?>)? $crate::form::FormField for $field_type_name $(<$generic_param>)? {
            type CustomOptions = $field_options_type_name $(<$generic_param>)?;

            fn with_options(
                options: $crate::form::FormFieldOptions,
                custom_options: Self::CustomOptions,
            ) -> Self {
                Self {
                    options,
                    custom_options,
                    value: None,
                }
            }

            fn options(&self) -> &$crate::form::FormFieldOptions {
                &self.options
            }

            fn value(&self) -> Option<&str> {
                self.value.as_deref()
            }

            async fn set_value(&mut self, field: $crate::form::FormFieldValue<'_>) -> std::result::Result<(), $crate::form::FormFieldValueError> {
                self.value = Some(field.into_text().await?);
                Ok(())
            }
        }
    };
}
pub(crate) use impl_form_field;

impl_form_field!(StringField, StringFieldOptions, "a string");

/// Custom options for a [`StringField`].
#[derive(Debug, Default, Clone)]
pub struct StringFieldOptions {
    /// The maximum length of the field. Used to set the `maxlength` attribute
    /// in the HTML input element.
    pub max_length: Option<u32>,
    /// The minimum length of the field. Used to set the `minlength` attribute
    /// in the HTML input element.
    pub min_length: Option<u32>,
    /// The size of the field. Used to set the `size` attribute in the HTML
    /// input element.
    pub size: Option<u32>,
    /// Corresponds to  the [`AutoCapitalize`] attribute in the HTML input
    /// element.
    pub autocapitalize: Option<AutoCapitalize>,
    /// Corresponds to the [`AutoComplete`] attribute in the HTML input element.
    pub autocomplete: Option<AutoComplete>,
    /// The direction of the text input, which can be set to `ltr`
    /// (left-to-right) or `rtl` (right-to-left). This corresponds to the
    /// [`Dir`] attribute in the HTML input element.
    pub dir: Option<Dir>,
    /// The [`dirname`] attribute in the HTML input element, which is used to
    /// specify the direction of the text input.
    ///
    /// [`dirname`]: https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/input#dirname
    pub dirname: Option<String>,
    /// A [`List`] of options for the `datalist` element, which can be used to
    /// provide predefined options for the input.
    pub list: Option<List>,
    /// The placeholder text for the input field, which is displayed when the
    /// field is empty.
    pub placeholder: Option<String>,
    /// If `true`, the field is read-only and cannot be modified by the user.
    pub readonly: Option<bool>,
}

impl Display for StringField {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut tag = HtmlTag::input("text");
        tag.attr("name", self.id());
        tag.attr("id", self.id());
        if self.options.required {
            tag.bool_attr("required");
        }
        if let Some(max_length) = self.custom_options.max_length {
            tag.attr("maxlength", max_length.to_string());
        }
        if let Some(min_length) = self.custom_options.min_length {
            tag.attr("minlength", min_length.to_string());
        }

        if let Some(placeholder) = &self.custom_options.placeholder {
            tag.attr("placeholder", placeholder);
        }
        if let Some(readonly) = self.custom_options.readonly {
            if readonly {
                tag.bool_attr("readonly");
            }
        }
        if let Some(autocomplete) = &self.custom_options.autocomplete {
            tag.attr("autocomplete", autocomplete.to_string());
        }

        if let Some(size) = self.custom_options.size {
            tag.attr("size", size.to_string());
        }

        if let Some(dir) = &self.custom_options.dir {
            tag.attr("dir", dir.as_str());
        }

        if let Some(dirname) = &self.custom_options.dirname {
            tag.attr("dirname", dirname);
        }

        if let Some(list) = &self.custom_options.list {
            let list_id = format!("__{}_datalist", self.id());
            tag.attr("list", &list_id);

            let data_list = HtmlTag::data_list(list.clone(), &list_id);
            tag.push_tag(data_list);
        }

        if let Some(value) = &self.value {
            tag.attr("value", value);
        }

        write!(f, "{}", tag.render())
    }
}

impl HtmlSafe for StringField {}

impl AsFormField for String {
    type Type = StringField;

    fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError> {
        let value = check_required(field)?;

        if let Some(max_length) = field.custom_options.max_length {
            if value.len() > max_length as usize {
                return Err(FormFieldValidationError::maximum_length_exceeded(
                    max_length,
                ));
            }
        }
        Ok(value.to_owned())
    }

    fn to_field_value(&self) -> String {
        self.to_owned()
    }
}

#[cfg(feature = "db")]
impl<const LEN: u32> AsFormField for LimitedString<LEN> {
    type Type = StringField;

    fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError> {
        let value = check_required(field)?;

        if value.len() > LEN as usize {
            return Err(FormFieldValidationError::maximum_length_exceeded(LEN));
        }
        Ok(LimitedString::new(value.to_owned()).expect("length has already been checked"))
    }

    fn to_field_value(&self) -> String {
        self.to_string()
    }
}

impl_form_field!(PasswordField, PasswordFieldOptions, "a password");

/// Custom options for a [`PasswordField`].
#[derive(Debug, Default, Clone)]
pub struct PasswordFieldOptions {
    /// The maximum length of the field. Used to set the `maxlength` attribute
    /// in the HTML input element.
    pub max_length: Option<u32>,
    /// The minimum length of the field. Used to set the `minlength` attribute
    /// in the HTML input element.
    pub min_length: Option<u32>,
    /// The size of the field. Used to set the [`size`] attribute in the HTML
    /// input element.
    ///
    /// [`size`]: https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/input#size
    pub size: Option<u32>,
    /// Corresponds to the [`AutoComplete`] attribute in the HTML input element.
    pub autocomplete: Option<AutoComplete>,
    /// The placeholder text for the input field, which is displayed when the
    /// field is empty.
    pub placeholder: Option<String>,
    /// If `true`, the field is read-only and cannot be modified by the user.
    pub readonly: Option<bool>,
}

impl Display for PasswordField {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut tag = HtmlTag::input("password");
        tag.attr("name", self.id());
        tag.attr("id", self.id());
        if self.options.required {
            tag.bool_attr("required");
        }
        if let Some(max_length) = self.custom_options.max_length {
            tag.attr("maxlength", max_length.to_string());
        }

        if let Some(min_length) = self.custom_options.min_length {
            tag.attr("minlength", min_length.to_string());
        }
        if let Some(placeholder) = &self.custom_options.placeholder {
            tag.attr("placeholder", placeholder);
        }
        if let Some(readonly) = self.custom_options.readonly {
            if readonly {
                tag.bool_attr("readonly");
            }
        }
        if let Some(autocomplete) = &self.custom_options.autocomplete {
            tag.attr("autocomplete", autocomplete.to_string());
        }

        if let Some(size) = self.custom_options.size {
            tag.attr("size", size.to_string());
        }

        // we don't set the value attribute for password fields
        // to avoid leaking the password in the HTML

        write!(f, "{}", tag.render())
    }
}

impl HtmlSafe for PasswordField {}

impl AsFormField for Password {
    type Type = PasswordField;

    fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError> {
        let value = check_required(field)?;

        if let Some(max_length) = field.custom_options.max_length {
            if value.len() > max_length as usize {
                return Err(FormFieldValidationError::maximum_length_exceeded(
                    max_length,
                ));
            }
        }

        Ok(Password::new(value))
    }

    fn to_field_value(&self) -> String {
        self.as_str().to_owned()
    }
}

impl AsFormField for PasswordHash {
    type Type = PasswordField;

    fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError> {
        let value = check_required(field)?;

        if let Some(max_length) = field.custom_options.max_length {
            if value.len() > max_length as usize {
                return Err(FormFieldValidationError::maximum_length_exceeded(
                    max_length,
                ));
            }
        }

        Ok(PasswordHash::from_password(&Password::new(value)))
    }

    fn to_field_value(&self) -> String {
        // cannot return the original password
        String::new()
    }
}

impl_form_field!(EmailField, EmailFieldOptions, "an email");

/// Custom options for [`EmailField`]
#[derive(Debug, Default, Clone)]
pub struct EmailFieldOptions {
    /// The maximum length of the field used to set the `maxlength` attribute
    /// in the HTML input element.
    pub max_length: Option<u32>,
    /// The minimum length of the field used to set the `minlength` attribute
    /// in the HTML input element.
    pub min_length: Option<u32>,
    /// The size of the field used to set the [`size`] attribute in the HTML
    /// input element.
    ///
    /// [`size`]: https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/input#size
    pub size: Option<u32>,
    /// Corresponds to the [`AutoCapitalize`] attribute in the HTML input
    /// element.
    pub autocomplete: Option<AutoComplete>,
    /// The direction of the text input, which can be set to `ltr`
    /// (left-to-right) or `rtl` (right-to-left). This corresponds to the
    /// [`Dir`] attribute in the HTML input element.
    pub dir: Option<Dir>,
    /// The [`dirname`] attribute in the HTML input element, which is used to
    /// specify the direction of the text input.
    ///
    /// [`dirname`]: https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/input#dirname
    pub dirname: Option<String>,
    /// A [`List`] of options for the `datalist` element, which can be used to
    /// provide predefined options for the input.
    pub list: Option<List>,
    /// The placeholder text for the input field, which is displayed when the
    /// field is empty.
    pub placeholder: Option<String>,
    /// If `true`, the field is read-only and cannot be modified by the user.
    pub readonly: Option<bool>,
}

impl Display for EmailField {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut tag = HtmlTag::input("email");
        let mut data_list: Option<HtmlTag> = None;

        tag.attr("name", self.id());
        tag.attr("id", self.id());
        if self.options.required {
            tag.bool_attr("required");
        }
        if let Some(max_length) = self.custom_options.max_length {
            tag.attr("maxlength", max_length.to_string());
        }
        if let Some(min_length) = self.custom_options.min_length {
            tag.attr("minlength", min_length.to_string());
        }
        if let Some(placeholder) = &self.custom_options.placeholder {
            tag.attr("placeholder", placeholder);
        }
        if let Some(readonly) = self.custom_options.readonly {
            if readonly {
                tag.bool_attr("readonly");
            }
        }
        if let Some(autocomplete) = &self.custom_options.autocomplete {
            tag.attr("autocomplete", autocomplete.to_string());
        }

        if let Some(size) = self.custom_options.size {
            tag.attr("size", size.to_string());
        }

        if let Some(dir) = &self.custom_options.dir {
            tag.attr("dir", dir.as_str());
        }

        if let Some(dirname) = &self.custom_options.dirname {
            tag.attr("dirname", dirname);
        }

        if let Some(list) = &self.custom_options.list {
            let list_id = format!("__{}_datalist", self.id());
            tag.attr("list", &list_id);

            data_list = Some(HtmlTag::data_list(list.clone(), &list_id));
        }

        if let Some(value) = &self.value {
            tag.attr("value", value);
        }

        if let Some(data_list) = data_list {
            let mut wrapper = HtmlTag::new("div");
            wrapper
                .attr("id", format!("__{}_datalist_wrapper", self.id()))
                .push_tag(tag)
                .push_tag(data_list);
            return write!(f, "{}", wrapper.render());
        }
        write!(f, "{}", tag.render())
    }
}

impl AsFormField for Email {
    type Type = EmailField;

    fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError>
    where
        Self: Sized,
    {
        let value = check_required(field)?;
        let opts = &field.custom_options;

        if let (Some(min), Some(max)) = (opts.min_length, opts.max_length) {
            if min > max {
                return Err(FormFieldValidationError::from_string(format!(
                    "min_length ({min}) exceeds max_length ({max})"
                )));
            }
        }

        if let Some(min) = opts.min_length {
            if value.len() < min as usize {
                return Err(FormFieldValidationError::minimum_length_not_met(min));
            }
        }

        if let Some(max) = opts.max_length {
            if value.len() > max as usize {
                return Err(FormFieldValidationError::maximum_length_exceeded(max));
            }
        }

        Ok(value.parse()?)
    }

    fn to_field_value(&self) -> String {
        self.as_str().to_owned()
    }
}

impl HtmlSafe for EmailField {}

impl_form_field!(IntegerField, IntegerFieldOptions, "an integer", T: Integer + Display);

/// Custom options for a [`IntegerField`].
#[derive(Debug, Clone)]
pub struct IntegerFieldOptions<T> {
    /// The minimum value of the field. Used to set the `min` attribute in the
    /// HTML input element.
    pub min: Option<T>,
    /// The maximum value of the field. Used to set the `max` attribute in the
    /// HTML input element.
    pub max: Option<T>,
    /// The placeholder text for the input field, which is displayed when the
    /// field is empty.
    pub placeholder: Option<String>,
    /// If `true`, the field is read-only and cannot be modified by the user.
    pub readonly: Option<bool>,
    /// The step size for the field. Used to set the [`Step`] attribute in the
    /// HTML input element.
    pub step: Option<Step<T>>,
}

impl<T: Integer> Default for IntegerFieldOptions<T> {
    fn default() -> Self {
        Self {
            min: T::MIN,
            max: T::MAX,
            placeholder: None,
            readonly: None,
            step: None,
        }
    }
}

impl<T: Integer + Display> Display for IntegerField<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut tag = HtmlTag::input("number");
        tag.attr("name", self.id());
        tag.attr("id", self.id());
        if self.options.required {
            tag.bool_attr("required");
        }
        if let Some(min) = &self.custom_options.min {
            tag.attr("min", min.to_string());
        }
        if let Some(max) = &self.custom_options.max {
            tag.attr("max", max.to_string());
        }

        if let Some(placeholder) = &self.custom_options.placeholder {
            tag.attr("placeholder", placeholder);
        }
        if let Some(readonly) = self.custom_options.readonly {
            if readonly {
                tag.bool_attr("readonly");
            }
        }
        if let Some(step) = &self.custom_options.step {
            tag.attr("step", step.to_string());
        }

        if let Some(value) = &self.value {
            tag.attr("value", value);
        }

        write!(f, "{}", tag.render())
    }
}

impl<T: Integer + Display> HtmlSafe for IntegerField<T> {}

/// A trait for numerical types that optionally have minimum and maximum values.
///
/// # Examples
///
/// ```
/// use cot::form::fields::Integer;
///
/// assert_eq!(<i8 as Integer>::MIN, Some(-128));
/// assert_eq!(<i8 as Integer>::MAX, Some(127));
/// ```
pub trait Integer: Sized + ToString + Send {
    /// The minimum value of the type.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::form::fields::Integer;
    ///
    /// assert_eq!(<i8 as Integer>::MIN, Some(-128));
    /// ```
    const MIN: Option<Self>;
    /// The maximum value of the type.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::form::fields::Integer;
    ///
    /// assert_eq!(<i8 as Integer>::MAX, Some(127));
    /// ```
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
                let value = check_required(field)?;

                let parsed: $type = value
                    .parse()
                    .map_err(|_| FormFieldValidationError::invalid_value(value))?;

                if let Some(min) = field.custom_options.min {
                    if parsed < min {
                        return Err(FormFieldValidationError::minimum_value_not_met(min));
                    }
                }

                if let Some(max) = field.custom_options.max {
                    if parsed > max {
                        return Err(FormFieldValidationError::maximum_value_exceeded(max));
                    }
                }

                Ok(parsed)
            }

            fn to_field_value(&self) -> String {
                self.to_string()
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

/// Custom options for a [`BoolField`].
#[derive(Debug, Default, Copy, Clone)]
pub struct BoolFieldOptions {
    /// If `true`, the field must be checked to be considered valid.
    pub must_be_true: Option<bool>,
}

impl Display for BoolField {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut bool_input = HtmlTag::input("checkbox");
        bool_input.attr("name", self.id());
        bool_input.attr("id", self.id());
        bool_input.attr("value", "1");

        if self.custom_options.must_be_true.unwrap_or(false) {
            bool_input.bool_attr("required");
            return write!(f, "{}", bool_input.render());
        }

        if let Some(value) = &self.value {
            if value == "1" {
                bool_input.bool_attr("checked");
            }
        }

        // Web browsers don't send anything when a checkbox is unchecked, so we
        // need to add a hidden input to send a "false" value.
        let mut hidden_input = HtmlTag::input("hidden");
        hidden_input.attr("name", self.id());
        hidden_input.attr("value", "0");
        let hidden = hidden_input.render();

        let checkbox = bool_input.render();
        write!(f, "{}{}", hidden.as_str(), checkbox.as_str())
    }
}

impl HtmlSafe for BoolField {}

/// Implementation of `AsFormField` for `bool`.
///
/// This implementation converts the string values "true", "on", and "1" to
/// `true`, and "false", "off", and "0" to `false`. It returns an error if the
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

    fn to_field_value(&self) -> String {
        String::from(if *self { "1" } else { "0" })
    }
}

impl<T: AsFormField> AsFormField for Option<T> {
    type Type = T::Type;

    fn new_field(
        mut options: FormFieldOptions,
        custom_options: <Self::Type as FormField>::CustomOptions,
    ) -> Self::Type {
        options.required = false;
        Self::Type::with_options(options, custom_options)
    }

    fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError> {
        let value = T::clean_value(field);
        match value {
            Ok(value) => Ok(Some(value)),
            Err(FormFieldValidationError::Required) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn to_field_value(&self) -> String {
        match self {
            Some(value) => value.to_field_value(),
            None => String::new(),
        }
    }
}

#[cfg(feature = "db")]
impl<T: AsFormField> AsFormField for Auto<T> {
    type Type = T::Type;

    fn new_field(
        mut options: FormFieldOptions,
        custom_options: <Self::Type as FormField>::CustomOptions,
    ) -> Self::Type {
        options.required = false;
        Self::Type::with_options(options, custom_options)
    }

    fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError>
    where
        Self: Sized,
    {
        let value = T::clean_value(field);
        match value {
            Ok(value) => Ok(Auto::fixed(value)),
            Err(FormFieldValidationError::Required) => Ok(Auto::auto()),
            Err(error) => Err(error),
        }
    }

    fn to_field_value(&self) -> String {
        match self {
            Auto::Fixed(value) => value.to_field_value(),
            Auto::Auto => String::new(),
        }
    }
}

#[cfg(feature = "db")]
impl<T> AsFormField for ForeignKey<T>
where
    T: Model,
    <T as Model>::PrimaryKey: AsFormField,
{
    type Type = <<T as Model>::PrimaryKey as AsFormField>::Type;

    fn new_field(
        options: FormFieldOptions,
        custom_options: <Self::Type as FormField>::CustomOptions,
    ) -> Self::Type {
        Self::Type::with_options(options, custom_options)
    }

    fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError>
    where
        Self: Sized,
    {
        let value = <T as Model>::PrimaryKey::clean_value(field);
        match value {
            Ok(value) => Ok(ForeignKey::PrimaryKey(value)),
            Err(error) => Err(error),
        }
    }

    fn to_field_value(&self) -> String {
        match self {
            ForeignKey::PrimaryKey(primary_key) => primary_key.to_field_value(),
            ForeignKey::Model(model) => model.primary_key().to_field_value(),
        }
    }
}

pub(crate) fn check_required<T: FormField>(field: &T) -> Result<&str, FormFieldValidationError> {
    if let Some(value) = field.value() {
        if value.is_empty() {
            Err(FormFieldValidationError::Required)
        } else {
            Ok(value)
        }
    } else {
        Err(FormFieldValidationError::Required)
    }
}

impl_form_field!(FloatField, FloatFieldOptions, "a float",  T: Float + Display);

/// Custom options for a [`FloatField`].
#[derive(Debug, Clone)]
pub struct FloatFieldOptions<T> {
    /// The minimum value of the field. Used to set the `min` attribute in the
    /// HTML input element.
    pub min: Option<T>,
    /// The maximum value of the field. Used to set the `max` attribute in the
    /// HTML input element.
    pub max: Option<T>,
    /// The placeholder text for the input field, which is displayed when the
    /// field is empty.
    pub placeholder: Option<String>,
    /// If `true`, the field is read-only and cannot be modified by the user.
    pub readonly: Option<bool>,
    /// The step size for the field. Used to set the [`Step`] attribute in the
    /// HTML input element.
    pub step: Option<Step<T>>,
}

impl<T: Float> Default for FloatFieldOptions<T> {
    fn default() -> Self {
        Self {
            min: T::MIN,
            max: T::MAX,
            placeholder: None,
            readonly: None,
            step: None,
        }
    }
}

impl<T: Float + Display> Display for FloatField<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut tag: HtmlTag = HtmlTag::input("number");
        tag.attr("name", self.id());
        tag.attr("id", self.id());
        if self.options.required {
            tag.bool_attr("required");
        }

        if let Some(min) = &self.custom_options.min {
            tag.attr("min", min.to_string());
        }
        if let Some(max) = &self.custom_options.max {
            tag.attr("max", max.to_string());
        }
        if let Some(placeholder) = &self.custom_options.placeholder {
            tag.attr("placeholder", placeholder);
        }
        if let Some(readonly) = self.custom_options.readonly {
            if readonly {
                tag.bool_attr("readonly");
            }
        }
        if let Some(step) = &self.custom_options.step {
            tag.attr("step", step.to_string());
        }
        if let Some(value) = &self.value {
            tag.attr("value", value);
        }

        write!(f, "{}", tag.render())
    }
}

impl<T: Float + Display> HtmlSafe for FloatField<T> {}

/// A trait for types that can be represented as a float.
///
/// This trait is implemented for `f32` and `f64`.
pub trait Float: Sized + ToString + Send {
    /// The minimum value of the type.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::form::fields::Float;
    ///
    /// assert_eq!(<f32 as Float>::MIN, Some(f32::MIN));
    /// ```
    const MIN: Option<Self>;
    /// The maximum value of the type.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::form::fields::Float;
    ///
    /// assert_eq!(<f32 as Float>::MAX, Some(f32::MAX));
    /// ```
    const MAX: Option<Self>;
}

macro_rules! impl_float {
    ($type:ty) => {
        impl Float for $type {
            const MIN: Option<Self> = Some(Self::MIN);
            const MAX: Option<Self> = Some(Self::MAX);
        }
    };
}

impl_float!(f32);
impl_float!(f64);

macro_rules! impl_float_as_form_field {
    ($type:ty) => {
        impl AsFormField for $type {
            type Type = FloatField<$type>;

            fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError> {
                let value = check_required(field)?;
                let parsed: $type = value
                    .parse()
                    .map_err(|_| FormFieldValidationError::invalid_value(value))?;

                if parsed.is_nan() || parsed.is_infinite() {
                    return Err(FormFieldValidationError::from_static(
                        "Cannot have NaN or inf as form input values",
                    ));
                }

                if let Some(min) = field.custom_options.min {
                    if parsed < min {
                        return Err(FormFieldValidationError::minimum_value_not_met(min));
                    }
                }

                if let Some(max) = field.custom_options.max {
                    if parsed > max {
                        return Err(FormFieldValidationError::maximum_value_exceeded(max));
                    }
                }

                Ok(parsed)
            }

            fn to_field_value(&self) -> String {
                self.to_string()
            }
        }
    };
}

impl_float_as_form_field!(f32);
impl_float_as_form_field!(f64);

impl_form_field!(UrlField, UrlFieldOptions, "a URL");

/// Custom options for a [`UrlField`].
#[derive(Debug, Default, Clone)]
pub struct UrlFieldOptions {
    /// The maximum length of the field. Used to set the `maxlength` attribute
    /// in the HTML input element.
    pub max_length: Option<u32>,
    /// The minimum length of the field. Used to set the `minlength` attribute
    /// in the HTML input element.
    pub min_length: Option<u32>,
    /// The size of the field. Used to set the [`size`]attribute in the HTML
    /// input element.
    ///
    /// [`size`]: https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/input#size
    pub size: Option<u32>,
    /// The [`List`] of options for the `datalist` element, which can be used to
    /// provide predefined options for the input.
    pub list: Option<List>,
    /// The direction of the text input, which can be set to `ltr`
    /// (left-to-right) or `rtl` (right-to-left). This corresponds to the
    /// [`Dir`] attribute in the HTML input element.
    pub dir: Option<Dir>,
    /// The [`dirname`] attribute in the HTML input element, which is used to
    /// specify the direction of the text input.
    ///
    /// [`dirname`]: https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/input#dirname
    pub dirname: Option<String>,
    /// The [`AutoComplete`] attribute in the HTML input element, which is used
    /// to specify how the browser should handle autocomplete for the input.
    pub autocomplete: Option<AutoComplete>,
    /// The placeholder text for the input field, which is displayed when the
    /// field is empty.
    pub placeholder: Option<String>,
    /// If `true`, the field is read-only and cannot be modified by the user.
    pub readonly: Option<bool>,
}

impl Display for UrlField {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut tag = HtmlTag::input("url");
        tag.attr("name", self.id());
        tag.attr("id", self.id());
        if self.options.required {
            tag.bool_attr("required");
        }
        if let Some(max_length) = self.custom_options.max_length {
            tag.attr("maxlength", max_length.to_string());
        }
        if let Some(min_length) = self.custom_options.min_length {
            tag.attr("minlength", min_length.to_string());
        }
        if let Some(placeholder) = &self.custom_options.placeholder {
            tag.attr("placeholder", placeholder);
        }
        if let Some(readonly) = self.custom_options.readonly {
            if readonly {
                tag.bool_attr("readonly");
            }
        }
        if let Some(autocomplete) = &self.custom_options.autocomplete {
            tag.attr("autocomplete", autocomplete.to_string());
        }

        if let Some(size) = self.custom_options.size {
            tag.attr("size", size.to_string());
        }

        if let Some(dir) = &self.custom_options.dir {
            tag.attr("dir", dir.as_str());
        }

        if let Some(dirname) = &self.custom_options.dirname {
            tag.attr("dirname", dirname);
        }

        if let Some(list) = &self.custom_options.list {
            let list_id = format!("__{}_datalist", self.id());
            tag.attr("list", &list_id);

            let data_list = HtmlTag::data_list(list.clone(), &list_id);
            tag.push_tag(data_list);
        }
        if let Some(value) = &self.value {
            tag.attr("value", value);
        }

        write!(f, "{}", tag.render())
    }
}

impl HtmlSafe for UrlField {}

impl AsFormField for Url {
    type Type = UrlField;

    fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError>
    where
        Self: Sized,
    {
        let value = check_required(field)?;

        Ok(value.parse()?)
    }

    fn to_field_value(&self) -> String {
        self.as_str().to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::form::FormFieldValue;

    #[test]
    fn string_field_render() {
        let field = StringField::with_options(
            FormFieldOptions {
                id: "test".to_owned(),
                name: "test".to_owned(),
                required: true,
            },
            StringFieldOptions {
                max_length: Some(10),
                min_length: Some(5),
                size: Some(15),
                autocapitalize: Some(AutoCapitalize::Words),
                autocomplete: Some(AutoComplete::Value("foo bar".to_string())),
                dir: Some(Dir::Ltr),
                dirname: Some("dir".to_string()),
                list: Some(List::new(["bar", "baz"])),
                placeholder: Some("Enter text".to_string()),
                readonly: Some(true),
            },
        );
        let html = field.to_string();
        assert!(html.contains("type=\"text\""));
        assert!(html.contains("name=\"test\""));
        assert!(html.contains("id=\"test\""));
        assert!(html.contains("required"));
        assert!(html.contains("maxlength=\"10\""));
        assert!(html.contains("minlength=\"5\""));
        assert!(html.contains("size=\"15\""));
        assert!(html.contains("autocomplete=\"foo bar\""));
        assert!(html.contains("dir=\"ltr\""));
        assert!(html.contains("dirname=\"dir\""));
        assert!(html.contains("placeholder=\"Enter text\""));
        assert!(html.contains("readonly"));
    }

    #[cot::test]
    async fn string_field_clean_value() {
        let mut field = StringField::with_options(
            FormFieldOptions {
                id: "test".to_owned(),
                name: "test".to_owned(),
                required: true,
            },
            StringFieldOptions {
                max_length: Some(10),
                ..Default::default()
            },
        );
        field
            .set_value(FormFieldValue::new_text("test"))
            .await
            .unwrap();
        let value = String::clean_value(&field).unwrap();
        assert_eq!(value, "test");
    }

    #[cot::test]
    async fn string_field_clean_required() {
        let mut field = StringField::with_options(
            FormFieldOptions {
                id: "test".to_owned(),
                name: "test".to_owned(),
                required: true,
            },
            StringFieldOptions {
                max_length: Some(10),
                ..Default::default()
            },
        );
        field.set_value(FormFieldValue::new_text("")).await.unwrap();
        let value = String::clean_value(&field);
        assert_eq!(value, Err(FormFieldValidationError::Required));
    }

    #[test]
    fn password_field_render() {
        let field = PasswordField::with_options(
            FormFieldOptions {
                id: "test".to_owned(),
                name: "test".to_owned(),
                required: true,
            },
            PasswordFieldOptions {
                max_length: Some(10),
                min_length: Some(5),
                size: Some(15),
                autocomplete: Some(AutoComplete::Value("foo bar".to_string())),
                placeholder: Some("Enter password".to_string()),
                readonly: Some(false),
            },
        );
        let html = field.to_string();
        assert!(html.contains("type=\"password\""));
        assert!(html.contains("required"));
        assert!(html.contains("maxlength=\"10\""));
        assert!(html.contains("minlength=\"5\""));
        assert!(html.contains("autocomplete=\"foo bar\""));
        assert!(html.contains("placeholder=\"Enter password\""));
    }

    #[cot::test]
    async fn password_field_clean_value() {
        let mut field = PasswordField::with_options(
            FormFieldOptions {
                id: "test".to_owned(),
                name: "test".to_owned(),
                required: true,
            },
            PasswordFieldOptions {
                max_length: Some(10),
                ..Default::default()
            },
        );
        field
            .set_value(FormFieldValue::new_text("password"))
            .await
            .unwrap();
        let value = Password::clean_value(&field).unwrap();
        assert_eq!(value.as_str(), "password");
    }

    #[test]
    fn email_field_render() {
        let field = EmailField::with_options(
            FormFieldOptions {
                id: "test_id".to_owned(),
                name: "test_name".to_owned(),
                required: true,
            },
            EmailFieldOptions {
                max_length: Some(10),
                min_length: Some(5),
                size: Some(15),
                autocomplete: Some(AutoComplete::Value("foo bar".to_string())),
                dir: Some(Dir::Ltr),
                dirname: Some("dir".to_string()),
                list: Some(List::new(["foo@example.com", "baz@example.com"])),
                placeholder: Some("Enter text".to_string()),
                readonly: Some(true),
            },
        );

        let html = field.to_string();

        assert!(html.contains("type=\"email\""));
        assert!(html.contains("required"));
        assert!(html.contains("minlength=\"5\""));
        assert!(html.contains("maxlength=\"10\""));
        assert!(html.contains("name=\"test_id\""));
        assert!(html.contains("id=\"test_id\""));
        assert!(html.contains("placeholder=\"Enter text\""));
        assert!(html.contains("readonly"));
        assert!(html.contains("autocomplete=\"foo bar\""));
        assert!(html.contains("dir=\"ltr\""));
        assert!(html.contains("dirname=\"dir\""));
        assert!(html.contains("list=\"__test_id_datalist\""));
        assert!(html.contains(r#"<datalist id="__test_id_datalist"><option value="foo@example.com">foo@example.com</option><option value="baz@example.com">baz@example.com</option></datalist>"#));
    }

    #[cot::test]
    async fn email_field_clean_valid() {
        let mut field = EmailField::with_options(
            FormFieldOptions {
                id: "email_test".to_owned(),
                name: "email_test".to_owned(),
                required: true,
            },
            EmailFieldOptions {
                min_length: Some(10),
                max_length: Some(50),
                ..Default::default()
            },
        );

        field
            .set_value(FormFieldValue::new_text("user@example.com"))
            .await
            .unwrap();
        let email = Email::clean_value(&field).unwrap();

        assert_eq!(email.as_str(), "user@example.com");
    }

    #[cot::test]
    async fn email_field_clean_invalid_format() {
        let mut field = EmailField::with_options(
            FormFieldOptions {
                id: "email_test".to_owned(),
                name: "email_test".to_owned(),
                required: true,
            },
            EmailFieldOptions {
                min_length: Some(10),
                max_length: Some(50),
                ..Default::default()
            },
        );

        field
            .set_value(FormFieldValue::new_text("invalid-email"))
            .await
            .unwrap();
        let result = Email::clean_value(&field);

        assert!(result.is_err());
    }

    #[cot::test]
    async fn email_field_clean_exceeds_max_length() {
        let mut field = EmailField::with_options(
            FormFieldOptions {
                id: "email_test".to_owned(),
                name: "email_test".to_owned(),
                required: true,
            },
            EmailFieldOptions {
                min_length: Some(5),
                max_length: Some(10),
                ..Default::default()
            },
        );

        field
            .set_value(FormFieldValue::new_text("averylongemail@example.com"))
            .await
            .unwrap();
        let result = Email::clean_value(&field);

        assert!(matches!(
            result,
            Err(FormFieldValidationError::MaximumLengthExceeded { max_length: _ })
        ));
    }

    #[cot::test]
    async fn email_field_clean_below_min_length() {
        let mut field = EmailField::with_options(
            FormFieldOptions {
                id: "email_test".to_owned(),
                name: "email_test".to_owned(),
                required: true,
            },
            EmailFieldOptions {
                min_length: Some(5),
                max_length: Some(10),
                ..Default::default()
            },
        );

        field
            .set_value(FormFieldValue::new_text("cot"))
            .await
            .unwrap();
        let result = Email::clean_value(&field);

        assert!(matches!(
            result,
            Err(FormFieldValidationError::MinimumLengthNotMet { min_length: _ })
        ));
    }

    #[cot::test]
    async fn email_field_clean_invalid_length_options() {
        let mut field = EmailField::with_options(
            FormFieldOptions {
                id: "email_test".to_owned(),
                name: "email_test".to_owned(),
                required: true,
            },
            EmailFieldOptions {
                min_length: Some(50),
                max_length: Some(10),
                ..Default::default()
            },
        );

        field
            .set_value(FormFieldValue::new_text("user@example.com"))
            .await
            .unwrap();
        let result = Email::clean_value(&field);

        assert!(result.is_err());
        if let Err(err) = result {
            let msg = err.to_string();
            assert!(msg.contains("min_length") && msg.contains("exceeds max_length"));
        }
    }

    #[test]
    fn integer_field_render() {
        let field = IntegerField::<i32>::with_options(
            FormFieldOptions {
                id: "test".to_owned(),
                name: "test".to_owned(),
                required: true,
            },
            IntegerFieldOptions {
                min: Some(1),
                max: Some(10),
                placeholder: Some("Enter text".to_string()),
                readonly: Some(false),
                step: Some(Step::Value(10)),
            },
        );
        let html = field.to_string();

        assert!(html.contains("type=\"number\""));
        assert!(html.contains("required"));
        assert!(html.contains("min=\"1\""));
        assert!(html.contains("max=\"10\""));
        assert!(html.contains("step=\"10\""));
        assert!(html.contains("placeholder=\"Enter text\""));
        assert!(html.contains("name=\"test\""));
        assert!(html.contains("id=\"test\""));
        assert!(!html.contains("readonly"));
    }

    #[cot::test]
    async fn integer_field_clean_value() {
        let mut field = IntegerField::<i32>::with_options(
            FormFieldOptions {
                id: "test".to_owned(),
                name: "test".to_owned(),
                required: true,
            },
            IntegerFieldOptions {
                min: Some(1),
                max: Some(10),
                ..Default::default()
            },
        );
        field
            .set_value(FormFieldValue::new_text("5"))
            .await
            .unwrap();
        let value = i32::clean_value(&field).unwrap();
        assert_eq!(value, 5);
    }

    #[cot::test]
    async fn integer_field_clean_value_below_min_value() {
        let mut field = IntegerField::<i32>::with_options(
            FormFieldOptions {
                id: "test".to_owned(),
                name: "test".to_owned(),
                required: true,
            },
            IntegerFieldOptions {
                min: Some(10),
                max: Some(50),
                ..Default::default()
            },
        );
        field
            .set_value(FormFieldValue::new_text("5"))
            .await
            .unwrap();
        let value = i32::clean_value(&field);
        assert!(matches!(
            value,
            Err(FormFieldValidationError::MinimumValueNotMet { min_value: _ })
        ));
    }

    #[cot::test]
    async fn integer_field_clean_value_above_max_value() {
        let mut field = IntegerField::<i32>::with_options(
            FormFieldOptions {
                id: "test".to_owned(),
                name: "test".to_owned(),
                required: true,
            },
            IntegerFieldOptions {
                min: Some(10),
                max: Some(50),
                ..Default::default()
            },
        );
        field
            .set_value(FormFieldValue::new_text("100"))
            .await
            .unwrap();
        let value = i32::clean_value(&field);
        assert!(matches!(
            value,
            Err(FormFieldValidationError::MaximumValueExceeded { max_value: _ })
        ));
    }

    #[test]
    fn bool_field_render() {
        let field = BoolField::with_options(
            FormFieldOptions {
                id: "test".to_owned(),
                name: "test".to_owned(),
                required: true,
            },
            BoolFieldOptions {
                must_be_true: Some(false),
            },
        );
        let html = field.to_string();
        assert!(html.contains("type=\"checkbox\""));
        assert!(html.contains("type=\"hidden\""));
        assert!(!html.contains("required"));
    }

    #[test]
    fn bool_field_render_must_be_true() {
        let field = BoolField::with_options(
            FormFieldOptions {
                id: "test".to_owned(),
                name: "test".to_owned(),
                required: true,
            },
            BoolFieldOptions {
                must_be_true: Some(true),
            },
        );
        let html = field.to_string();
        assert!(html.contains("type=\"checkbox\""));
        assert!(!html.contains("type=\"hidden\""));
        assert!(html.contains("required"));
    }

    #[cot::test]
    async fn bool_field_clean_value() {
        let mut field = BoolField::with_options(
            FormFieldOptions {
                id: "test".to_owned(),
                name: "test".to_owned(),
                required: true,
            },
            BoolFieldOptions {
                must_be_true: Some(true),
            },
        );
        field
            .set_value(FormFieldValue::new_text("true"))
            .await
            .unwrap();
        let value = bool::clean_value(&field).unwrap();
        assert!(value);
    }

    #[test]
    fn float_field_render() {
        let field = FloatField::<f32>::with_options(
            FormFieldOptions {
                id: "test".to_owned(),
                name: "test".to_owned(),
                required: true,
            },
            FloatFieldOptions {
                min: Some(1.5),
                max: Some(10.7),
                placeholder: Some("Enter text".to_string()),
                readonly: Some(true),
                step: Some(Step::Any),
            },
        );
        let html = field.to_string();

        assert!(html.contains("type=\"number\""));
        assert!(html.contains("required"));
        assert!(html.contains("min=\"1.5\""));
        assert!(html.contains("max=\"10.7\""));
        assert!(html.contains("step=\"any\""));
        assert!(html.contains("placeholder=\"Enter text\""));
        assert!(html.contains("name=\"test\""));
        assert!(html.contains("id=\"test\""));
        assert!(html.contains("readonly"));
    }

    #[cot::test]
    #[expect(clippy::float_cmp)]
    async fn float_field_clean_value() {
        let mut field = FloatField::<f32>::with_options(
            FormFieldOptions {
                id: "test".to_owned(),
                name: "test".to_owned(),
                required: true,
            },
            FloatFieldOptions {
                min: Some(1.0),
                max: Some(10.0),
                ..Default::default()
            },
        );
        field
            .set_value(FormFieldValue::new_text("5.0"))
            .await
            .unwrap();
        let value = f32::clean_value(&field).unwrap();
        assert_eq!(value, 5.0f32);
    }

    #[cot::test]
    async fn float_field_clean_value_min_value_not_met() {
        let mut field = FloatField::<f32>::with_options(
            FormFieldOptions {
                id: "test".to_owned(),
                name: "test".to_owned(),
                required: true,
            },
            FloatFieldOptions {
                min: Some(5.0),
                max: Some(10.0),
                ..Default::default()
            },
        );
        field
            .set_value(FormFieldValue::new_text("2.0"))
            .await
            .unwrap();
        let value = f32::clean_value(&field);
        assert!(matches!(
            value,
            Err(FormFieldValidationError::MinimumValueNotMet { min_value: _ })
        ));
    }

    #[cot::test]
    async fn float_field_clean_value_max_value_exceeded() {
        let mut field = FloatField::<f32>::with_options(
            FormFieldOptions {
                id: "test".to_owned(),
                name: "test".to_owned(),
                required: true,
            },
            FloatFieldOptions {
                min: Some(5.0),
                max: Some(10.0),
                ..Default::default()
            },
        );
        field
            .set_value(FormFieldValue::new_text("20.0"))
            .await
            .unwrap();
        let value = f32::clean_value(&field);
        assert!(matches!(
            value,
            Err(FormFieldValidationError::MaximumValueExceeded { max_value: _ })
        ));
    }

    #[cot::test]
    async fn float_field_clean_value_nan_and_inf() {
        let mut field = FloatField::<f32>::with_options(
            FormFieldOptions {
                id: "test".to_owned(),
                name: "test".to_owned(),
                required: true,
            },
            FloatFieldOptions {
                min: Some(1.0),
                max: Some(10.0),
                ..Default::default()
            },
        );
        let bad_inputs = ["NaN", "inf"];

        for &bad_input in &bad_inputs {
            field
                .set_value(FormFieldValue::new_text(bad_input))
                .await
                .unwrap();
            let value = f32::clean_value(&field);
            assert_eq!(
                value,
                Err(FormFieldValidationError::from_static(
                    "Cannot have NaN or inf as form input values"
                ))
            );
        }
    }

    #[cot::test]
    async fn float_field_clean_required() {
        let mut field = FloatField::<f32>::with_options(
            FormFieldOptions {
                id: "test".to_owned(),
                name: "test".to_owned(),
                required: true,
            },
            FloatFieldOptions {
                min: Some(1.0),
                max: Some(10.0),
                ..Default::default()
            },
        );
        field.set_value(FormFieldValue::new_text("")).await.unwrap();
        let value = f32::clean_value(&field);
        assert_eq!(value, Err(FormFieldValidationError::Required));
    }

    #[cot::test]
    async fn url_field_clean_value() {
        let mut field = UrlField::with_options(
            FormFieldOptions {
                id: "test".to_owned(),
                name: "test".to_owned(),
                required: true,
            },
            UrlFieldOptions {
                max_length: Some(100),
                min_length: Some(5),
                size: Some(30),
                list: Some(List::new(["https://example.com"])),
                dir: Some(Dir::Ltr),
                dirname: Some("dir".to_owned()),
                autocomplete: Some(AutoComplete::Value("url".to_owned())),
                placeholder: Some("Enter URL".to_owned()),
                readonly: Some(true),
            },
        );

        field
            .set_value(FormFieldValue::new_text("https://example.com"))
            .await
            .unwrap();

        let value = Url::clean_value(&field).unwrap();
        assert_eq!(
            value.as_str(),
            Url::new("https://example.com").unwrap().as_str()
        );
    }

    #[cot::test]
    async fn url_field_render() {
        let mut field = UrlField::with_options(
            FormFieldOptions {
                id: "id_url".to_owned(),
                name: "url".to_owned(),
                required: true,
            },
            UrlFieldOptions {
                max_length: Some(120),
                min_length: Some(10),
                size: Some(40),
                list: Some(List::new(["https://one.com", "https://two.com"])),
                dir: Some(Dir::Ltr),
                dirname: Some("lang".to_owned()),
                autocomplete: Some(AutoComplete::Value("url".to_owned())),
                placeholder: Some("Paste link".to_owned()),
                readonly: Some(true),
            },
        );

        field
            .set_value(FormFieldValue::new_text("http://example.com"))
            .await
            .unwrap();

        let html = field.to_string();

        assert!(html.contains("type=\"url\""));
        assert!(html.contains("id=\"id_url\""));
        assert!(html.contains("name=\"id_url\""));
        assert!(html.contains("required"));
        assert!(html.contains("value=\"http://example.com\""));
        assert!(html.contains("maxlength=\"120\""));
        assert!(html.contains("minlength=\"10\""));
        assert!(html.contains("size=\"40\""));
        assert!(html.contains("placeholder=\"Paste link\""));
        assert!(html.contains("autocomplete=\"url\""));
        assert!(html.contains("dir=\"ltr\""));
        assert!(html.contains("dirname=\"lang\""));
        assert!(html.contains("readonly"));
        assert!(html.contains("<datalist"));
    }

    #[cot::test]
    async fn url_field_clean_required() {
        let mut field = UrlField::with_options(
            FormFieldOptions {
                id: "id_url".to_owned(),
                name: "url".to_owned(),
                required: true,
            },
            UrlFieldOptions {
                max_length: Some(50),
                min_length: Some(5),
                size: Some(20),
                list: None,
                dir: Some(Dir::Rtl),
                dirname: Some("rtl".to_owned()),
                autocomplete: Some(AutoComplete::Off),
                placeholder: Some("Enter a link".to_owned()),
                readonly: Some(false),
            },
        );

        field.set_value(FormFieldValue::new_text("")).await.unwrap();

        let value = Url::clean_value(&field);
        assert_eq!(value, Err(FormFieldValidationError::Required));
    }
}
