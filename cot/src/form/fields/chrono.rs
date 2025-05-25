use std::fmt::{Display, Formatter};
use std::time::Duration;

use askama::filters::HtmlSafe;
use chrono::{NaiveTime, Weekday, WeekdaySet};
use cot::form::fields::{SelectChoice, SelectField, check_required};
use cot::form::{AsFormField, FormFieldValidationError};
use cot::html::HtmlTag;

use crate::form::FormField;
use crate::form::fields::{SelectMultipleField, check_required_multiple};

impl AsFormField for Weekday {
    type Type = SelectField<Self>;

    fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError> {
        let value = check_required(field)?;

        Ok(Self::from_str(value)?)
    }

    fn to_field_value(&self) -> String {
        <Self as SelectChoice>::to_string(self)
    }
}

macro_rules! impl_as_form_field_mult {
    ($field_type:ty) => {
        impl_as_form_field_mult_collection!(::std::vec::Vec<$field_type>, $field_type);
        impl_as_form_field_mult_collection!(::std::collections::VecDeque<$field_type>, $field_type);
        impl_as_form_field_mult_collection!(
            ::std::collections::LinkedList<$field_type>,
            $field_type
        );
        impl_as_form_field_mult_collection!(::std::collections::HashSet<$field_type>, $field_type);
        impl_as_form_field_mult_collection!(::indexmap::IndexSet<$field_type>, $field_type);
    };
}

macro_rules! impl_as_form_field_mult_collection {
    ($collection_type:ty, $field_type:ty) => {
        impl AsFormField for $collection_type {
            type Type = SelectMultipleField<$field_type>;

            fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError> {
                let value = check_required_multiple(field)?;

                value.iter().map(|id| <$field_type>::from_str(id)).collect()
            }

            fn to_field_value(&self) -> String {
                String::new()
            }
        }
    };
}

impl_as_form_field_mult!(Weekday);
impl_as_form_field_mult_collection!(WeekdaySet, Weekday);

const MONDAY_ID: &'static str = "mon";
const TUESDAY_ID: &'static str = "tue";
const WEDNESDAY_ID: &'static str = "wed";
const THURSDAY_ID: &'static str = "thu";
const FRIDAY_ID: &'static str = "fri";
const SATURDAY_ID: &'static str = "sat";
const SUNDAY_ID: &'static str = "sun";

impl SelectChoice for Weekday {
    fn default_choices() -> Vec<Self>
    where
        Self: Sized,
    {
        vec![
            Weekday::Mon,
            Weekday::Tue,
            Weekday::Wed,
            Weekday::Thu,
            Weekday::Fri,
            Weekday::Sat,
            Weekday::Sun,
        ]
    }

    fn from_str(s: &str) -> Result<Self, FormFieldValidationError>
    where
        Self: Sized,
    {
        match s.to_lowercase().as_str() {
            MONDAY_ID => Ok(Weekday::Mon),
            TUESDAY_ID => Ok(Weekday::Tue),
            WEDNESDAY_ID => Ok(Weekday::Wed),
            THURSDAY_ID => Ok(Weekday::Thu),
            FRIDAY_ID => Ok(Weekday::Fri),
            SATURDAY_ID => Ok(Weekday::Sat),
            SUNDAY_ID => Ok(Weekday::Sun),
            _ => Err(FormFieldValidationError::invalid_value(s.to_owned())),
        }
    }

    fn id(&self) -> String {
        match self {
            Weekday::Mon => MONDAY_ID.to_string(),
            Weekday::Tue => TUESDAY_ID.to_string(),
            Weekday::Wed => WEDNESDAY_ID.to_string(),
            Weekday::Thu => THURSDAY_ID.to_string(),
            Weekday::Fri => FRIDAY_ID.to_string(),
            Weekday::Sat => SATURDAY_ID.to_string(),
            Weekday::Sun => SUNDAY_ID.to_string(),
        }
    }

    fn to_string(&self) -> String {
        match self {
            Weekday::Mon => "Monday".to_string(),
            Weekday::Tue => "Tuesday".to_string(),
            Weekday::Wed => "Wednesday".to_string(),
            Weekday::Thu => "Thursday".to_string(),
            Weekday::Fri => "Friday".to_string(),
            Weekday::Sat => "Saturday".to_string(),
            Weekday::Sun => "Sunday".to_string(),
        }
    }
}

crate::impl_form_field!(TimeField, TimeFieldOptions, "a time");

/// Custom options for a [`TimeField`].
///
/// This struct configures the behavior and constraints of time input fields.
/// It allows setting minimum and maximum allowed times, as well as the step
/// interval between allowed time values.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
///
/// use chrono::NaiveTime;
/// use cot::form::fields::{TimeField, TimeFieldOptions};
/// use cot::form::{FormField, FormFieldOptions};
///
/// let options = TimeFieldOptions {
///     min: Some(NaiveTime::from_hms_opt(9, 0, 0).unwrap()),
///     max: Some(NaiveTime::from_hms_opt(17, 0, 0).unwrap()),
///     step: Some(Duration::from_secs(900)), // 15-minute intervals
/// };
///
/// let field = TimeField::with_options(FormFieldOptions::new("appointment_time"), options);
/// ```
#[derive(Debug, Copy, Clone, Default)]
pub struct TimeFieldOptions {
    /// The minimum value of the field. Used to set the `min` attribute in the
    /// HTML input element.
    pub min: Option<NaiveTime>,
    /// The maximum value of the field. Used to set the `max` attribute in the
    /// HTML input element.
    pub max: Option<NaiveTime>,
    /// The step interval between valid time values. Used to set the [`step`
    /// attribute] in the HTML input element.
    ///
    /// [`step` attribute]: https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/input/time#using_the_step_attribute
    pub step: Option<Duration>,
}

impl Display for TimeField {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut tag: HtmlTag = HtmlTag::input("time");
        tag.attr("name", self.name());
        tag.attr("id", self.id());
        if self.options.required {
            tag.bool_attr("required");
        }

        if let Some(min) = &self.custom_options.min {
            tag.attr("min", &min.to_string());
        }
        if let Some(max) = &self.custom_options.max {
            tag.attr("max", &max.to_string());
        }
        if let Some(step) = &self.custom_options.step {
            tag.attr("step", &step.as_secs().to_string());
        }
        if let Some(value) = &self.value {
            tag.attr("value", value);
        }

        write!(f, "{}", tag.render())
    }
}

impl HtmlSafe for TimeField {}

impl AsFormField for NaiveTime {
    type Type = TimeField;

    fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError> {
        let value = check_required(field)?;

        let parsed: NaiveTime = value
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

#[cfg(test)]
mod tests {
    use chrono::Timelike;

    use super::*;

    #[test]
    fn time_field_render() {
        let field = TimeField::with_options(
            FormFieldOptions {
                id: "appointment".to_owned(),
                name: "appointment".to_owned(),
                required: true,
            },
            TimeFieldOptions {
                min: Some(NaiveTime::from_hms_opt(9, 0, 0).unwrap()),
                max: Some(NaiveTime::from_hms_opt(17, 0, 0).unwrap()),
                step: Some(Duration::from_secs(900)), // 15 minutes
            },
        );
        let html = field.to_string();
        assert!(html.contains("type=\"time\""));
        assert!(html.contains("required"));
        assert!(html.contains("min=\"09:00:00\""));
        assert!(html.contains("max=\"17:00:00\""));
        assert!(html.contains("step=\"900\""));
    }

    #[cot::test]
    async fn time_field_clean_value() {
        let mut field = TimeField::with_options(
            FormFieldOptions {
                id: "appointment".to_owned(),
                name: "appointment".to_owned(),
                required: true,
            },
            TimeFieldOptions {
                min: Some(NaiveTime::from_hms_opt(9, 0, 0).unwrap()),
                max: Some(NaiveTime::from_hms_opt(17, 0, 0).unwrap()),
                step: None,
            },
        );
        field
            .set_value(FormFieldValue::new_text("13:30:00"))
            .await
            .unwrap();
        let value = NaiveTime::clean_value(&field).unwrap();
        assert_eq!(value.hour(), 13);
        assert_eq!(value.minute(), 30);
        assert_eq!(value.second(), 0);
    }

    #[cot::test]
    async fn time_field_clean_value_below_min() {
        let mut field = TimeField::with_options(
            FormFieldOptions {
                id: "appointment".to_owned(),
                name: "appointment".to_owned(),
                required: true,
            },
            TimeFieldOptions {
                min: Some(NaiveTime::from_hms_opt(9, 0, 0).unwrap()),
                max: Some(NaiveTime::from_hms_opt(17, 0, 0).unwrap()),
                step: None,
            },
        );
        field
            .set_value(FormFieldValue::new_text("08:30:00"))
            .await
            .unwrap();
        let result = NaiveTime::clean_value(&field);
        assert!(matches!(
            result,
            Err(FormFieldValidationError::MinimumValueNotMet { min_value: _ })
        ));
    }

    #[cot::test]
    async fn time_field_clean_value_above_max() {
        let mut field = TimeField::with_options(
            FormFieldOptions {
                id: "appointment".to_owned(),
                name: "appointment".to_owned(),
                required: true,
            },
            TimeFieldOptions {
                min: Some(NaiveTime::from_hms_opt(9, 0, 0).unwrap()),
                max: Some(NaiveTime::from_hms_opt(17, 0, 0).unwrap()),
                step: None,
            },
        );
        field
            .set_value(FormFieldValue::new_text("18:30:00"))
            .await
            .unwrap();
        let result = NaiveTime::clean_value(&field);
        assert!(matches!(
            result,
            Err(FormFieldValidationError::MaximumValueExceeded { max_value: _ })
        ));
    }

    #[cot::test]
    async fn time_field_clean_required() {
        let mut field = TimeField::with_options(
            FormFieldOptions {
                id: "appointment".to_owned(),
                name: "appointment".to_owned(),
                required: true,
            },
            TimeFieldOptions {
                min: None,
                max: None,
                step: None,
            },
        );
        field.set_value(FormFieldValue::new_text("")).await.unwrap();
        let result = NaiveTime::clean_value(&field);
        assert_eq!(result, Err(FormFieldValidationError::Required));
    }
}
