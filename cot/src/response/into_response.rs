use cot::core::impl_into_cot_error;
use cot::core::response::{IntoResponse, Response};
#[cfg(feature = "json")]
use cot_core::headers::HTML_CONTENT_TYPE;
use cot_core::html::Html;

impl IntoResponse for Html {
    /// Create a new HTML response.
    ///
    /// This creates a new [`Response`] object with a content type of
    /// `text/html; charset=utf-8` and given body.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot_core::html::Html;
    /// use cot_core::response::IntoResponse;
    ///
    /// let html = Html::new("<div>Hello</div>");
    ///
    /// let response = html.into_response();
    /// ```
    fn into_response(self) -> crate::Result<Response> {
        self.0
            .into_response()
            .with_content_type(HTML_CONTENT_TYPE)
            .into_response()
    }
}

#[cfg(feature = "json")]
impl<D: serde::Serialize> IntoResponse for cot::json::Json<D> {
    /// Create a new JSON response.
    ///
    /// This creates a new [`Response`] object with a content type of
    /// `application/json` and given body.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// use cot::json::Json;
    /// use cot_core::response::IntoResponse;
    ///
    /// let data = HashMap::from([("hello", "world")]);
    /// let json = Json(data);
    ///
    /// let response = json.into_response();
    /// ```
    fn into_response(self) -> crate::Result<Response> {
        // a "reasonable default" for a JSON response size
        const DEFAULT_JSON_SIZE: usize = 128;

        let mut buf = Vec::with_capacity(DEFAULT_JSON_SIZE);
        let mut serializer = serde_json::Serializer::new(&mut buf);
        serde_path_to_error::serialize(&self.0, &mut serializer).map_err(JsonSerializeError)?;
        let data = String::from_utf8(buf).expect("JSON serialization always returns valid UTF-8");

        data.with_content_type(JSON_CONTENT_TYPE).into_response()
    }
}

#[cfg(feature = "json")]
#[derive(Debug, thiserror::Error)]
#[error("JSON serialization error: {0}")]
struct JsonSerializeError(serde_path_to_error::Error<serde_json::Error>);
#[cfg(feature = "json")]
impl_into_cot_error!(JsonSerializeError, INTERNAL_SERVER_ERROR);

#[cfg(test)]
mod tests {
    use cot_core::StatusCode;

    #[cfg(feature = "json")]
    #[cot_macros::test]
    async fn test_json_struct_into_response() {
        use serde::Serialize;

        #[derive(Serialize, PartialEq, Debug)]
        struct TestData {
            name: String,
            value: i32,
        }

        let data = TestData {
            name: "test".to_string(),
            value: 123,
        };
        let json = cot::json::Json(data);
        let response = json.into_response().unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(http::header::CONTENT_TYPE).unwrap(),
            JSON_CONTENT_TYPE
        );

        let body_bytes = response.into_body().into_bytes().await.unwrap();
        let expected_json = r#"{"name":"test","value":123}"#;

        assert_eq!(body_bytes, expected_json.as_bytes());
    }

    #[cfg(feature = "json")]
    #[cot_macros::test]
    async fn test_json_hashmap_into_response() {
        use std::collections::HashMap;

        let data = HashMap::from([("key", "value")]);
        let json = cot::json::Json(data);
        let response = json.into_response().unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(http::header::CONTENT_TYPE).unwrap(),
            JSON_CONTENT_TYPE
        );

        let body_bytes = response.into_body().into_bytes().await.unwrap();
        let expected_json = r#"{"key":"value"}"#;
        assert_eq!(body_bytes, expected_json.as_bytes());
    }
}
