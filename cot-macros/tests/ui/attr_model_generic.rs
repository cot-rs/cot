use cot::db::model;

#[model]
struct MyModel<T> {
    #[model(primary_key)]
    id: i32,
    some_data: T,
}

fn main() {}
