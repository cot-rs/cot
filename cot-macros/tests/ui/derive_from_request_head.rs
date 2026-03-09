use cot::request::RequestHead;
use cot::request::extractors::FromRequestHead;

#[derive(FromRequestHead)]
struct MyStruct {
    user_id: DummyExtractor,
    session_id: DummyExtractor,
}

#[derive(FromRequestHead)]
struct MyUnitStruct;

#[derive(FromRequestHead)]
struct MyTupleStruct(DummyExtractor, DummyExtractor);

struct DummyExtractor;

impl FromRequestHead for DummyExtractor {
    async fn from_request_head(_head: &RequestHead) -> cot::Result<Self> {
        Ok(Self)
    }
}

fn main() {}
