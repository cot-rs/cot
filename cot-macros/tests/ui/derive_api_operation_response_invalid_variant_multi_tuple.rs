use cot::openapi::ApiOperationResponse;

#[derive(ApiOperationResponse)]
enum MyResponse {
    A(u32, String),
}

fn main() {}
