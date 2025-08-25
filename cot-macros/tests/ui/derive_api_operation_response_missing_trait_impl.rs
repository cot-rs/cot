use cot_macros::ApiOperationResponse as DeriveApiOperationResponse;

#[derive(DeriveApiOperationResponse)]
enum MyResponse {
    A(Dummy),
}

struct Dummy;

fn main() {}
