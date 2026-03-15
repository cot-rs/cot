use cot::openapi::ApiOperationResponse;

#[derive(ApiOperationResponse)]
enum MyResponse {
    A(Dummy),
}

struct Dummy;

fn main() {}
