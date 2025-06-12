use chrono::{Weekday, WeekdaySet};

use crate::form::fields::{
    SelectChoice, SelectField, SelectMultipleField, check_required, check_required_multiple,
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

#[cfg(test)]
mod tests {
    use std::collections::{HashSet, LinkedList, VecDeque};

    use chrono::Weekday;

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
            .set_value(crate::form::FormFieldValue::new_text("fri"))
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
            .set_value(crate::form::FormFieldValue::new_text("invalid_day"))
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

        field
            .set_value(crate::form::FormFieldValue::new_text(""))
            .await
            .unwrap();

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
            .set_value(crate::form::FormFieldValue::new_text("mon"))
            .await
            .unwrap();
        field
            .set_value(crate::form::FormFieldValue::new_text("wed"))
            .await
            .unwrap();
        field
            .set_value(crate::form::FormFieldValue::new_text("fri"))
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
            .set_value(crate::form::FormFieldValue::new_text("mon"))
            .await
            .unwrap();
        field
            .set_value(crate::form::FormFieldValue::new_text("invalid_day"))
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
            .set_value(crate::form::FormFieldValue::new_text("tue"))
            .await
            .unwrap();
        field
            .set_value(crate::form::FormFieldValue::new_text("thu"))
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
            .set_value(crate::form::FormFieldValue::new_text("sat"))
            .await
            .unwrap();
        field
            .set_value(crate::form::FormFieldValue::new_text("sun"))
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
            .set_value(crate::form::FormFieldValue::new_text("mon"))
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
            .set_value(crate::form::FormFieldValue::new_text("wed"))
            .await
            .unwrap();
        field
            .set_value(crate::form::FormFieldValue::new_text("fri"))
            .await
            .unwrap();
        field
            .set_value(crate::form::FormFieldValue::new_text("wed")) // duplicate
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
            .set_value(crate::form::FormFieldValue::new_text("mon"))
            .await
            .unwrap();
        field
            .set_value(crate::form::FormFieldValue::new_text("tue"))
            .await
            .unwrap();
        field
            .set_value(crate::form::FormFieldValue::new_text("fri"))
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
}
