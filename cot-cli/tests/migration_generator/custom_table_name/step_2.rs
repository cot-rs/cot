use cot::db::{Auto, LimitedString, model};

#[derive(Debug)]
#[model(table_name = "myamazing_table_name")]
struct CustomModel {
    #[model(primary_key)]
    id: Auto<i32>,
    field_1: String,
    field_2: i32,
}
