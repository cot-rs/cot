use cot::http::request::Parts;
use cot::request::extractors::FromRequestParts;

#[derive(FromRequestParts)]
struct MyStruct {
    user_id: DummyExtractor,
    session_id: DummyExtractor,
}

struct DummyExtractor;

impl FromRequestParts for DummyExtractor {
    async fn from_request_parts(_parts: &mut Parts) -> cot::Result<Self> {
        Ok(Self)
    }
}

fn main() {}
