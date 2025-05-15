use cot::db::{Auto, Model, model};
#[model]
struct Test {
    #[model(primary_key)]
    id: Auto<i32>,
}
