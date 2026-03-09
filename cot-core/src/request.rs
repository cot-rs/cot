use indexmap::IndexMap;

use crate::Body;
use crate::error::impl_into_cot_error;

pub mod extractors;
mod path_params_deserializer;

/// HTTP request type.
pub type Request = http::Request<Body>;

/// HTTP request head type.
pub type RequestHead = http::request::Parts;

#[derive(Debug, thiserror::Error)]
#[error("invalid content type; expected `{expected}`, found `{actual}`")]
pub struct InvalidContentType {
    pub expected: &'static str,
    pub actual: String,
}
impl_into_cot_error!(InvalidContentType, BAD_REQUEST);

#[repr(transparent)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AppName(pub String);

#[repr(transparent)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RouteName(pub String);

/// Path parameters extracted from the request URL, and available as a map of
/// strings.
///
/// This struct is meant to be mainly used via the [`PathParams::parse`]
/// method, which will deserialize the path parameters into a type `T`
/// implementing `serde::DeserializeOwned`. If needed, you can also access the
/// path parameters directly using the [`PathParams::get`] method.
///
/// # Examples
///
/// ```
/// use cot::request::{PathParams, Request, RequestExt};
/// use cot::response::Response;
/// use cot::test::TestRequestBuilder;
///
/// async fn my_handler(mut request: Request) -> cot::Result<Response> {
///     let path_params = request.path_params();
///     let name = path_params.get("name").unwrap();
///
///     // using more ergonomic syntax:
///     let name: String = request.path_params().parse()?;
///
///     let name = println!("Hello, {}!", name);
///     // ...
///     # unimplemented!()
/// }
/// ```
#[derive(Debug, Clone)]
pub struct PathParams {
    params: IndexMap<String, String>,
}

impl Default for PathParams {
    fn default() -> Self {
        Self::new()
    }
}

impl PathParams {
    /// Creates a new [`PathParams`] instance.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::PathParams;
    ///
    /// let mut path_params = PathParams::new();
    /// path_params.insert("name".into(), "world".into());
    /// assert_eq!(path_params.get("name"), Some("world"));
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            params: IndexMap::new(),
        }
    }

    /// Inserts a new path parameter.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::PathParams;
    ///
    /// let mut path_params = PathParams::new();
    /// path_params.insert("name".into(), "world".into());
    /// assert_eq!(path_params.get("name"), Some("world"));
    /// ```
    pub fn insert(&mut self, name: String, value: String) {
        self.params.insert(name, value);
    }

    /// Iterates over the path parameters.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::PathParams;
    ///
    /// let mut path_params = PathParams::new();
    /// path_params.insert("name".into(), "world".into());
    /// for (name, value) in path_params.iter() {
    ///     println!("{}: {}", name, value);
    /// }
    /// ```
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.params
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
    }

    /// Returns the number of path parameters.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::PathParams;
    ///
    /// let path_params = PathParams::new();
    /// assert_eq!(path_params.len(), 0);
    /// ```
    #[must_use]
    pub fn len(&self) -> usize {
        self.params.len()
    }

    /// Returns `true` if the path parameters are empty.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::PathParams;
    ///
    /// let path_params = PathParams::new();
    /// assert!(path_params.is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.params.is_empty()
    }

    /// Returns the value of a path parameter.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::PathParams;
    ///
    /// let mut path_params = PathParams::new();
    /// path_params.insert("name".into(), "world".into());
    /// assert_eq!(path_params.get("name"), Some("world"));
    /// ```
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&str> {
        self.params.get(name).map(String::as_str)
    }

    /// Returns the value of a path parameter at the given index.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::PathParams;
    ///
    /// let mut path_params = PathParams::new();
    /// path_params.insert("name".into(), "world".into());
    /// assert_eq!(path_params.get_index(0), Some("world"));
    /// ```
    #[must_use]
    pub fn get_index(&self, index: usize) -> Option<&str> {
        self.params
            .get_index(index)
            .map(|(_, value)| value.as_str())
    }

    /// Returns the key of a path parameter at the given index.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::PathParams;
    ///
    /// let mut path_params = PathParams::new();
    /// path_params.insert("name".into(), "world".into());
    /// assert_eq!(path_params.key_at_index(0), Some("name"));
    /// ```
    #[must_use]
    pub fn key_at_index(&self, index: usize) -> Option<&str> {
        self.params.get_index(index).map(|(key, _)| key.as_str())
    }

    /// Deserializes the path parameters into a type `T` implementing
    /// `serde::DeserializeOwned`.
    ///
    /// # Errors
    ///
    /// Throws an error if the path parameters could not be deserialized.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::request::PathParams;
    ///
    /// # fn main() -> Result<(), cot::Error> {
    /// let mut path_params = PathParams::new();
    /// path_params.insert("hello".into(), "world".into());
    ///
    /// let hello: String = path_params.parse()?;
    /// assert_eq!(hello, "world");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ```
    /// use cot::request::PathParams;
    ///
    /// # fn main() -> Result<(), cot::Error> {
    /// let mut path_params = PathParams::new();
    /// path_params.insert("hello".into(), "world".into());
    /// path_params.insert("name".into(), "john".into());
    ///
    /// let (hello, name): (String, String) = path_params.parse()?;
    /// assert_eq!(hello, "world");
    /// assert_eq!(name, "john");
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ```
    /// use cot::request::PathParams;
    /// use serde::Deserialize;
    ///
    /// # fn main() -> Result<(), cot::Error> {
    /// let mut path_params = PathParams::new();
    /// path_params.insert("hello".into(), "world".into());
    /// path_params.insert("name".into(), "john".into());
    ///
    /// #[derive(Deserialize)]
    /// struct Params {
    ///     hello: String,
    ///     name: String,
    /// }
    ///
    /// let params: Params = path_params.parse()?;
    /// assert_eq!(params.hello, "world");
    /// assert_eq!(params.name, "john");
    /// # Ok(())
    /// # }
    /// ```
    pub fn parse<'de, T: serde::Deserialize<'de>>(
        &'de self,
    ) -> Result<T, PathParamsDeserializerError> {
        let deserializer = path_params_deserializer::PathParamsDeserializer::new(self);
        serde_path_to_error::deserialize(deserializer).map_err(PathParamsDeserializerError)
    }
}

/// An error that occurs when deserializing path parameters.
#[derive(Debug, Clone, thiserror::Error)]
#[error("could not parse path parameters: {0}")]
pub struct PathParamsDeserializerError(
    // A wrapper over the original deserializer error. The exact error reason
    // shouldn't be useful to the user, hence we're not exposing it.
    #[source] serde_path_to_error::Error<path_params_deserializer::PathParamsDeserializerError>,
);
impl_into_cot_error!(PathParamsDeserializerError, BAD_REQUEST);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_params() {
        let mut path_params = PathParams::new();
        path_params.insert("name".into(), "world".into());

        assert_eq!(path_params.get("name"), Some("world"));
        assert_eq!(path_params.get("missing"), None);
    }

    #[test]
    fn path_params_parse() {
        #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
        struct Params {
            hello: String,
            foo: String,
        }

        let mut path_params = PathParams::new();
        path_params.insert("hello".into(), "world".into());
        path_params.insert("foo".into(), "bar".into());

        let params: Params = path_params.parse().unwrap();
        assert_eq!(
            params,
            Params {
                hello: "world".to_string(),
                foo: "bar".to_string(),
            }
        );
    }
}
