use std::future::Future;
use std::sync::Arc;

use cot::request::{PathParams, Request};
use http::request::Parts;
use serde::de::DeserializeOwned;

use crate::db::Database;
use crate::form::{Form, FormResult};
use crate::request::RequestExt;
use crate::router::Urls;
use crate::session::Session;

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

impl FromRequestParts for Urls {
    async fn from_request_parts(parts: &mut Parts) -> cot::Result<Self> {
        Ok(Self::from_parts(parts))
    }
}

/// An extractor that extract data from the URL params.
///
/// # Examples
///
/// ```
/// // TODO
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Path<D>(pub D);

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

/// An extractor that extract data from a JSON body.
///
/// # Examples
///
/// ```
/// // TODO
/// ```
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Json<D>(pub D);

impl<D: DeserializeOwned> FromRequest for Json<D> {
    async fn from_request(mut request: Request) -> cot::Result<Self> {
        Ok(Self(request.json().await?))
    }
}

#[derive(Debug)]
pub struct RequestForm<F: Form>(pub FormResult<F>);

impl<F: Form> FromRequest for RequestForm<F> {
    async fn from_request(mut request: Request) -> cot::Result<Self> {
        Ok(Self(F::from_request(&mut request).await?))
    }
}

impl FromRequestParts for Session {
    async fn from_request_parts(parts: &mut Parts) -> cot::Result<Self> {
        Ok(parts.session().clone())
    }
}

#[derive(Debug)]
pub struct RequestDb(pub Arc<Database>);

impl FromRequestParts for RequestDb {
    async fn from_request_parts(parts: &mut Parts) -> cot::Result<Self> {
        Ok(Self(parts.db().clone()))
    }
}

// TODO tests
// TODO docs
// TODO examples
// TODO change examples to ues the new extractors
// TODO change admin to ues the new extractors
// TODO generic RequestExt
// TODO auth object
// TODO HTTP method router
// TODO URL params extractor
// TODO RequestExt::extract{,_parts}
