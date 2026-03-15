use cot::response::IntoResponse;

#[derive(IntoResponse)]
enum MyResponse {
    A { field: u32 },
}

fn main() {}
