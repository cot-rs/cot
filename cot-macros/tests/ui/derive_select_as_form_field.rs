use cot::form::fields::{SelectChoice, SelectAsFormField};

#[derive(SelectChoice, SelectAsFormField, Debug, Clone, PartialEq, Eq, Hash)]
enum Status {
    Draft,
    Published,
    Archived,
}

fn main() {}
