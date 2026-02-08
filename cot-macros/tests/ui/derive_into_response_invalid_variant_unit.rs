use cot::response::IntoResponse;

#[derive(IntoResponse)]
enum MyResponse {
    A,
}

fn main() {}
