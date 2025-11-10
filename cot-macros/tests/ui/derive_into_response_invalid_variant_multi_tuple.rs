use cot::response::IntoResponse;

#[derive(IntoResponse)]
enum MyResponse {
    A(u32, String),
}

fn main() {}
