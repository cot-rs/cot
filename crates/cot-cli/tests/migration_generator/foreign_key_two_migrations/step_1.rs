use cot::db::{model, Auto, ForeignKey};

#[model]
struct Parent {
    #[model(primary_key)]
    id: Auto<i32>,
}

fn main() {}
