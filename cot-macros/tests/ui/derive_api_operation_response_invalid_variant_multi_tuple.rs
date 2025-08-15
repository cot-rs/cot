use cot_macros::ApiOperationResponse;

#[derive(ApiOperationResponse)]
enum MyResponse {
    A(u32, String),
}

fn main() {}
