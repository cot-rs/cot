use std::fmt::{Display, Formatter};

use askama::filters::HtmlSafe;
use chrono::{
    DateTime, Duration, FixedOffset, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, Offset,
    ParseError, TimeZone, Weekday, WeekdaySet,
};
use chrono_tz::Tz;
use cot::form::FormField;
use cot::form::fields::impl_form_field;
use cot::html::HtmlTag;

use crate::form::fields::{
    SelectChoice, SelectField, SelectMultipleField, Step, check_required, check_required_multiple,
};
use crate::form::{AsFormField, FormFieldValidationError};

impl AsFormField for Weekday {
    type Type = SelectField<Self>;

    fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError> {
        let value = check_required(field)?;

        Self::from_str(value)
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

const MONDAY_ID: &str = "mon";
const TUESDAY_ID: &str = "tue";
const WEDNESDAY_ID: &str = "wed";
const THURSDAY_ID: &str = "thu";
const FRIDAY_ID: &str = "fri";
const SATURDAY_ID: &str = "sat";
const SUNDAY_ID: &str = "sun";

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

const BROWSER_DATETIME_FMT: &str = "%Y-%m-%dT%H:%M:%S";
const BROWSER_DATETIME_WITHOUT_SEC_FMT: &str = "%Y-%m-%dT%H:%M";
const BROWSER_DATE_FMT: &str = "%Y-%m-%d";
const BROWSER_TIME_FMT: &str = "%H:%M:%S";
const BROWSER_TIME_WITHOUT_SEC_FMT: &str = "%H:%M";

fn parse_datetime_with_fallback(value: &str) -> Result<NaiveDateTime, ParseError> {
    NaiveDateTime::parse_from_str(value, BROWSER_DATETIME_FMT)
        .or_else(|_| NaiveDateTime::parse_from_str(value, BROWSER_DATETIME_WITHOUT_SEC_FMT))
}

fn parse_time_with_fallback(value: &str) -> Result<NaiveTime, ParseError> {
    NaiveTime::parse_from_str(value, BROWSER_TIME_FMT)
        .or_else(|_| NaiveTime::parse_from_str(value, BROWSER_TIME_WITHOUT_SEC_FMT))
}

impl_form_field!(DateTimeField, DateTimeFieldOptions, "a datetime");

/// Custom options for [`DateTimeField`]
///
/// Specifies the HTML attributes applied to a datetime-local input.
///
/// # Example
///
/// ```
/// use chrono::{Duration, NaiveDateTime};
/// use cot::form::fields::{DateTimeField, DateTimeFieldOptions, Step};
/// use cot::form::{FormField, FormFieldOptions};
///
/// let now = chrono::Local::now().naive_local();
/// let in_two_days = now + Duration::hours(48);
///
/// let options = DateTimeFieldOptions {
///     min: Some(now),
///     max: Some(in_two_days),
///     readonly: Some(true),
///     step: Some(Step::Value(Duration::seconds(300))),
/// };
///
/// let field = DateTimeField::with_options(
///     FormFieldOptions {
///         id: "event_time".into(),
///         name: "event_time".into(),
///         required: true,
///     },
///     options,
/// );
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct DateTimeFieldOptions {
    /// The maximum datetime value of the field used to set the `max` attribute
    /// in the HTML input element.
    pub max: Option<NaiveDateTime>,
    /// The minimum datetime value of the field used to set the `min` attribute
    /// in the HTML input element.
    pub min: Option<NaiveDateTime>,
    /// Whether the field should be read-only. When set to `true`, the user
    /// cannot modify the field value through the HTML input element.
    pub readonly: Option<bool>,
    /// Granularity of the datetime input, in seconds.
    ///
    /// Corresponds to the `step` attribute on `<input type="datetime-local">`.
    /// If `None`, the browser’s default (60 seconds) is used. To override,
    /// supply `Step::Value(Duration)` where `Duration` specifies the number
    /// of seconds between steps.
    ///
    /// [`step` attribute]: https://developer.mozilla.org/en-US/docs/Web/HTML/Element/input/datetime-local#step
    pub step: Option<Step<Duration>>,
}

impl Display for DateTimeField {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut tag = HtmlTag::input("datetime-local");
        tag.attr("name", self.id());
        tag.attr("id", self.id());
        if self.options.required {
            tag.bool_attr("required");
        }
        if let Some(max) = self.custom_options.max {
            tag.attr(
                "max",
                max.format(BROWSER_DATETIME_WITHOUT_SEC_FMT).to_string(),
            );
        }
        if let Some(min) = self.custom_options.min {
            tag.attr(
                "min",
                min.format(BROWSER_DATETIME_WITHOUT_SEC_FMT).to_string(),
            );
        }

        if let Some(readonly) = self.custom_options.readonly {
            if readonly {
                tag.bool_attr("readonly");
            }
        }

        if let Some(value) = &self.value {
            tag.attr("value", value);
        }

        if let Some(step) = &self.custom_options.step {
            let step_value = match step {
                Step::Any => "any".to_string(),
                Step::Value(v) => v.num_seconds().to_string(),
            };
            tag.attr("step", step_value);
        }

        write!(f, "{}", tag.render())
    }
}

impl AsFormField for NaiveDateTime {
    type Type = DateTimeField;

    fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError>
    where
        Self: Sized,
    {
        let value = check_required(field)?;
        let date_time = parse_datetime_with_fallback(value)?;
        let opts = &field.custom_options;

        if let Some(min) = &opts.min {
            if date_time < *min {
                return Err(FormFieldValidationError::minimum_value_not_met(min));
            }
        }

        if let Some(max) = &opts.max {
            if date_time > *max {
                return Err(FormFieldValidationError::maximum_value_exceeded(max));
            }
        }

        Ok(date_time)
    }

    fn to_field_value(&self) -> String {
        self.to_string()
    }
}

impl HtmlSafe for DateTimeField {}

impl_form_field!(
    DateTimeWithTimezoneField,
    DateTimeWithTimezoneFieldOptions,
    "a datetime with timezone"
);

impl From<ParseError> for FormFieldValidationError {
    fn from(error: ParseError) -> Self {
        FormFieldValidationError::from_string(error.to_string())
    }
}

/// Custom options for [`DateTimeWithTimezoneField`]
///
/// Specifies the HTML attributes applied to a `datetime‐local` input, plus
/// a user‐specified timezone and DST‐disambiguation policy.
///
/// # Example
///
/// ```
/// use chrono::Duration;
/// use chrono_tz::Tz;
/// use cot::form::fields::{
///     DateTimeWithTimezoneField, DateTimeWithTimezoneFieldOptions, Step,
/// };
/// use cot::form::{FormField, FormFieldOptions};
///
/// // Suppose we want America/New_York timezone with DST handling, and if there's a DST fold,
/// // we choose the later offset (i.e. `prefer_latest = true`).
/// let tz: Tz = "America/New_York".parse().unwrap();
///
/// let options = DateTimeWithTimezoneFieldOptions {
///     min: None,
///     max: None,
///     readonly: Some(false),
///     step: Some(Step::Value(Duration::seconds(60))),
///     timezone: Some(tz),
///     // If the given local time is ambiguous (DST fall‐back), pick the later of the two possibilities.
///     prefer_latest: Some(true),
/// };
///
/// let field = DateTimeWithTimezoneField::with_options(
///     FormFieldOptions {
///         id: "dt".into(),
///         name: "dt".into(),
///         required: true,
///     },
///     options,
/// );
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct DateTimeWithTimezoneFieldOptions {
    /// The maximum allowed datetime (with offset) for this field.
    ///
    /// When present, sets the `max` attribute on `<input
    /// type="datetime-local">`, and also enforces `datetime > max` at
    /// validation time.
    pub max: Option<DateTime<FixedOffset>>,

    /// The minimum allowed datetime (with offset) for this field.
    ///
    /// When present, sets the `min` attribute on `<input
    /// type="datetime-local">`, and also enforces `datetime < min` at
    /// validation time.
    pub min: Option<DateTime<FixedOffset>>,

    /// Whether the field should be read‐only.
    ///
    /// When `Some(true)`, we render `readonly="true"` on the HTML tag,
    /// preventing user edits. When `None` or `Some(false)`, the input is
    /// editable.
    pub readonly: Option<bool>,

    /// The increment (in seconds) between valid datetimes.
    ///
    /// Corresponds to the `step` attribute on `<input type="datetime-local">`.
    /// If `None`, the browser’s default (60 seconds) is used. To override,
    /// supply `Step::Value(Duration)` where `Duration` indicates the number
    /// of seconds per “tick.”
    ///
    /// [`step` attribute]: https://developer.mozilla.org/en-US/docs/Web/HTML/Element/input/datetime-local#step
    pub step: Option<Step<Duration>>,

    /// The timezone to use when parsing the browser‐supplied string.
    ///
    /// Browsers send a naive "YYYY-MM-DDThh:mm" (no offset). If `timezone` is
    /// `Some(tz)`, we convert that naive time into a `DateTime<FixedOffset>`
    /// using `tz.from_local_datetime(...)`. The timezone should be a
    /// [`Tz`] which is capable of handling true DST transitions and
    /// timezone rules. If `None`, defaults to UTC.
    pub timezone: Option<Tz>,

    /// Choose how to handle ambiguous local time conversion (e.g. during a DST
    /// fall-back).
    ///
    /// - `Some(true)`:  always pick the **later** of the two possible instants
    ///   (DST time).
    /// - `Some(false)`: always pick the **earlier** of the two possible
    ///   instants (standard time).
    /// - `None`:         treat an ambiguous local time as a validation error.
    pub prefer_latest: Option<bool>,
}

impl Display for DateTimeWithTimezoneField {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut tag = HtmlTag::input("datetime-local");
        tag.attr("name", self.id());
        tag.attr("id", self.id());

        if self.options.required {
            tag.bool_attr("required");
        }
        if let Some(max) = self.custom_options.max {
            tag.attr(
                "max",
                max.naive_local()
                    .format(BROWSER_DATETIME_WITHOUT_SEC_FMT)
                    .to_string(),
            );
        }
        if let Some(min) = self.custom_options.min {
            tag.attr(
                "min",
                min.naive_local()
                    .format(BROWSER_DATETIME_WITHOUT_SEC_FMT)
                    .to_string(),
            );
        }

        if let Some(readonly) = self.custom_options.readonly {
            if readonly {
                tag.bool_attr("readonly");
            }
        }

        if let Some(value) = &self.value {
            tag.attr("value", value);
        }

        if let Some(step) = &self.custom_options.step {
            let step_value = match step {
                Step::Any => "any".to_string(),
                Step::Value(v) => v.num_seconds().to_string(),
            };
            tag.attr("step", step_value);
        }

        write!(f, "{}", tag.render())
    }
}

impl AsFormField for DateTime<FixedOffset> {
    type Type = DateTimeWithTimezoneField;

    fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError>
    where
        Self: Sized,
    {
        let value = check_required(field)?;
        // Browsers only support naive datetime.
        let naive = parse_datetime_with_fallback(value)?;
        // default to UTC if offset(timezone) is not provided.
        let tz = field.custom_options.timezone.unwrap_or(Tz::UTC);

        let date_time = match tz.from_local_datetime(&naive) {
            LocalResult::Single(dt) => dt,
            LocalResult::Ambiguous(dt1, dt2) => {
                if let Some(prefer_latest) = field.custom_options.prefer_latest {
                    if prefer_latest { dt2 } else { dt1 }
                } else {
                    return Err(FormFieldValidationError::ambiguous_datetime(naive));
                }
            }
            LocalResult::None => {
                return Err(FormFieldValidationError::non_existent_local_datetime(
                    naive, tz,
                ));
            }
        };

        let opts = &field.custom_options;
        // transform the timezone into a fixed offset.
        let date_time = date_time.with_timezone(&date_time.offset().fix());

        if let Some(min) = &opts.min {
            if date_time < *min {
                return Err(FormFieldValidationError::minimum_value_not_met(min));
            }
        }

        if let Some(max) = &opts.max {
            if date_time > *max {
                return Err(FormFieldValidationError::maximum_value_exceeded(max));
            }
        }

        Ok(date_time)
    }

    fn to_field_value(&self) -> String {
        self.to_string()
    }
}

impl HtmlSafe for DateTimeWithTimezoneField {}

impl_form_field!(TimeField, TimeFieldOptions, "a time");

/// Custom options for [`TimeField`]
///
/// Defines the HTML attributes applied to a time-only input.
///
/// # Example
///
/// ```
/// use chrono::{Duration, NaiveTime};
/// use cot::form::fields::{Step, TimeField, TimeFieldOptions};
/// use cot::form::{FormField, FormFieldOptions};
///
/// let options = TimeFieldOptions {
///     min: Some(NaiveTime::from_hms_opt(9, 0, 0).unwrap()),
///     max: Some(NaiveTime::from_hms_opt(17, 0, 0).unwrap()),
///     readonly: Some(true),
///     step: Some(Step::Value(Duration::seconds(900))),
/// };
///
/// let field = TimeField::with_options(
///     FormFieldOptions {
///         id: "event_time".into(),
///         name: "event_time".into(),
///         required: true,
///     },
///     options,
/// );
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct TimeFieldOptions {
    /// The maximum time value of the field used to set the `max` attribute
    /// in the HTML input element.
    pub max: Option<NaiveTime>,
    /// The minimum time value of the field used to set the `min` attribute
    /// in the HTML input element.
    pub min: Option<NaiveTime>,
    /// Whether the field should be read-only. When set to `true`, the user
    /// cannot modify the field value through the HTML input element.
    pub readonly: Option<bool>,
    /// The increment (in seconds) between valid time values.
    ///
    /// Corresponds to the `step` attribute on `<input type="time">`. If `None`,
    /// the browser’s default (60 seconds) is used. To override, supply
    /// `Step::Value(Duration)` where `Duration` is the number of seconds
    /// between ticks.
    ///
    /// [`step` attribute]: https://developer.mozilla.org/en-US/docs/Web/HTML/Element/input/time#step
    pub step: Option<Step<Duration>>,
}

impl Display for TimeField {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut tag = HtmlTag::input("time");
        tag.attr("name", self.id());
        tag.attr("id", self.id());
        if self.options.required {
            tag.bool_attr("required");
        }
        if let Some(max) = self.custom_options.max {
            tag.attr("max", max.to_string());
        }
        if let Some(min) = self.custom_options.min {
            tag.attr("min", min.to_string());
        }
        if let Some(value) = &self.value {
            tag.attr("value", value);
        }

        if let Some(readonly) = self.custom_options.readonly {
            if readonly {
                tag.bool_attr("readonly");
            }
        }

        if let Some(step) = &self.custom_options.step {
            let step_value = match step {
                Step::Any => "any".to_string(),
                Step::Value(v) => v.num_seconds().to_string(),
            };
            tag.attr("step", step_value);
        }

        write!(f, "{}", tag.render())
    }
}

impl AsFormField for NaiveTime {
    type Type = TimeField;

    fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError>
    where
        Self: Sized,
    {
        let value = check_required(field)?;
        let time = parse_time_with_fallback(value)?;
        let opts = &field.custom_options;

        if let Some(min) = &opts.min {
            if time < *min {
                return Err(FormFieldValidationError::minimum_value_not_met(min));
            }
        }

        if let Some(max) = &opts.max {
            if time > *max {
                return Err(FormFieldValidationError::maximum_value_exceeded(max));
            }
        }

        Ok(time)
    }

    fn to_field_value(&self) -> String {
        self.to_string()
    }
}

impl HtmlSafe for TimeField {}

impl_form_field!(DateField, DateFieldOptions, "a date");

/// Custom options for [`DateField`]
///
/// Defines the HTML attributes for a date-only input: the allowable range,
/// whether it is editable, and the day interval between selectable dates.
///
/// # Example
///
/// ```
/// use chrono::{Duration, NaiveDate};
/// use cot::form::fields::{DateField, DateFieldOptions, Step};
/// use cot::form::{FormField, FormFieldOptions};
///
/// let options = DateFieldOptions {
///     min: Some(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()),
///     max: Some(NaiveDate::from_ymd_opt(2025, 12, 31).unwrap()),
///     readonly: None,
///     step: Some(Step::Value(Duration::days(7))),
/// };
///
/// let field = DateField::with_options(
///     FormFieldOptions {
///         id: "event_time".into(),
///         name: "event_time".into(),
///         required: true,
///     },
///     options,
/// );
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct DateFieldOptions {
    /// The maximum date value of the field used to set the `max` attribute
    /// in the HTML input element.
    pub max: Option<NaiveDate>,
    /// The minimum date value of the field used to set the `min` attribute
    /// in the HTML input element.
    pub min: Option<NaiveDate>,
    /// Whether the field should be read-only. When set to `true`, the user
    /// cannot modify the field value through the HTML input element.
    pub readonly: Option<bool>,
    /// The increment (in days) between valid date values.
    ///
    /// Corresponds to the `step` attribute on `<input type="date">`. If `None`,
    /// the browser’s default (1 day) is used. To override, supply
    /// `Step::Value(Duration)` where `Duration` represents the number of
    /// days per step (e.g., `Duration::days(7)`).
    ///
    /// [`step` attribute]: https://developer.mozilla.org/en-US/docs/Web/HTML/Element/input/date#step
    pub step: Option<Step<Duration>>,
}

impl Display for DateField {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut tag = HtmlTag::input("date");
        tag.attr("name", self.id());
        tag.attr("id", self.id());
        if self.options.required {
            tag.bool_attr("required");
        }
        if let Some(max) = self.custom_options.max {
            tag.attr("max", max.to_string());
        }
        if let Some(min) = self.custom_options.min {
            tag.attr("min", min.to_string());
        }
        if let Some(value) = &self.value {
            tag.attr("value", value);
        }

        if let Some(readonly) = self.custom_options.readonly {
            if readonly {
                tag.bool_attr("readonly");
            }
        }

        if let Some(step) = &self.custom_options.step {
            let step_value = match step {
                Step::Any => "any".to_string(),
                Step::Value(v) => v.num_days().to_string(),
            };
            tag.attr("step", step_value);
        }

        write!(f, "{}", tag.render())
    }
}

impl AsFormField for NaiveDate {
    type Type = DateField;

    fn clean_value(field: &Self::Type) -> Result<Self, FormFieldValidationError>
    where
        Self: Sized,
    {
        let value = check_required(field)?;
        let date = NaiveDate::parse_from_str(value, BROWSER_DATE_FMT)
            .map_err(|err| FormFieldValidationError::from_string(err.to_string()))?;
        let opts = &field.custom_options;

        if let Some(min) = &opts.min {
            if date < *min {
                return Err(FormFieldValidationError::minimum_value_not_met(min));
            }
        }

        if let Some(max) = &opts.max {
            if date > *max {
                return Err(FormFieldValidationError::maximum_value_exceeded(max));
            }
        }

        Ok(date)
    }

    fn to_field_value(&self) -> String {
        self.to_string()
    }
}

impl HtmlSafe for DateField {}

#[cfg(test)]
mod tests {
    use std::collections::{HashSet, LinkedList, VecDeque};

    use chrono::Weekday;
    use cot::form::FormFieldValue;

    use super::*;
    use crate::form::fields::{SelectFieldOptions, SelectMultipleFieldOptions};
    use crate::form::{FormField, FormFieldOptions};

    #[test]
    fn weekday_select_choice_default_choices() {
        let choices = Weekday::default_choices();
        assert_eq!(choices.len(), 7);
        assert_eq!(choices[0], Weekday::Mon);
        assert_eq!(choices[1], Weekday::Tue);
        assert_eq!(choices[2], Weekday::Wed);
        assert_eq!(choices[3], Weekday::Thu);
        assert_eq!(choices[4], Weekday::Fri);
        assert_eq!(choices[5], Weekday::Sat);
        assert_eq!(choices[6], Weekday::Sun);
    }

    #[test]
    fn weekday_select_choice_from_str_valid() {
        assert_eq!(Weekday::from_str("mon").unwrap(), Weekday::Mon);
        assert_eq!(Weekday::from_str("tue").unwrap(), Weekday::Tue);
        assert_eq!(Weekday::from_str("wed").unwrap(), Weekday::Wed);
        assert_eq!(Weekday::from_str("thu").unwrap(), Weekday::Thu);
        assert_eq!(Weekday::from_str("fri").unwrap(), Weekday::Fri);
        assert_eq!(Weekday::from_str("sat").unwrap(), Weekday::Sat);
        assert_eq!(Weekday::from_str("sun").unwrap(), Weekday::Sun);
    }

    #[test]
    fn weekday_select_choice_from_str_case_insensitive() {
        assert_eq!(Weekday::from_str("MON").unwrap(), Weekday::Mon);
        assert_eq!(Weekday::from_str("TUE").unwrap(), Weekday::Tue);
        assert_eq!(Weekday::from_str("Wed").unwrap(), Weekday::Wed);
        assert_eq!(Weekday::from_str("THU").unwrap(), Weekday::Thu);
        assert_eq!(Weekday::from_str("Fri").unwrap(), Weekday::Fri);
        assert_eq!(Weekday::from_str("SAT").unwrap(), Weekday::Sat);
        assert_eq!(Weekday::from_str("Sun").unwrap(), Weekday::Sun);
    }

    #[test]
    fn weekday_select_choice_from_str_invalid() {
        let result = Weekday::from_str("invalid");
        assert!(result.is_err());
        if let Err(FormFieldValidationError::InvalidValue(value)) = result {
            assert_eq!(value, "invalid");
        } else {
            panic!("Expected InvalidValue error");
        }
    }

    #[test]
    fn weekday_select_choice_id() {
        assert_eq!(Weekday::Mon.id(), "mon");
        assert_eq!(Weekday::Tue.id(), "tue");
        assert_eq!(Weekday::Wed.id(), "wed");
        assert_eq!(Weekday::Thu.id(), "thu");
        assert_eq!(Weekday::Fri.id(), "fri");
        assert_eq!(Weekday::Sat.id(), "sat");
        assert_eq!(Weekday::Sun.id(), "sun");
    }

    #[test]
    fn weekday_select_choice_to_string() {
        assert_eq!(SelectChoice::to_string(&Weekday::Mon), "Monday");
        assert_eq!(SelectChoice::to_string(&Weekday::Tue), "Tuesday");
        assert_eq!(SelectChoice::to_string(&Weekday::Wed), "Wednesday");
        assert_eq!(SelectChoice::to_string(&Weekday::Thu), "Thursday");
        assert_eq!(SelectChoice::to_string(&Weekday::Fri), "Friday");
        assert_eq!(SelectChoice::to_string(&Weekday::Sat), "Saturday");
        assert_eq!(SelectChoice::to_string(&Weekday::Sun), "Sunday");
    }

    #[cot::test]
    async fn weekday_as_form_field_clean_value() {
        let mut field = SelectField::<Weekday>::with_options(
            FormFieldOptions {
                id: "weekday".to_owned(),
                name: "weekday".to_owned(),
                required: true,
            },
            SelectFieldOptions::default(),
        );

        field
            .set_value(FormFieldValue::new_text("fri"))
            .await
            .unwrap();

        let weekday = Weekday::clean_value(&field).unwrap();
        assert_eq!(weekday, Weekday::Fri);
    }

    #[cot::test]
    async fn weekday_as_form_field_clean_value_invalid() {
        let mut field = SelectField::<Weekday>::with_options(
            FormFieldOptions {
                id: "weekday".to_owned(),
                name: "weekday".to_owned(),
                required: true,
            },
            SelectFieldOptions::default(),
        );

        field
            .set_value(FormFieldValue::new_text("invalid_day"))
            .await
            .unwrap();

        let result = Weekday::clean_value(&field);
        assert!(result.is_err());
        if let Err(FormFieldValidationError::InvalidValue(value)) = result {
            assert_eq!(value, "invalid_day");
        } else {
            panic!("Expected InvalidValue error");
        }
    }

    #[cot::test]
    async fn weekday_as_form_field_clean_value_required_empty() {
        let mut field = SelectField::<Weekday>::with_options(
            FormFieldOptions {
                id: "weekday".to_owned(),
                name: "weekday".to_owned(),
                required: true,
            },
            SelectFieldOptions::default(),
        );

        field.set_value(FormFieldValue::new_text("")).await.unwrap();

        let result = Weekday::clean_value(&field);
        assert_eq!(result, Err(FormFieldValidationError::Required));
    }

    #[test]
    fn weekday_as_form_field_to_field_value() {
        assert_eq!(Weekday::Mon.to_field_value(), "Monday");
        assert_eq!(Weekday::Wed.to_field_value(), "Wednesday");
        assert_eq!(Weekday::Sun.to_field_value(), "Sunday");
    }

    #[cot::test]
    async fn weekday_vec_as_form_field_clean_value() {
        let mut field = SelectMultipleField::<Weekday>::with_options(
            FormFieldOptions {
                id: "weekdays".to_owned(),
                name: "weekdays".to_owned(),
                required: true,
            },
            SelectMultipleFieldOptions::default(),
        );

        field
            .set_value(FormFieldValue::new_text("mon"))
            .await
            .unwrap();
        field
            .set_value(FormFieldValue::new_text("wed"))
            .await
            .unwrap();
        field
            .set_value(FormFieldValue::new_text("fri"))
            .await
            .unwrap();

        let weekdays = Vec::<Weekday>::clean_value(&field).unwrap();
        assert_eq!(weekdays.len(), 3);
        assert!(weekdays.contains(&Weekday::Mon));
        assert!(weekdays.contains(&Weekday::Wed));
        assert!(weekdays.contains(&Weekday::Fri));
    }

    #[cot::test]
    async fn weekday_vec_as_form_field_clean_value_empty_required() {
        let field = SelectMultipleField::<Weekday>::with_options(
            FormFieldOptions {
                id: "weekdays".to_owned(),
                name: "weekdays".to_owned(),
                required: true,
            },
            SelectMultipleFieldOptions::default(),
        );

        let result = Vec::<Weekday>::clean_value(&field);
        assert_eq!(result, Err(FormFieldValidationError::Required));
    }

    #[cot::test]
    async fn weekday_vec_as_form_field_clean_value_invalid() {
        let mut field = SelectMultipleField::<Weekday>::with_options(
            FormFieldOptions {
                id: "weekdays".to_owned(),
                name: "weekdays".to_owned(),
                required: false,
            },
            SelectMultipleFieldOptions::default(),
        );

        field
            .set_value(FormFieldValue::new_text("mon"))
            .await
            .unwrap();
        field
            .set_value(FormFieldValue::new_text("invalid_day"))
            .await
            .unwrap();

        let result = Vec::<Weekday>::clean_value(&field);
        assert!(result.is_err());
        if let Err(FormFieldValidationError::InvalidValue(value)) = result {
            assert_eq!(value, "invalid_day");
        } else {
            panic!("Expected InvalidValue error");
        }
    }

    #[test]
    fn weekday_vec_as_form_field_to_field_value() {
        let weekdays = vec![Weekday::Mon, Weekday::Wed, Weekday::Fri];
        assert_eq!(weekdays.to_field_value(), "");
    }

    #[cot::test]
    async fn weekday_hash_set_as_form_field_clean_value() {
        let mut field = SelectMultipleField::<Weekday>::with_options(
            FormFieldOptions {
                id: "weekdays".to_owned(),
                name: "weekdays".to_owned(),
                required: false,
            },
            SelectMultipleFieldOptions::default(),
        );

        field
            .set_value(FormFieldValue::new_text("tue"))
            .await
            .unwrap();
        field
            .set_value(FormFieldValue::new_text("thu"))
            .await
            .unwrap();

        let weekdays = HashSet::<Weekday>::clean_value(&field).unwrap();
        assert_eq!(weekdays.len(), 2);
        assert!(weekdays.contains(&Weekday::Tue));
        assert!(weekdays.contains(&Weekday::Thu));
    }

    #[cot::test]
    async fn weekday_vec_deque_as_form_field_clean_value() {
        let mut field = SelectMultipleField::<Weekday>::with_options(
            FormFieldOptions {
                id: "weekdays".to_owned(),
                name: "weekdays".to_owned(),
                required: false,
            },
            SelectMultipleFieldOptions::default(),
        );

        field
            .set_value(FormFieldValue::new_text("sat"))
            .await
            .unwrap();
        field
            .set_value(FormFieldValue::new_text("sun"))
            .await
            .unwrap();

        let weekdays = VecDeque::<Weekday>::clean_value(&field).unwrap();
        assert_eq!(weekdays.len(), 2);
        assert!(weekdays.contains(&Weekday::Sat));
        assert!(weekdays.contains(&Weekday::Sun));
    }

    #[cot::test]
    async fn weekday_linked_list_as_form_field_clean_value() {
        let mut field = SelectMultipleField::<Weekday>::with_options(
            FormFieldOptions {
                id: "weekdays".to_owned(),
                name: "weekdays".to_owned(),
                required: false,
            },
            SelectMultipleFieldOptions::default(),
        );

        field
            .set_value(FormFieldValue::new_text("mon"))
            .await
            .unwrap();

        let weekdays = LinkedList::<Weekday>::clean_value(&field).unwrap();
        assert_eq!(weekdays.len(), 1);
        assert!(weekdays.contains(&Weekday::Mon));
    }

    #[cot::test]
    async fn weekday_index_set_as_form_field_clean_value() {
        let mut field = SelectMultipleField::<Weekday>::with_options(
            FormFieldOptions {
                id: "weekdays".to_owned(),
                name: "weekdays".to_owned(),
                required: false,
            },
            SelectMultipleFieldOptions::default(),
        );

        field
            .set_value(FormFieldValue::new_text("wed"))
            .await
            .unwrap();
        field
            .set_value(FormFieldValue::new_text("fri"))
            .await
            .unwrap();
        field
            .set_value(FormFieldValue::new_text("wed")) // duplicate
            .await
            .unwrap();

        let weekdays = indexmap::IndexSet::<Weekday>::clean_value(&field).unwrap();
        assert_eq!(weekdays.len(), 2); // Should deduplicate
        assert!(weekdays.contains(&Weekday::Wed));
        assert!(weekdays.contains(&Weekday::Fri));
    }

    #[cot::test]
    async fn weekday_set_as_form_field_clean_value() {
        let mut field = SelectMultipleField::<Weekday>::with_options(
            FormFieldOptions {
                id: "weekdays".to_owned(),
                name: "weekdays".to_owned(),
                required: false,
            },
            SelectMultipleFieldOptions::default(),
        );

        field
            .set_value(FormFieldValue::new_text("mon"))
            .await
            .unwrap();
        field
            .set_value(FormFieldValue::new_text("tue"))
            .await
            .unwrap();
        field
            .set_value(FormFieldValue::new_text("fri"))
            .await
            .unwrap();

        let weekday_set = WeekdaySet::clean_value(&field).unwrap();
        assert!(weekday_set.contains(Weekday::Mon));
        assert!(weekday_set.contains(Weekday::Tue));
        assert!(!weekday_set.contains(Weekday::Wed));
        assert!(!weekday_set.contains(Weekday::Thu));
        assert!(weekday_set.contains(Weekday::Fri));
        assert!(!weekday_set.contains(Weekday::Sat));
        assert!(!weekday_set.contains(Weekday::Sun));
    }

    #[test]
    fn weekday_set_as_form_field_to_field_value() {
        let weekday_set = WeekdaySet::from_array([Weekday::Mon, Weekday::Fri]);
        assert_eq!(weekday_set.to_field_value(), "");
    }

    #[test]
    fn weekday_select_field_render() {
        let field = SelectField::<Weekday>::with_options(
            FormFieldOptions {
                id: "weekday".to_owned(),
                name: "weekday".to_owned(),
                required: false,
            },
            SelectFieldOptions::default(),
        );

        let html = field.to_string();
        assert!(html.contains("<select"));
        assert!(html.contains("name=\"weekday\""));
        assert!(html.contains("id=\"weekday\""));
        assert!(html.contains("Monday"));
        assert!(html.contains("Tuesday"));
        assert!(html.contains("Wednesday"));
        assert!(html.contains("Thursday"));
        assert!(html.contains("Friday"));
        assert!(html.contains("Saturday"));
        assert!(html.contains("Sunday"));
        assert!(html.contains("value=\"mon\""));
        assert!(html.contains("value=\"tue\""));
        assert!(html.contains("value=\"wed\""));
        assert!(html.contains("value=\"thu\""));
        assert!(html.contains("value=\"fri\""));
        assert!(html.contains("value=\"sat\""));
        assert!(html.contains("value=\"sun\""));
    }

    #[test]
    fn weekday_select_multiple_field_render() {
        let field = SelectMultipleField::<Weekday>::with_options(
            FormFieldOptions {
                id: "weekdays".to_owned(),
                name: "weekdays".to_owned(),
                required: false,
            },
            SelectMultipleFieldOptions::default(),
        );

        let html = field.to_string();
        assert!(html.contains("<select"));
        assert!(html.contains("multiple"));
        assert!(html.contains("name=\"weekdays\""));
        assert!(html.contains("id=\"weekdays\""));
        assert!(html.contains("Monday"));
        assert!(html.contains("Friday"));
        assert!(html.contains("Sunday"));
    }

    #[test]
    fn datetime_field_render() {
        let field = DateTimeField::with_options(
            FormFieldOptions {
                id: "dt".into(),
                name: "dt".into(),
                required: true,
            },
            DateTimeFieldOptions {
                min: Some(
                    NaiveDateTime::parse_from_str("2025-05-27T00:00:00", "%Y-%m-%dT%H:%M:%S")
                        .unwrap(),
                ),
                max: Some(
                    NaiveDateTime::parse_from_str("2025-05-28T00:00:00", "%Y-%m-%dT%H:%M:%S")
                        .unwrap(),
                ),
                readonly: Some(true),
                step: Some(Step::Value(Duration::seconds(60))),
            },
        );
        let html = field.to_string();
        assert!(html.contains("type=\"datetime-local\""));
        assert!(html.contains("name=\"dt\""));
        assert!(html.contains("id=\"dt\""));
        assert!(html.contains("required"));
        assert!(html.contains("readonly"));
        assert!(html.contains("min=\"2025-05-27T00:00\""));
        assert!(html.contains("max=\"2025-05-28T00:00\""));
        assert!(html.contains("step=\"60\""));
    }
    #[cot::test]
    async fn datetime_field_clean_valid() {
        let mut field = DateTimeField::with_options(
            FormFieldOptions {
                id: "dt".into(),
                name: "dt".into(),
                required: true,
            },
            DateTimeFieldOptions {
                min: Some(
                    NaiveDateTime::parse_from_str("2025-05-27T00:00:00", "%Y-%m-%dT%H:%M:%S")
                        .unwrap(),
                ),
                max: Some(
                    NaiveDateTime::parse_from_str("2025-05-28T00:00:00", "%Y-%m-%dT%H:%M:%S")
                        .unwrap(),
                ),
                readonly: None,
                step: Some(Step::Value(Duration::seconds(60))),
            },
        );

        for &dt in &["2025-05-27T12:34", "2025-05-27T12:34:00"] {
            field.set_value(FormFieldValue::new_text(dt)).await.unwrap();
            let dt = NaiveDateTime::clean_value(&field).unwrap();
            assert_eq!(dt.to_string(), "2025-05-27 12:34:00");
        }
    }

    #[cot::test]
    async fn datetime_field_clean_below_min() {
        let mut field = DateTimeField::with_options(
            FormFieldOptions {
                id: "dt".into(),
                name: "dt".into(),
                required: true,
            },
            DateTimeFieldOptions {
                min: Some(
                    NaiveDateTime::parse_from_str("2025-05-27T10:00:00", "%Y-%m-%dT%H:%M:%S")
                        .unwrap(),
                ),
                max: None,
                readonly: None,
                step: Some(Step::Value(Duration::seconds(60))),
            },
        );
        for &dt in &["2025-05-27T09:59", "2025-05-27T09:59:00"] {
            field.set_value(FormFieldValue::new_text(dt)).await.unwrap();
            let err = NaiveDateTime::clean_value(&field).unwrap_err();
            assert!(matches!(
                err,
                FormFieldValidationError::MinimumValueNotMet { .. }
            ));
        }
    }

    #[cot::test]
    async fn datetime_field_clean_above_max() {
        let mut field = DateTimeField::with_options(
            FormFieldOptions {
                id: "dt".into(),
                name: "dt".into(),
                required: true,
            },
            DateTimeFieldOptions {
                min: None,
                max: Some(
                    NaiveDateTime::parse_from_str("2025-05-27T10:00:00", "%Y-%m-%dT%H:%M:%S")
                        .unwrap(),
                ),
                readonly: None,
                step: Some(Step::Value(Duration::seconds(60))),
            },
        );
        for &dt in &["2025-05-27T10:01", "2025-05-27T10:01:00"] {
            field.set_value(FormFieldValue::new_text(dt)).await.unwrap();
            let err = NaiveDateTime::clean_value(&field).unwrap_err();
            assert!(matches!(
                err,
                FormFieldValidationError::MaximumValueExceeded { .. }
            ));
        }
    }

    #[test]
    fn datetime_with_tz_field_render() {
        let field = DateTimeWithTimezoneField::with_options(
            FormFieldOptions {
                id: "dt".into(),
                name: "dt".into(),
                required: true,
            },
            DateTimeWithTimezoneFieldOptions {
                min: Some(
                    DateTime::parse_from_str("2025-05-27T00:00:00 +0000", "%Y-%m-%dT%H:%M:%S %z")
                        .unwrap(),
                ),
                max: Some(
                    DateTime::parse_from_str("2025-05-28T00:00:00 +0000", "%Y-%m-%dT%H:%M:%S %z")
                        .unwrap(),
                ),
                readonly: Some(true),
                step: Some(Step::Value(Duration::seconds(60))),
                timezone: None,
                prefer_latest: None,
            },
        );
        let html = field.to_string();
        assert_eq!(
            html,
            "<input type=\"datetime-local\" name=\"dt\" id=\"dt\" max=\"2025-05-28T00:00\" min=\"2025-05-27T00:00\" step=\"60\" required readonly/>"
        );
    }

    #[cot::test]
    async fn datetime_with_tz_clean_valid_default_utc() {
        let mut field = DateTimeWithTimezoneField::with_options(
            FormFieldOptions {
                id: "dt".into(),
                name: "dt".into(),
                required: true,
            },
            DateTimeWithTimezoneFieldOptions {
                min: None,
                max: None,
                readonly: None,
                step: None,
                timezone: None,
                prefer_latest: None,
            },
        );
        field
            .set_value(FormFieldValue::new_text("2025-05-27T12:34"))
            .await
            .unwrap();

        let dt = DateTime::<FixedOffset>::clean_value(&field).unwrap();
        assert_eq!(dt.to_rfc3339(), "2025-05-27T12:34:00+00:00");
    }

    #[cot::test]
    async fn datetime_with_tz_clean_valid_custom_offset() {
        let offset = Tz::America__New_York;
        let mut field = DateTimeWithTimezoneField::with_options(
            FormFieldOptions {
                id: "dt".into(),
                name: "dt".into(),
                required: true,
            },
            DateTimeWithTimezoneFieldOptions {
                min: None,
                max: None,
                readonly: None,
                step: None,
                timezone: Some(offset),
                prefer_latest: None,
            },
        );
        field
            .set_value(FormFieldValue::new_text("2025-05-27T01:23"))
            .await
            .unwrap();

        let dt = DateTime::<FixedOffset>::clean_value(&field).unwrap();
        assert_eq!(dt.to_rfc3339(), "2025-05-27T01:23:00-04:00");
    }

    #[cot::test]
    async fn datetime_with_tz_clean_ambiguous_time_prefer_earliest() {
        let offset = Tz::America__New_York;
        let mut field = DateTimeWithTimezoneField::with_options(
            FormFieldOptions {
                id: "dt".into(),
                name: "dt".into(),
                required: true,
            },
            DateTimeWithTimezoneFieldOptions {
                min: None,
                max: None,
                readonly: None,
                step: None,
                timezone: Some(offset),
                prefer_latest: Some(false),
            },
        );
        field
            .set_value(FormFieldValue::new_text("2024-11-03T01:30"))
            .await
            .unwrap();

        let dt = DateTime::<FixedOffset>::clean_value(&field).unwrap();
        assert_eq!(dt.to_rfc3339(), "2024-11-03T01:30:00-04:00");
    }

    #[cot::test]
    async fn datetime_with_tz_clean_ambiguous_time_prefer_latest() {
        let offset = Tz::America__New_York;
        let mut field = DateTimeWithTimezoneField::with_options(
            FormFieldOptions {
                id: "dt".into(),
                name: "dt".into(),
                required: true,
            },
            DateTimeWithTimezoneFieldOptions {
                min: None,
                max: None,
                readonly: None,
                step: None,
                timezone: Some(offset),
                prefer_latest: Some(true),
            },
        );
        field
            .set_value(FormFieldValue::new_text("2024-11-03T01:30"))
            .await
            .unwrap();

        let dt = DateTime::<FixedOffset>::clean_value(&field).unwrap();
        assert_eq!(dt.to_rfc3339(), "2024-11-03T01:30:00-05:00");
    }

    #[cot::test]
    async fn datetime_with_tz_clean_ambiguous_time_unhandled() {
        let offset = Tz::America__New_York;
        let mut field = DateTimeWithTimezoneField::with_options(
            FormFieldOptions {
                id: "dt".into(),
                name: "dt".into(),
                required: true,
            },
            DateTimeWithTimezoneFieldOptions {
                min: None,
                max: None,
                readonly: None,
                step: None,
                timezone: Some(offset),
                prefer_latest: None,
            },
        );
        field
            .set_value(FormFieldValue::new_text("2024-11-03T01:30"))
            .await
            .unwrap();

        let dt = DateTime::<FixedOffset>::clean_value(&field);
        assert!(matches!(
            dt,
            Err(FormFieldValidationError::AmbiguousDateTime { .. })
        ));
    }
    #[cot::test]
    async fn datetime_with_tz_clean_non_existent_local_time() {
        let offset = Tz::America__New_York;
        let mut field = DateTimeWithTimezoneField::with_options(
            FormFieldOptions {
                id: "dt".into(),
                name: "dt".into(),
                required: true,
            },
            DateTimeWithTimezoneFieldOptions {
                min: None,
                max: None,
                readonly: None,
                step: None,
                timezone: Some(offset),
                prefer_latest: None,
            },
        );
        field
            .set_value(FormFieldValue::new_text("2024-03-10T02:30"))
            .await
            .unwrap();

        let dt = DateTime::<FixedOffset>::clean_value(&field);
        assert!(matches!(
            dt,
            Err(FormFieldValidationError::NonExistentLocalDateTime { .. })
        ));
    }

    #[cot::test]
    async fn datetime_with_tz_clean_below_min() {
        let min_dt =
            DateTime::parse_from_str("2025-05-27T10:00:00 +0000", "%Y-%m-%dT%H:%M:%S %z").unwrap();
        let mut field = DateTimeWithTimezoneField::with_options(
            FormFieldOptions {
                id: "dt".into(),
                name: "dt".into(),
                required: true,
            },
            DateTimeWithTimezoneFieldOptions {
                min: Some(min_dt),
                max: None,
                readonly: None,
                step: None,
                timezone: None,
                prefer_latest: None,
            },
        );
        field
            .set_value(FormFieldValue::new_text("2025-05-27T09:59"))
            .await
            .unwrap();
        let err = DateTime::<FixedOffset>::clean_value(&field).unwrap_err();
        assert!(matches!(
            err,
            FormFieldValidationError::MinimumValueNotMet { .. }
        ));
    }

    #[cot::test]
    async fn datetime_with_tz_clean_above_max() {
        let max_dt =
            DateTime::parse_from_str("2025-05-27T10:00:00 +0000", "%Y-%m-%dT%H:%M:%S %z").unwrap();
        let mut field = DateTimeWithTimezoneField::with_options(
            FormFieldOptions {
                id: "dt".into(),
                name: "dt".into(),
                required: true,
            },
            DateTimeWithTimezoneFieldOptions {
                min: None,
                max: Some(max_dt),
                readonly: None,
                step: None,
                timezone: None,
                prefer_latest: None,
            },
        );
        field
            .set_value(FormFieldValue::new_text("2025-05-27T10:01"))
            .await
            .unwrap();
        let err = DateTime::<FixedOffset>::clean_value(&field).unwrap_err();
        assert!(matches!(
            err,
            FormFieldValidationError::MaximumValueExceeded { .. }
        ));
    }

    #[cot::test]
    async fn datetime_with_tz_clean_invalid_format() {
        let mut field = DateTimeWithTimezoneField::with_options(
            FormFieldOptions {
                id: "dt".into(),
                name: "dt".into(),
                required: true,
            },
            DateTimeWithTimezoneFieldOptions {
                min: None,
                max: None,
                readonly: None,
                step: None,
                timezone: None,
                prefer_latest: None,
            },
        );
        field
            .set_value(FormFieldValue::new_text("not-a-valid-datetime"))
            .await
            .unwrap();

        let result = DateTime::<FixedOffset>::clean_value(&field);
        assert!(result.is_err());
    }

    #[cot::test]
    async fn datetime_with_tz_clean_required() {
        let mut field = DateTimeWithTimezoneField::with_options(
            FormFieldOptions {
                id: "dt".into(),
                name: "dt".into(),
                required: true,
            },
            DateTimeWithTimezoneFieldOptions {
                min: None,
                max: None,
                readonly: None,
                step: None,
                timezone: None,
                prefer_latest: None,
            },
        );

        field.set_value(FormFieldValue::new_text("")).await.unwrap();
        let result = DateTime::<FixedOffset>::clean_value(&field);
        assert_eq!(result, Err(FormFieldValidationError::Required));
    }

    #[test]
    fn time_field_render() {
        let field = TimeField::with_options(
            FormFieldOptions {
                id: "time".into(),
                name: "time".into(),
                required: true,
            },
            TimeFieldOptions {
                min: Some(NaiveTime::parse_from_str("09:00:00", "%H:%M:%S").unwrap()),
                max: Some(NaiveTime::parse_from_str("17:00:00", "%H:%M:%S").unwrap()),
                readonly: Some(false),
                step: Some(Step::Value(Duration::seconds(60))),
            },
        );
        let html = field.to_string();
        assert!(html.contains("type=\"time\""));
        assert!(html.contains("name=\"time\""));
        assert!(html.contains("id=\"time\""));
        assert!(html.contains("required"));
        assert!(html.contains("min=\"09:00:00\""));
        assert!(html.contains("max=\"17:00:00\""));
        assert!(html.contains("step=\"60\""));
    }
    #[cot::test]
    async fn time_field_clean_valid() {
        let mut field = TimeField::with_options(
            FormFieldOptions {
                id: "t".into(),
                name: "t".into(),
                required: true,
            },
            TimeFieldOptions {
                min: Some(NaiveTime::parse_from_str("09:00:00", "%H:%M:%S").unwrap()),
                max: Some(NaiveTime::parse_from_str("17:00:00", "%H:%M:%S").unwrap()),
                readonly: Some(false),
                step: Some(Step::Value(Duration::seconds(60))),
            },
        );
        field
            .set_value(FormFieldValue::new_text("12:30"))
            .await
            .unwrap();
        let t = NaiveTime::clean_value(&field).unwrap();
        assert_eq!(t.to_string(), "12:30:00");
    }

    #[cot::test]
    async fn time_field_clean_below_min() {
        let mut field = TimeField::with_options(
            FormFieldOptions {
                id: "t".into(),
                name: "t".into(),
                required: true,
            },
            TimeFieldOptions {
                min: Some(NaiveTime::parse_from_str("09:00:00", "%H:%M:%S").unwrap()),
                max: None,
                readonly: Some(false),
                step: Some(Step::Value(Duration::seconds(60))),
            },
        );
        for &time in &["08:59:00", "08:59"] {
            field
                .set_value(FormFieldValue::new_text(time))
                .await
                .unwrap();
            let err = NaiveTime::clean_value(&field).unwrap_err();
            assert!(matches!(
                err,
                FormFieldValidationError::MinimumValueNotMet { .. }
            ));
        }
    }

    #[cot::test]
    async fn time_field_clean_above_max() {
        let mut field = TimeField::with_options(
            FormFieldOptions {
                id: "t".into(),
                name: "t".into(),
                required: true,
            },
            TimeFieldOptions {
                min: None,
                max: Some(NaiveTime::parse_from_str("17:00:00", "%H:%M:%S").unwrap()),
                readonly: Some(false),
                step: Some(Step::Value(Duration::seconds(60))),
            },
        );

        for &time in &["17:01:00", "17:01"] {
            field
                .set_value(FormFieldValue::new_text(time))
                .await
                .unwrap();
            let err = NaiveTime::clean_value(&field).unwrap_err();
            assert!(matches!(
                err,
                FormFieldValidationError::MaximumValueExceeded { .. }
            ));
        }
    }

    #[test]
    fn date_field_render() {
        let field = DateField::with_options(
            FormFieldOptions {
                id: "d".into(),
                name: "d".into(),
                required: true,
            },
            DateFieldOptions {
                min: Some(NaiveDate::parse_from_str("2025-05-27", "%Y-%m-%d").unwrap()),
                max: Some(NaiveDate::parse_from_str("2025-05-28", "%Y-%m-%d").unwrap()),
                readonly: None,
                step: Some(Step::Value(Duration::days(1))),
            },
        );
        let html = field.to_string();
        assert!(html.contains("type=\"date\""));
        assert!(html.contains("required"));
        assert!(html.contains("min=\"2025-05-27\""));
        assert!(html.contains("max=\"2025-05-28\""));
        assert!(html.contains("step=\"1\""));
    }
    #[cot::test]
    async fn date_field_clean_valid() {
        let mut field = DateField::with_options(
            FormFieldOptions {
                id: "d".into(),
                name: "d".into(),
                required: true,
            },
            DateFieldOptions {
                min: Some(NaiveDate::parse_from_str("2025-05-27", "%Y-%m-%d").unwrap()),
                max: Some(NaiveDate::parse_from_str("2025-05-28", "%Y-%m-%d").unwrap()),
                readonly: None,
                step: Some(Step::Value(Duration::days(1))),
            },
        );
        field
            .set_value(FormFieldValue::new_text("2025-05-27"))
            .await
            .unwrap();
        let d = NaiveDate::clean_value(&field).unwrap();
        assert_eq!(d.to_string(), "2025-05-27");
    }

    #[cot::test]
    async fn date_field_clean_below_min() {
        let mut field = DateField::with_options(
            FormFieldOptions {
                id: "d".into(),
                name: "d".into(),
                required: true,
            },
            DateFieldOptions {
                min: Some(NaiveDate::parse_from_str("2025-05-27", "%Y-%m-%d").unwrap()),
                max: None,
                readonly: None,
                step: Some(Step::Value(Duration::days(1))),
            },
        );
        field
            .set_value(FormFieldValue::new_text("2025-05-26"))
            .await
            .unwrap();
        let err = NaiveDate::clean_value(&field).unwrap_err();
        assert!(matches!(
            err,
            FormFieldValidationError::MinimumValueNotMet { .. }
        ));
    }

    #[cot::test]
    async fn date_field_clean_above_max() {
        let mut field = DateField::with_options(
            FormFieldOptions {
                id: "d".into(),
                name: "d".into(),
                required: true,
            },
            DateFieldOptions {
                min: None,
                max: Some(NaiveDate::parse_from_str("2025-05-27", "%Y-%m-%d").unwrap()),
                readonly: None,
                step: Some(Step::Value(Duration::days(1))),
            },
        );
        field
            .set_value(FormFieldValue::new_text("2025-05-28"))
            .await
            .unwrap();
        let err = NaiveDate::clean_value(&field).unwrap_err();
        assert!(matches!(
            err,
            FormFieldValidationError::MaximumValueExceeded { .. }
        ));
    }
}
