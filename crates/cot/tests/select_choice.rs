use cot::form::fields::SelectChoice;

#[derive(SelectChoice, Debug, PartialEq, Eq)]
enum Status {
    Draft,
    Published,
    Archived,
}

#[derive(SelectChoice, Debug, PartialEq, Eq)]
enum MixedCase {
    FooBar,
    Baz,
    SnakeCase,
}

#[derive(SelectChoice, Debug, PartialEq, Eq)]
enum WithOverrides {
    #[select_choice(id = "custom", name = "Custom Display")]
    Custom,
    #[select_choice(name = "Bar Human")]
    Bar,
    #[select_choice(id = "baz_id")]
    Baz,
    Default,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_ids_and_names() {
        assert_eq!(Status::Draft.id(), "Draft");
        assert_eq!(Status::Draft.to_string(), "Draft");
        assert_eq!(Status::Published.id(), "Published");
        assert_eq!(Status::Published.to_string(), "Published");
        assert_eq!(Status::Archived.id(), "Archived");
        assert_eq!(Status::Archived.to_string(), "Archived");
    }

    #[test]
    fn mixedcase_ids_and_names() {
        assert_eq!(MixedCase::FooBar.id(), "FooBar");
        assert_eq!(MixedCase::FooBar.to_string(), "FooBar");
        assert_eq!(MixedCase::Baz.id(), "Baz");
        assert_eq!(MixedCase::Baz.to_string(), "Baz");
        assert_eq!(MixedCase::SnakeCase.id(), "SnakeCase");
        assert_eq!(MixedCase::SnakeCase.to_string(), "SnakeCase");
    }

    #[test]
    fn with_overrides_ids_and_names() {
        assert_eq!(WithOverrides::Custom.id(), "custom");
        assert_eq!(WithOverrides::Custom.to_string(), "Custom Display");
        assert_eq!(WithOverrides::Bar.id(), "Bar");
        assert_eq!(WithOverrides::Bar.to_string(), "Bar Human");
        assert_eq!(WithOverrides::Baz.id(), "baz_id");
        assert_eq!(WithOverrides::Baz.to_string(), "Baz");
        assert_eq!(WithOverrides::Default.id(), "Default");
        assert_eq!(WithOverrides::Default.to_string(), "Default");
    }

    #[test]
    fn status_default_choices() {
        assert_eq!(
            Status::default_choices(),
            vec![Status::Draft, Status::Published, Status::Archived]
        );
    }

    #[test]
    fn with_overrides_default_choices() {
        assert_eq!(
            WithOverrides::default_choices(),
            vec![
                WithOverrides::Custom,
                WithOverrides::Bar,
                WithOverrides::Baz,
                WithOverrides::Default
            ]
        );
    }

    #[test]
    fn status_id_roundtrip() {
        for value in Status::default_choices() {
            assert_eq!(Status::from_str(&value.id()), Ok(value));
        }
    }

    #[test]
    fn mixedcase_id_roundtrip() {
        for value in MixedCase::default_choices() {
            assert_eq!(MixedCase::from_str(&value.id()), Ok(value));
        }
    }

    #[test]
    fn with_overrides_id_roundtrip() {
        for value in WithOverrides::default_choices() {
            assert_eq!(WithOverrides::from_str(&value.id()), Ok(value));
        }
    }
}
