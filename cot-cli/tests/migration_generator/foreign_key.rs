use cot::db::{model, Auto, ForeignKey};

#[model]
struct Child {
    #[model(primary_key)]
    id: Auto<i32>,
    parent: ForeignKey<Parent>,
}

#[model]
struct Parent {
    #[model(primary_key)]
    id: Auto<i32>,
}

fn main() {}
