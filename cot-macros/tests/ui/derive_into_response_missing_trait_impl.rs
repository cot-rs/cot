use cot::response::IntoResponse;

#[derive(IntoResponse)]
enum MyResponse {
    A(Dummy),
}

struct Dummy;

fn main() {}
