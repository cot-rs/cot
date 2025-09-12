use std::fmt::{Debug, Display, Formatter};

use askama::filters::HtmlSafe;
/// Derive helper that implements `AsFormField` for select-like enums and common
/// collections.
///
/// Apply this together with [`SelectChoice`] to your enum to enable using it
/// directly as a form field (`SelectField<T>`) and as multi-select via common
/// collections (`Vec<T>`, `VecDeque<T>`, `LinkedList<T>`, `HashSet<T>`, and
/// `indexmap::IndexSet<T>`).
///
/// ```
/// use cot::form::fields::{SelectAsFormField, SelectChoice, SelectField, SelectMultipleField};
///
/// #[derive(SelectChoice, SelectAsFormField, Debug, Clone, PartialEq, Eq, Hash)]
/// enum Status {
///     Draft,
///     Published,
///     Archived,
/// }
///
/// // `Status` works with `SelectField<Status>` and `SelectMultipleField<Status>`.
/// ```
pub use cot_macros::SelectAsFormField;
/// Derive the [`SelectChoice`] trait for an enum.
///
/// This macro automatically implements the [`SelectChoice`] trait for enums,
/// allowing them to be used with [`SelectField`] and [`SelectMultipleField`]
/// form fields. The macro generates implementations for all required methods
/// based on the enum variants.
///
/// # Requirements
///
/// - The type must be an enum (not a struct or union)
/// - The enum must have at least one variant
/// - All variants must be unit variants (no associated data)
///
/// # Default Behavior
///
/// By default, the macro uses the variant name for both the ID and display
/// name:
/// - `id()` returns the variant name as a string
/// - `to_string()` returns the variant name as a string
/// - `from_str()` matches the variant name (case-sensitive)
/// - `default_choices()` returns all variants in declaration order
///
/// # Attributes
///
/// You can customize the behavior using the `#[select_choice(...)]` attribute
/// on individual enum variants:
///
/// ## `id`
///
/// Override the ID used for this variant in form submissions and HTML.
///
/// ```
/// use cot::form::fields::SelectChoice;
///
/// #[derive(SelectChoice, Debug, PartialEq)]
/// enum Status {
///     #[select_choice(id = "draft")]
///     Draft,
///     #[select_choice(id = "published")]
///     Published,
/// }
///
/// assert_eq!(Status::Draft.id(), "draft");
/// assert_eq!(Status::Published.id(), "published");
/// ```
///
/// ## `name`
///
/// Override the display name shown to users in the select dropdown.
///
/// ```
/// use cot::form::fields::SelectChoice;
///
/// #[derive(SelectChoice, Debug, PartialEq)]
/// enum Priority {
///     #[select_choice(name = "Low Priority")]
///     Low,
///     #[select_choice(name = "High Priority")]
///     High,
/// }
///
/// assert_eq!(Priority::Low.to_string(), "Low Priority");
/// assert_eq!(Priority::High.to_string(), "High Priority");
/// ```
///
/// # Error Cases
///
/// The macro will fail to compile if:
///
/// - The type is not an enum
/// - The enum has no variants
/// - Any variant has associated data (non-unit variants)
///
/// ```compile_fail
/// use cot_macros::SelectChoice;
///
/// // This will fail - structs are not supported
/// #[derive(SelectChoice)]
/// struct NotAnEnum {
///     field: String,
/// }
/// ```
///
/// ```compile_fail
/// use cot_macros::SelectChoice;
///
/// // This will fail - empty enums are not supported
/// #[derive(SelectChoice)]
/// enum EmptyEnum {}
/// ```
///
/// ```compile_fail
/// use cot_macros::SelectChoice;
///
/// // This will fail - only unit variants are supported
/// #[derive(SelectChoice)]
/// enum EnumWithData {
///     Unit,
///     WithData(String),
///     WithFields { field: i32 },
/// }
/// ```
///
/// [`SelectChoice`]: cot::form::fields::SelectChoice
/// [`SelectField`]: cot::form::fields::SelectField
/// [`SelectMultipleField`]: cot::form::fields::SelectMultipleField
pub use cot_macros::SelectChoice;
use indexmap::IndexSet;

use crate::form::fields::impl_form_field;
use crate::form::{
    FormField, FormFieldOptions, FormFieldValidationError, FormFieldValue, FormFieldValueError,
};
use crate::html::HtmlTag;

impl_form_field!(SelectField, SelectFieldOptions, "a dropdown list", T: SelectChoice + Send);

/// Custom options for a [`SelectField`].
#[derive(Debug, Clone)]
pub struct SelectFieldOptions<T> {
    /// The list of available choices for the select field.
    /// If not set, the default choices from [`SelectChoice::default_choices`]
    /// will be used.
    pub choices: Option<Vec<T>>,
    /// Custom text for the empty option when the field is not required.
    /// If not set, "—" will be used as the default empty option text.
    /// If the field is required, no empty option will be displayed, unless
    /// this is set explicitly.
    pub none_option: Option<String>,
}

impl<T> Default for SelectFieldOptions<T> {
    fn default() -> Self {
        Self {
            choices: None,
            none_option: None,
        }
    }
}

impl<T: SelectChoice + Send> Display for SelectField<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        const DEFAULT_NONE_OPTION: &str = "—";

        let value = if let Some(value) = self.value.clone() {
            IndexSet::from([value])
        } else {
            IndexSet::new()
        };

        let none_option = if let Some(none_option) = &self.custom_options.none_option {
            Some(none_option.as_str())
        } else if self.options.required {
            None
        } else {
            Some(DEFAULT_NONE_OPTION)
        };
        render_select(
            f,
            self,
            false,
            none_option,
            None,
            self.custom_options.choices.as_ref(),
            &value,
        )
    }
}

impl<T: SelectChoice + Send> HtmlSafe for SelectField<T> {}

/// A form field for a multiple-choice select box.
///
/// This field allows users to select multiple values from a predefined list of
/// choices. Unlike [`SelectField`], this field can accept multiple selections
/// and renders as a multi-select HTML element.
#[derive(Debug)]
pub struct SelectMultipleField<T> {
    options: FormFieldOptions,
    custom_options: SelectMultipleFieldOptions<T>,
    value: IndexSet<String>,
}

impl<T> SelectMultipleField<T> {
    /// Returns an iterator over the selected values as string slices.
    pub fn values(&self) -> impl Iterator<Item = &str> {
        self.value.iter().map(AsRef::as_ref)
    }
}

impl<T: SelectChoice + Send> FormField for SelectMultipleField<T> {
    type CustomOptions = SelectMultipleFieldOptions<T>;

    fn with_options(options: FormFieldOptions, custom_options: Self::CustomOptions) -> Self {
        Self {
            options,
            custom_options,
            value: IndexSet::new(),
        }
    }

    fn options(&self) -> &FormFieldOptions {
        &self.options
    }

    fn value(&self) -> Option<&str> {
        None
    }

    async fn set_value(&mut self, field: FormFieldValue<'_>) -> Result<(), FormFieldValueError> {
        self.value.insert(field.into_text().await?);
        Ok(())
    }
}

/// Custom options for a [`SelectMultipleField`].
#[derive(Debug, Clone)]
pub struct SelectMultipleFieldOptions<T> {
    /// The list of available choices for the multi-select field.
    /// If not set, the default choices from [`SelectChoice::default_choices`]
    /// will be used.
    pub choices: Option<Vec<T>>,
    /// The number of visible options in the select box.
    /// Sets the [`size`] attribute on the HTML select element.
    /// If not set, the browser's default size will be used.
    ///
    /// [`size`]: https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Attributes/size
    pub size: Option<u32>,
}

impl<T> Default for SelectMultipleFieldOptions<T> {
    fn default() -> Self {
        Self {
            choices: None,
            size: None,
        }
    }
}

impl<T: SelectChoice + Send> Display for SelectMultipleField<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        render_select(
            f,
            self,
            true,
            None,
            self.custom_options.size,
            self.custom_options.choices.as_ref(),
            &self.value,
        )
    }
}

impl<T: SelectChoice + Send> HtmlSafe for SelectMultipleField<T> {}

fn render_select<T: FormField, S: SelectChoice>(
    f: &mut Formatter<'_>,
    field: &T,
    multiple: bool,
    empty_option: Option<&str>,
    size: Option<u32>,
    choices: Option<&Vec<S>>,
    selected: &IndexSet<String>,
) -> std::fmt::Result {
    let mut tag: HtmlTag = HtmlTag::new("select");
    tag.attr("name", field.id());
    tag.attr("id", field.id());
    if multiple {
        tag.bool_attr("multiple");
    }
    if field.options().required {
        tag.bool_attr("required");
    }

    if let Some(size) = size {
        tag.attr("size", size.to_string());
    }

    if let Some(empty_option) = empty_option {
        tag.push_tag(
            HtmlTag::new("option")
                .attr("value", "")
                .push_str(empty_option),
        );
    }

    let choices = if let Some(choices) = choices {
        choices
    } else {
        &S::default_choices()
    };
    for choice in choices {
        let mut child = HtmlTag::new("option");
        child
            .attr("value", choice.id())
            .push_str(choice.to_string());
        if selected.contains(&choice.id()) {
            child.bool_attr("selected");
        }
        tag.push_tag(child);
    }

    write!(f, "{}", tag.render())
}

pub(crate) fn check_required_multiple<T>(
    field: &SelectMultipleField<T>,
) -> Result<&IndexSet<String>, FormFieldValidationError> {
    if field.value.is_empty() {
        Err(FormFieldValidationError::Required)
    } else {
        Ok(&field.value)
    }
}

impl<T: SelectChoice + Send> crate::form::AsFormField for ::std::vec::Vec<T> {
    type Type = SelectMultipleField<T>;

    fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError> {
        let values = check_required_multiple(field)?;
        values.iter().map(|id| T::from_str(id)).collect()
    }

    fn to_field_value(&self) -> String {
        String::new()
    }
}

impl<T: SelectChoice + Send> crate::form::AsFormField for ::std::collections::VecDeque<T> {
    type Type = SelectMultipleField<T>;

    fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError> {
        let values = check_required_multiple(field)?;
        let mut out = ::std::collections::VecDeque::new();
        for id in values {
            out.push_back(T::from_str(id)?);
        }
        Ok(out)
    }

    fn to_field_value(&self) -> String {
        String::new()
    }
}

impl<T: SelectChoice + Send> crate::form::AsFormField for ::std::collections::LinkedList<T> {
    type Type = SelectMultipleField<T>;

    fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError> {
        let values = check_required_multiple(field)?;
        let mut out = ::std::collections::LinkedList::new();
        for id in values {
            out.push_back(T::from_str(id)?);
        }
        Ok(out)
    }

    fn to_field_value(&self) -> String {
        String::new()
    }
}

impl<T: SelectChoice + Eq + ::std::hash::Hash + Send, S: ::std::hash::BuildHasher + Default>
    crate::form::AsFormField for ::std::collections::HashSet<T, S>
{
    type Type = SelectMultipleField<T>;

    fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError> {
        let values = check_required_multiple(field)?;
        let mut out = ::std::collections::HashSet::default();
        for id in values {
            out.insert(T::from_str(id)?);
        }
        Ok(out)
    }

    fn to_field_value(&self) -> String {
        String::new()
    }
}

impl<T: SelectChoice + Eq + ::std::hash::Hash + Send> crate::form::AsFormField
    for ::indexmap::IndexSet<T>
{
    type Type = SelectMultipleField<T>;

    fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError> {
        let values = check_required_multiple(field)?;
        let mut out = ::indexmap::IndexSet::new();
        for id in values {
            out.insert(T::from_str(id)?);
        }
        Ok(out)
    }

    fn to_field_value(&self) -> String {
        String::new()
    }
}

/// A trait for types that can be used as choices in select fields.
///
/// This trait enables types to be used with [`SelectField`] and
/// [`SelectMultipleField`], providing the necessary methods for converting
/// between string representations and the actual type values.
///
/// # Examples
///
/// ```
/// use cot::form::FormFieldValidationError;
/// use cot::form::fields::SelectChoice;
///
/// #[derive(Debug, Clone, PartialEq)]
/// enum Status {
///     Draft,
///     Published,
///     Archived,
/// }
///
/// impl SelectChoice for Status {
///     fn default_choices() -> Vec<Self> {
///         vec![Self::Draft, Self::Published, Self::Archived]
///     }
///
///     fn from_str(s: &str) -> Result<Self, FormFieldValidationError> {
///         match s {
///             "draft" => Ok(Self::Draft),
///             "published" => Ok(Self::Published),
///             "archived" => Ok(Self::Archived),
///             _ => Err(FormFieldValidationError::invalid_value(s.to_owned())),
///         }
///     }
///
///     fn id(&self) -> String {
///         match self {
///             Self::Draft => "draft".to_string(),
///             Self::Published => "published".to_string(),
///             Self::Archived => "archived".to_string(),
///         }
///     }
///
///     fn to_string(&self) -> String {
///         match self {
///             Self::Draft => "Draft".to_string(),
///             Self::Published => "Published".to_string(),
///             Self::Archived => "Archived".to_string(),
///         }
///     }
/// }
///
/// assert_eq!(Status::from_str("draft").unwrap(), Status::Draft);
/// assert_eq!(Status::Draft.id(), "draft");
/// assert_eq!(Status::Draft.to_string(), "Draft");
/// ```
pub trait SelectChoice {
    /// Returns the default list of choices for this type.
    ///
    /// This method is called when no explicit choices are provided to a select
    /// field. The default implementation returns an empty vector.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::form::fields::SelectChoice;
    ///
    /// #[derive(Debug, Clone)]
    /// enum Color {
    ///     Red,
    ///     Green,
    ///     Blue,
    /// }
    ///
    /// impl SelectChoice for Color {
    ///     fn default_choices() -> Vec<Self> {
    ///         vec![Self::Red, Self::Green, Self::Blue]
    ///     }
    /// #
    /// #     fn from_str(_: &str) -> Result<Self, cot::form::FormFieldValidationError> {
    /// #         unimplemented!()
    /// #     }
    /// #     fn id(&self) -> String {
    /// #         unimplemented!()
    /// #     }
    /// #     fn to_string(&self) -> String {
    /// #         unimplemented!()
    /// #     }
    /// }
    ///
    /// assert_eq!(Color::default_choices().len(), 3);
    /// ```
    #[must_use]
    fn default_choices() -> Vec<Self>
    where
        Self: Sized,
    {
        vec![]
    }

    /// Converts a string representation to the choice type.
    ///
    /// This method is used during form processing to convert submitted form
    /// values back into the appropriate choice type.
    ///
    /// # Errors
    ///
    /// If the string does not match any valid choice, this method should return
    /// a `FormFieldValidationError` indicating the invalid value.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::form::FormFieldValidationError;
    /// use cot::form::fields::SelectChoice;
    ///
    /// #[derive(Debug, Clone, PartialEq)]
    /// enum Size {
    ///     Small,
    ///     Medium,
    ///     Large,
    /// }
    ///
    /// impl SelectChoice for Size {
    ///     fn from_str(s: &str) -> Result<Self, FormFieldValidationError> {
    ///         match s {
    ///             "small" => Ok(Self::Small),
    ///             "medium" => Ok(Self::Medium),
    ///             "large" => Ok(Self::Large),
    ///             _ => Err(FormFieldValidationError::invalid_value(s.to_owned())),
    ///         }
    ///     }
    /// #
    /// #     fn id(&self) -> String {
    /// #         unimplemented!()
    /// #     }
    /// #     fn to_string(&self) -> String {
    /// #         unimplemented!()
    /// #     }
    /// }
    ///
    /// assert_eq!(Size::from_str("small").unwrap(), Size::Small);
    /// assert!(Size::from_str("invalid").is_err());
    /// ```
    fn from_str(s: &str) -> Result<Self, FormFieldValidationError>
    where
        Self: Sized;

    /// Returns the unique identifier for this choice.
    ///
    /// This value is used as the `value` attribute in HTML option elements
    /// and should be unique among all choices for a given type.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::form::fields::SelectChoice;
    ///
    /// #[derive(Debug)]
    /// enum Priority {
    ///     Low,
    ///     High,
    /// }
    ///
    /// impl SelectChoice for Priority {
    ///     fn id(&self) -> String {
    ///         match self {
    ///             Self::Low => "low".to_string(),
    ///             Self::High => "high".to_string(),
    ///         }
    ///     }
    /// #
    /// #     fn from_str(_: &str) -> Result<Self, cot::form::FormFieldValidationError> {
    /// #         unimplemented!()
    /// #     }
    /// #     fn to_string(&self) -> String {
    /// #         unimplemented!()
    /// #     }
    /// }
    ///
    /// assert_eq!(Priority::Low.id(), "low");
    /// assert_eq!(Priority::High.id(), "high");
    /// ```
    fn id(&self) -> String;

    /// Returns the human-readable display text for this choice.
    ///
    /// This text is shown to users in the option elements and should be
    /// descriptive and user-friendly.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::form::fields::SelectChoice;
    ///
    /// #[derive(Debug)]
    /// enum Status {
    ///     Active,
    ///     Inactive,
    /// }
    ///
    /// impl SelectChoice for Status {
    ///     fn to_string(&self) -> String {
    ///         match self {
    ///             Self::Active => "Currently Active".to_string(),
    ///             Self::Inactive => "Currently Inactive".to_string(),
    ///         }
    ///     }
    /// #
    /// #     fn from_str(_: &str) -> Result<Self, cot::form::FormFieldValidationError> {
    /// #         unimplemented!()
    /// #     }
    /// #     fn id(&self) -> String {
    /// #         unimplemented!()
    /// #     }
    /// }
    ///
    /// assert_eq!(Status::Active.to_string(), "Currently Active");
    /// ```
    fn to_string(&self) -> String;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    enum TestChoice {
        Option1,
        Option2,
        Option3,
    }

    impl SelectChoice for TestChoice {
        fn default_choices() -> Vec<Self> {
            vec![Self::Option1, Self::Option2, Self::Option3]
        }

        fn from_str(s: &str) -> Result<Self, FormFieldValidationError> {
            match s {
                "opt1" => Ok(Self::Option1),
                "opt2" => Ok(Self::Option2),
                "opt3" => Ok(Self::Option3),
                _ => Err(FormFieldValidationError::invalid_value(s.to_owned())),
            }
        }

        fn id(&self) -> String {
            match self {
                Self::Option1 => "opt1".to_string(),
                Self::Option2 => "opt2".to_string(),
                Self::Option3 => "opt3".to_string(),
            }
        }

        fn to_string(&self) -> String {
            match self {
                Self::Option1 => "Option 1".to_string(),
                Self::Option2 => "Option 2".to_string(),
                Self::Option3 => "Option 3".to_string(),
            }
        }
    }

    #[test]
    fn select_field_render_default() {
        let field = SelectField::<TestChoice>::with_options(
            FormFieldOptions {
                id: "test_select".to_owned(),
                name: "test_select".to_owned(),
                required: false,
            },
            SelectFieldOptions::default(),
        );
        let html = field.to_string();

        assert!(html.contains("<select"));
        assert!(html.contains("name=\"test_select\""));
        assert!(html.contains("id=\"test_select\""));
        assert!(!html.contains("required"));
        assert!(html.contains("—")); // default empty option
        assert!(html.contains("Option 1"));
        assert!(html.contains("Option 2"));
        assert!(html.contains("Option 3"));
        assert!(html.contains("value=\"opt1\""));
        assert!(html.contains("value=\"opt2\""));
        assert!(html.contains("value=\"opt3\""));
    }

    #[test]
    fn select_field_render_required() {
        let field = SelectField::<TestChoice>::with_options(
            FormFieldOptions {
                id: "test_select".to_owned(),
                name: "test_select".to_owned(),
                required: true,
            },
            SelectFieldOptions::default(),
        );
        let html = field.to_string();

        assert!(html.contains("required"));
        assert!(!html.contains("—")); // no empty option for required field
    }

    #[test]
    fn select_field_render_custom_none_option() {
        let field = SelectField::<TestChoice>::with_options(
            FormFieldOptions {
                id: "test_select".to_owned(),
                name: "test_select".to_owned(),
                required: false,
            },
            SelectFieldOptions {
                choices: None,
                none_option: Some("Please select...".to_string()),
            },
        );
        let html = field.to_string();

        assert!(html.contains("Please select..."));
        assert!(!html.contains("—"));
    }

    #[test]
    fn select_field_render_custom_choices() {
        let field = SelectField::<TestChoice>::with_options(
            FormFieldOptions {
                id: "test_select".to_owned(),
                name: "test_select".to_owned(),
                required: false,
            },
            SelectFieldOptions {
                choices: Some(vec![TestChoice::Option1, TestChoice::Option3]),
                none_option: None,
            },
        );
        let html = field.to_string();

        assert!(html.contains("Option 1"));
        assert!(!html.contains("Option 2")); // not in custom choices
        assert!(html.contains("Option 3"));
    }

    #[cot::test]
    async fn select_field_with_value() {
        let mut field = SelectField::<TestChoice>::with_options(
            FormFieldOptions {
                id: "test_select".to_owned(),
                name: "test_select".to_owned(),
                required: false,
            },
            SelectFieldOptions::default(),
        );

        field
            .set_value(FormFieldValue::new_text("opt2"))
            .await
            .unwrap();
        let html = field.to_string();

        assert!(html.contains("<option value=\"opt2\" selected>Option 2</option>"));
    }

    #[test]
    fn select_multiple_field_render_default() {
        let field = SelectMultipleField::<TestChoice>::with_options(
            FormFieldOptions {
                id: "test_multi".to_owned(),
                name: "test_multi".to_owned(),
                required: false,
            },
            SelectMultipleFieldOptions::default(),
        );
        let html = field.to_string();

        assert!(html.contains("<select"));
        assert!(html.contains("multiple"));
        assert!(html.contains("name=\"test_multi\""));
        assert!(html.contains("id=\"test_multi\""));
        assert!(!html.contains("required"));
        assert!(html.contains("Option 1"));
        assert!(html.contains("Option 2"));
        assert!(html.contains("Option 3"));
    }

    #[test]
    fn select_multiple_field_render_with_size() {
        let field = SelectMultipleField::<TestChoice>::with_options(
            FormFieldOptions {
                id: "test_multi".to_owned(),
                name: "test_multi".to_owned(),
                required: false,
            },
            SelectMultipleFieldOptions {
                choices: None,
                size: Some(5),
            },
        );
        let html = field.to_string();

        assert!(html.contains("size=\"5\""));
    }

    #[test]
    fn select_multiple_field_render_required() {
        let field = SelectMultipleField::<TestChoice>::with_options(
            FormFieldOptions {
                id: "test_multi".to_owned(),
                name: "test_multi".to_owned(),
                required: true,
            },
            SelectMultipleFieldOptions::default(),
        );
        let html = field.to_string();

        assert!(html.contains("required"));
    }

    #[cot::test]
    async fn select_multiple_field_with_values() {
        let mut field = SelectMultipleField::<TestChoice>::with_options(
            FormFieldOptions {
                id: "test_multi".to_owned(),
                name: "test_multi".to_owned(),
                required: false,
            },
            SelectMultipleFieldOptions::default(),
        );

        field
            .set_value(FormFieldValue::new_text("opt1"))
            .await
            .unwrap();
        field
            .set_value(FormFieldValue::new_text("opt3"))
            .await
            .unwrap();

        let html = field.to_string();
        assert!(html.contains("<option value=\"opt1\" selected>Option 1</option>"));
        assert!(html.contains("<option value=\"opt3\" selected>Option 3</option>"));
        assert!(!html.contains("<option value=\"opt2\" selected>"));

        let values: Vec<&str> = field.values().collect();
        assert_eq!(values.len(), 2);
        assert!(values.contains(&"opt1"));
        assert!(values.contains(&"opt3"));
    }

    #[test]
    fn select_choice_default_choices() {
        let choices = TestChoice::default_choices();
        assert_eq!(choices.len(), 3);
        assert_eq!(choices[0], TestChoice::Option1);
        assert_eq!(choices[1], TestChoice::Option2);
        assert_eq!(choices[2], TestChoice::Option3);
    }

    #[test]
    fn select_choice_from_str_invalid() {
        let result = TestChoice::from_str("invalid");
        assert!(result.is_err());
        if let Err(FormFieldValidationError::InvalidValue(value)) = result {
            assert_eq!(value, "invalid");
        } else {
            panic!("Expected InvalidValue error");
        }
    }

    #[test]
    fn check_required_multiple_empty() {
        let field = SelectMultipleField::<TestChoice>::with_options(
            FormFieldOptions {
                id: "test".to_owned(),
                name: "test".to_owned(),
                required: true,
            },
            SelectMultipleFieldOptions::default(),
        );

        let result = check_required_multiple(&field);
        assert_eq!(result, Err(FormFieldValidationError::Required));
    }

    #[cot::test]
    async fn check_required_multiple_with_values() {
        let mut field = SelectMultipleField::<TestChoice>::with_options(
            FormFieldOptions {
                id: "test".to_owned(),
                name: "test".to_owned(),
                required: true,
            },
            SelectMultipleFieldOptions::default(),
        );

        field
            .set_value(FormFieldValue::new_text("opt1"))
            .await
            .unwrap();
        let result = check_required_multiple(&field);
        assert!(result.is_ok());

        let values = result.unwrap();
        assert_eq!(values.len(), 1);
        assert!(values.contains("opt1"));
    }

    #[cot::test]
    async fn select_multiple_field_values_iterator() {
        let mut field = SelectMultipleField::<TestChoice>::with_options(
            FormFieldOptions {
                id: "test".to_owned(),
                name: "test".to_owned(),
                required: false,
            },
            SelectMultipleFieldOptions::default(),
        );

        let values: Vec<&str> = field.values().collect();
        assert!(values.is_empty());

        field
            .set_value(FormFieldValue::new_text("opt2"))
            .await
            .unwrap();
        field
            .set_value(FormFieldValue::new_text("opt1"))
            .await
            .unwrap();
        field
            .set_value(FormFieldValue::new_text("opt2"))
            .await
            .unwrap(); // duplicate should be ignored

        let values: Vec<&str> = field.values().collect();
        assert_eq!(values.len(), 2); // IndexSet should deduplicate
        assert!(values.contains(&"opt1"));
        assert!(values.contains(&"opt2"));
    }
}
