use crate::response::IntoResponse;

/// Trait for adding headers and extensions to a response.
pub trait IntoResponseParts {
    /// The type returned in the event of an error.
    ///
    /// This can be used to fallibly convert types into headers or extensions.
    type Error: IntoResponse;

    /// Set parts of the response
    fn into_response_parts(
        self,
        res: http::response::Parts,
    ) -> Result<http::response::Parts, Self::Error>;
}
