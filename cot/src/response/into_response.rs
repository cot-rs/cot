use std::borrow::Cow;
use std::convert::Infallible;
use std::fmt;

use bytes::{Bytes, BytesMut};
use cot::response::Response;
use cot::{Body, StatusCode};
use http;

use crate::response::IntoResponseParts;

/// Trait for generating responses.
/// Types that implement `IntoResponse` can be returned from handlers.
///
/// # Implementing `IntoResponse`
///
/// You generally shouldn't have to implement `IntoResponse` manually, as cot
/// provides implementations for many common types.
///
/// However, it might be necessary if you have a custom error type that you want
/// to return from handlers.

pub trait IntoResponse {
    /// Create a response.
    #[must_use]
    fn into_response(self) -> Response;
}

// impl<T> IntoResponse for T
// where
//     T: Into<Response>,
// {
//     fn into_response(self) -> Response {
//         self.into()
//     }
// }

impl IntoResponse for StatusCode {
    fn into_response(self) -> Response {
        let mut res = ().into_response();
        *res.status_mut() = self;
        res
    }
}

impl IntoResponse for () {
    fn into_response(self) -> Response {
        Body::empty().into_response()
    }
}

impl IntoResponse for Infallible {
    fn into_response(self) -> Response {
        match self {}
    }
}

impl<T, E> IntoResponse for Result<T, E>
where
    T: IntoResponse,
    E: IntoResponse,
{
    fn into_response(self) -> Response {
        match self {
            Ok(value) => value.into_response(),
            Err(err) => err.into_response(),
        }
    }
}

// impl<B> IntoResponse for Response<B>
// where
//     B: http_body::Body<Data = Bytes> + Send + 'static,
//     B::Error: Into<BoxError>,
// {
//     fn into_response(self) -> Response {
//         self.map(Body::new)
//     }
// }

impl IntoResponse for http::response::Parts {
    fn into_response(self) -> Response {
        Response::from_parts(self, Body::empty())
    }
}

impl IntoResponse for Body {
    fn into_response(self) -> Response {
        Response::new(self)
    }
}

impl IntoResponse for &'static str {
    fn into_response(self) -> Response {
        Cow::Borrowed(self).into_response()
    }
}

impl IntoResponse for String {
    fn into_response(self) -> Response {
        Cow::<'static, str>::Owned(self).into_response()
    }
}

impl IntoResponse for Box<str> {
    fn into_response(self) -> Response {
        String::from(self).into_response()
    }
}

impl IntoResponse for BytesMut {
    fn into_response(self) -> Response {
        self.freeze().into_response()
    }
}

// impl<T, U> IntoResponse for Chain<T, U>
// where
//     T: Buf + Unpin + Send + 'static,
//     U: Buf + Unpin + Send + 'static,
// {
//     fn into_response(self) -> Response {
//         let (first, second) = self.into_inner();
//         let mut res = Response::new(Body::new(BytesChainBody {
//             first: Some(first),
//             second: Some(second),
//         }));
//         res.headers_mut().insert(
//             header::CONTENT_TYPE,
//
// HeaderValue::from_static(mime::APPLICATION_OCTET_STREAM.as_ref()),         );
//         res
//     }
// }
//
// struct BytesChainBody<T, U> {
//     first: Option<T>,
//     second: Option<U>,
// }
//
// impl<T, U> http_body::Body for BytesChainBody<T, U>
// where
//     T: Buf + Unpin,
//     U: Buf + Unpin,
// {
//     type Data = Bytes;
//     type Error = Infallible;
//
//     fn poll_frame(
//         mut self: Pin<&mut Self>,
//         _cx: &mut Context<'_>,
//     ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
//         if let Some(mut buf) = self.first.take() {
//             let bytes = buf.copy_to_bytes(buf.remaining());
//             return Poll::Ready(Some(Ok(Frame::data(bytes))));
//         }
//
//         if let Some(mut buf) = self.second.take() {
//             let bytes = buf.copy_to_bytes(buf.remaining());
//             return Poll::Ready(Some(Ok(Frame::data(bytes))));
//         }
//
//         Poll::Ready(None)
//     }
//
//     fn is_end_stream(&self) -> bool {
//         self.first.is_none() && self.second.is_none()
//     }
//
//     fn size_hint(&self) -> SizeHint {
//         match (self.first.as_ref(), self.second.as_ref()) {
//             (Some(first), Some(second)) => {
//                 let total_size = first.remaining() + second.remaining();
//                 SizeHint::with_exact(total_size as u64)
//             }
//             (Some(buf), None) => SizeHint::with_exact(buf.remaining() as
// u64),             (None, Some(buf)) => SizeHint::with_exact(buf.remaining()
// as u64),             (None, None) => SizeHint::with_exact(0),
//         }
//     }
// }

impl IntoResponse for &'static [u8] {
    fn into_response(self) -> Response {
        Cow::Borrowed(self).into_response()
    }
}

impl<const N: usize> IntoResponse for &'static [u8; N] {
    fn into_response(self) -> Response {
        self.as_slice().into_response()
    }
}

impl<const N: usize> IntoResponse for [u8; N] {
    fn into_response(self) -> Response {
        self.to_vec().into_response()
    }
}

impl IntoResponse for Vec<u8> {
    fn into_response(self) -> Response {
        Cow::<'static, [u8]>::Owned(self).into_response()
    }
}

impl IntoResponse for Box<[u8]> {
    fn into_response(self) -> Response {
        Vec::from(self).into_response()
    }
}

macro_rules! into_response_from_type_and_mime {
    ($ty:ty, $mime:expr) => {
        impl IntoResponse for $ty {
            fn into_response(self) -> Response {
                let mut res = Body::from(self).into_response();
                res.headers_mut().insert(
                    http::header::CONTENT_TYPE,
                    http::HeaderValue::from_static($mime.as_ref()),
                );
                res
            }
        }
    };
}

// into_response_from_type_and_mime!(Cow<'static, str>, mime::TEXT_PLAIN_UTF_8);
// into_response_from_type_and_mime!(Cow<'static, [u8]>,
// mime::APPLICATION_OCTET_STREAM);
into_response_from_type_and_mime!(Bytes, mime::APPLICATION_OCTET_STREAM);

impl<R> IntoResponse for (StatusCode, R)
where
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let mut res = self.1.into_response();
        *res.status_mut() = self.0;
        res
    }
}

impl IntoResponse for http::HeaderMap {
    fn into_response(self) -> Response {
        let mut res = ().into_response();
        *res.headers_mut() = self;
        res
    }
}

impl IntoResponse for http::Extensions {
    fn into_response(self) -> Response {
        let mut res = ().into_response();
        *res.extensions_mut() = self;
        res
    }
}

impl<K, V, const N: usize> IntoResponse for [(K, V); N]
where
    K: TryInto<http::HeaderName>,
    K::Error: fmt::Display,
    V: TryInto<http::HeaderValue>,
    V::Error: fmt::Display,
{
    fn into_response(self) -> Response {
        (self, ()).into_response()
    }
}

impl<R> IntoResponse for (http::response::Parts, R)
where
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let (parts, res) = self;
        (parts.status, parts.headers, parts.extensions, res).into_response()
    }
}

impl<R> IntoResponse for (http::response::Response<()>, R)
where
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let (template, res) = self;
        let (parts, ()) = template.into_parts();
        (parts, res).into_response()
    }
}

impl<R> IntoResponse for (R,)
where
    R: IntoResponse,
{
    fn into_response(self) -> Response {
        let (res,) = self;
        res.into_response()
    }
}

macro_rules! impl_into_response {
    ( $($ty_lhs:ident,)* ($ty_from_request:ident) $(,$ty_rhs:ident)* ) => {
        #[allow(non_snake_case)]
        impl<$($ty_lhs,)* $ty_from_request, $($ty_rhs,)*> IntoResponse for ($($ty_lhs,)* $ty_from_request, $($ty_rhs,)*)
        where
            $($ty_lhs: IntoResponseParts,)*
            $ty_from_request: IntoResponse,
            $($ty_rhs: IntoResponseParts,)*
        {
            fn into_response(self) -> Response {
                let ($($ty_lhs,)* $ty_from_request, $($ty_rhs),*) = self;

                let res = $ty_from_request.into_response();
                let (parts, _) = res.into_parts();

                $(
                    let parts = match $ty_lhs.into_response_parts(parts) {
                        Ok(parts) => parts,
                        Err(err) => {
                            return err.into_response();
                        }
                    };
                )*
                $(
                    let parts = match $ty_rhs.into_response_parts(parts) {
                        Ok(parts) => parts,
                        Err(err) => {
                            return err.into_response();
                        }
                    };
                )*

                parts
            }
        }

        #[allow(non_snake_case)]
        impl<$($ty_lhs,)* $ty_from_request, $($ty_rhs,)*> IntoResponse for (StatusCode, $($ty_lhs,)* $ty_from_request, $($ty_rhs,)*)
        where
            $($ty_lhs: IntoResponseParts,)*
            $ty_from_request: IntoResponse,
            $($ty_rhs: IntoResponseParts,)*
        {
            fn into_response(self) -> Response {
                let (status, $($ty_lhs,)* $ty_from_request, $($ty_rhs),*) = self;

                let res = $ty_from_request.into_response();
                let (parts, _) = res.into_parts();

                $(
                    let parts = match $ty_lhs.into_response_parts(parts) {
                        Ok(parts) => parts,
                        Err(err) => {
                            return err.into_response();
                        }
                    };
                )*
                $(
                    let parts = match $ty_rhs.into_response_parts(parts) {
                        Ok(parts) => parts,
                        Err(err) => {
                            return err.into_response();
                        }
                    };
                )*

                (status, parts).into_response()
            }
        }

        #[allow(non_snake_case)]
        impl<$($ty_lhs,)* $ty_from_request, $($ty_rhs,)*> IntoResponse for (http::response::Parts, $($ty_lhs,)* $ty_from_request, $($ty_rhs,)*)
        where
            $($ty_lhs: IntoResponseParts,)*
            $ty_from_request: IntoResponse,
            $($ty_rhs: IntoResponseParts,)*
        {
            fn into_response(self) -> Response {
                let (outer_parts, $($ty_lhs,)* $ty_from_request, $($ty_rhs),*) = self;

                let res = $ty_from_request.into_response();
                let (parts, _) = res.into_parts();

                $(
                    let parts = match $ty_lhs.into_response_parts(parts) {
                        Ok(parts) => parts,
                        Err(err) => {
                            return err.into_response();
                        }
                    };
                )*
                $(
                    let parts = match $ty_rhs.into_response_parts(parts) {
                        Ok(parts) => parts,
                        Err(err) => {
                            return err.into_response();
                        }
                    };
                )*

                (outer_parts, parts).into_response()
            }
        }

        #[allow(non_snake_case)]
        impl<$($ty_lhs,)* $ty_from_request, $($ty_rhs,)*> IntoResponse for (http::response::Response<()>, $($ty_lhs,)* $ty_from_request, $($ty_rhs,)*)
        where
            $($ty_lhs: IntoResponseParts,)*
            $ty_from_request: IntoResponse,
            $($ty_rhs: IntoResponseParts,)*
        {
            fn into_response(self) -> Response {
                let (template,  $($ty_lhs,)* $ty_from_request, $($ty_rhs),*) = self;

                let res = $ty_from_request.into_response();
                let (parts, _) = res.into_parts();

                (parts, res).into_response()
            }
        }
    }
}

handle_all_parameters_from_request!(impl_into_response);
