/// A type that represents JSON content.
///
/// Note that this is just a newtype wrapper around data and does not
/// provide any content validation.
///
/// # Examples
///
/// ```
/// use cot::json::Json;
///
/// let Json(data) = Json("content");
/// assert_eq!(data, "content");
/// ```
#[cfg(feature = "json")]
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Json<D>(pub D);
