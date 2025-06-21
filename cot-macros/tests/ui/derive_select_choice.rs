use cot::form::fields::SelectChoice;
use cot_macros::SelectChoice;

#[derive(SelectChoice, Debug, PartialEq, Eq)]
enum Status {
    Draft,
    Published,
    Archived,
}

#[derive(SelectChoice, Debug, PartialEq, Eq)]
enum MixedCase {
    FooBar,
    BAZ,
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

fn main() {
    // Status
    assert_eq!(Status::Draft.id(), "Draft");
    assert_eq!(Status::Published.id(), "Published");
    assert_eq!(Status::Archived.id(), "Archived");

    assert_eq!(Status::Draft.to_string(), "Draft");
    assert_eq!(Status::Published.to_string(), "Published");
    assert_eq!(Status::Archived.to_string(), "Archived");

    // MixedCase
    assert_eq!(MixedCase::FooBar.id(), "FooBar");
    assert_eq!(MixedCase::BAZ.id(), "BAZ");
    assert_eq!(MixedCase::SnakeCase.id(), "SnakeCase");

    assert_eq!(MixedCase::FooBar.to_string(), "FooBar");
    assert_eq!(MixedCase::BAZ.to_string(), "BAZ");
    assert_eq!(MixedCase::SnakeCase.to_string(), "SnakeCase");

    // WithOverrides
    assert_eq!(WithOverrides::Custom.id(), "custom");
    assert_eq!(WithOverrides::Custom.to_string(), "Custom Display");
    assert_eq!(WithOverrides::Bar.id(), "Bar");
    assert_eq!(WithOverrides::Bar.to_string(), "Bar Human");
    assert_eq!(WithOverrides::Baz.id(), "baz_id");
    assert_eq!(WithOverrides::Baz.to_string(), "Baz");
    assert_eq!(WithOverrides::Default.id(), "Default");
    assert_eq!(WithOverrides::Default.to_string(), "Default");

    // default_choices
    assert_eq!(
        Status::default_choices(),
        vec![Status::Draft, Status::Published, Status::Archived]
    );
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
