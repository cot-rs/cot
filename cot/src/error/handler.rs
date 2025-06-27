use cot::request::Request;
use cot::response::Response;
use http::request::Parts;

pub trait ErrorPageHandler {
    fn handle(&self, request: Request) -> impl Future<Output = cot::Result<Response>> + Send;
}

macro_rules! impl_request_handler {
    ($($ty:ident),*) => {
        impl<Func, $($ty,)* Fut, R> ErrorPageHandler for Func
        where
            Func: FnOnce($($ty,)*) -> Fut + Clone + Send + Sync + 'static,
            $($ty: FromErrorRequestParts + Send,)*
            Fut: Future<Output = R> + Send,
            R: IntoResponse,
        {
            #[allow(non_snake_case)]
            async fn handle(&self, request: Request) -> Result<Response> {
                #[allow(unused_variables, unused_mut)] // for the case where there are no params
                let (mut parts, _body) = request.into_parts();

                $(
                    let $ty = $ty::from_request_parts(&mut parts).await?;
                )*

                self.clone()($($ty,)*).await.into_response()
            }
        }
    };
}

pub trait FromErrorRequestParts: Sized {
    /// Extracts data from the request parts.
    ///
    /// # Errors
    ///
    /// Throws an error if the extractor fails to extract the data from the
    /// request parts.
    fn from_request_parts(parts: &mut Parts) -> impl Future<Output = crate::Result<Self>> + Send;
}

macro_rules! impl_from_error_request_parts {
    ($ty:ty) => {
        impl FromErrorRequestParts for $ty {
            fn from_request_parts(
                parts: &mut Parts,
            ) -> impl Future<Output = crate::Result<Self>> + Send {
                $crate::request::extractors::FromRequestParts::from_request_parts(parts)
            }
        }
    };
}

pub use impl_from_error_request_parts;

impl_from_error_request_parts!(crate::router::Urls);

#[derive(Debug)]
#[repr(transparent)]
pub struct ResponseError(pub crate::Error);

impl FromErrorRequestParts for ResponseError {
    fn from_request_parts(parts: &mut Parts) -> impl Future<Output = cot::Result<Self>> + Send {
        parts.extensions..map_or_else(||)
    }
}
