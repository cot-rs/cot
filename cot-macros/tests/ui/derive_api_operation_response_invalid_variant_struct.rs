use cot_macros::ApiOperationResponse;

#[derive(ApiOperationResponse)]
enum MyResponse {
    A { field: u32 },
}

fn main() {}
