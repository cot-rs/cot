use std::future::Future;

use cot::request::{PathParams, Request};
use http::request::Parts;
use serde::de::DeserializeOwned;

pub trait FromRequest: Sized {
    fn from_request(request: Request) -> impl Future<Output = cot::Result<Self>> + Send;
}

impl FromRequest for Request {
    async fn from_request(request: Request) -> cot::Result<Self> {
        Ok(request)
    }
}

pub trait FromRequestParts: Sized {
    fn from_request_parts(parts: &mut Parts) -> impl Future<Output = cot::Result<Self>> + Send;
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Path<D>(D);

impl<D: DeserializeOwned> FromRequestParts for Path<D> {
    async fn from_request_parts(parts: &mut Parts) -> cot::Result<Self> {
        let params = parts
            .extensions
            .get::<PathParams>()
            .expect("PathParams extension missing")
            .parse()?;
        Ok(Self(params))
    }
}
