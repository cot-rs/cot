//! Path matching and routing.
//!
//! This module provides a path matcher that can be used to match paths against
//! a given pattern. It also provides a way to reverse paths to their original
//! form given a set of parameters.
use std::collections::HashMap;
use std::fmt::{Display, Write};
use std::sync::Arc;

use cot::router::tree::MatchitPattern;
use cot_core::error::impl_into_cot_error;
use thiserror::Error;

const PATH_MATCHER_ERROR_PREFIX: &str = "route conflict error:";
/// An error produced when parsing a route path pattern fails.
#[derive(Debug, Error)]
#[non_exhaustive]
pub(super) enum PathMatcherError {
    /// Two parameters appear consecutively with no literal text between them,
    #[error(
        "{PATH_MATCHER_ERROR_PREFIX} consecutive parameters are not allowed in pattern `{pattern}`"
    )]
    #[non_exhaustive]
    ConsecutiveParams { pattern: String },
    /// A `{` was opened but never closed with a matching `}`.
    #[error(
        "{PATH_MATCHER_ERROR_PREFIX} unclosed parameter `{{{name}` in pattern `{pattern}`; expected a closing `}}`"
    )]
    #[non_exhaustive]
    UnclosedParam { pattern: String, name: String },
    /// A `}` appeared without a preceding `{` to open it.
    #[error(
        "{PATH_MATCHER_ERROR_PREFIX} closing brace `}}` without a matching opening `{{` in pattern `{pattern}`"
    )]
    #[non_exhaustive]
    UnmatchedClosingBrace { pattern: String },
    /// A parameter name is empty or contains characters other than
    /// alphanumerics/underscore, or starts with a digit.
    #[error(
        "{PATH_MATCHER_ERROR_PREFIX} invalid parameter name `{name}` in pattern `{pattern}`; parameter names must start \
         with a letter or underscore and contain only letters, digits, or underscores"
    )]
    #[non_exhaustive]
    InvalidParamName { pattern: String, name: String },
    /// Same as [`PathMatcherError::InvalidParamName`], but for the name
    /// following a `*` in a wildcard segment.
    #[error(
        "{PATH_MATCHER_ERROR_PREFIX} invalid wildcard name `{name}` in pattern `{pattern}`; wildcard names must start \
         with a letter or underscore and contain only letters, digits, or underscores"
    )]
    #[non_exhaustive]
    InvalidWildcardName { pattern: String, name: String },
    /// A wildcard segment was followed by more path segments,
    #[error(
        "{PATH_MATCHER_ERROR_PREFIX} wildcard parameter `{{*{name}}}` must be the last segment of pattern `{pattern}`; \
         a wildcard consumes the rest of the path, so nothing can follow it"
    )]
    #[non_exhaustive]
    WildcardNotAtEnd { pattern: String, name: String },
    #[error("{PATH_MATCHER_ERROR_PREFIX} unsupported brace in {pattern}")]
    UnsupportedLiteralBrace { pattern: String },
}
impl_into_cot_error!(PathMatcherError);

/// An absolute route path.
///
/// The path is normalized to always begin with `/` and allows paths to be
/// joined without introducing duplicate `/` separators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AbsolutePath(String);

impl AbsolutePath {
    #[must_use]
    pub(crate) fn new<S: Into<String>>(s: S) -> Self {
        let mut s = s.into();
        if !s.starts_with('/') {
            s.insert(0, '/');
        }
        Self(s)
    }

    #[must_use]
    pub(crate) fn root() -> Self {
        Self(String::from("/"))
    }

    #[must_use]
    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }

    #[must_use]
    pub(crate) fn join(&self, suffix: &AbsolutePath) -> AbsolutePath {
        let trimmed = self.0.strip_suffix('/').unwrap_or(&self.0);
        AbsolutePath(format!("{trimmed}{}", suffix.0))
    }
}

impl Display for AbsolutePath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<AbsolutePath> for String {
    fn from(value: AbsolutePath) -> Self {
        value.0
    }
}

#[derive(Debug, Clone)]
pub(super) struct PathMatcher {
    parts: Vec<PathPart>,
}

impl PathMatcher {
    #[must_use]
    pub(crate) fn new<T: Into<String>>(path_pattern: T) -> Self {
        match Self::try_new(path_pattern) {
            Ok(matcher) => matcher,
            Err(err) => panic!("{err}"),
        }
    }

    pub(crate) fn try_new<T: Into<String>>(path_pattern: T) -> Result<Self, PathMatcherError> {
        #[derive(Debug, Copy, Clone)]
        enum State {
            Literal { start: usize },
            Param { start: usize },
        }

        let mut path_pattern = path_pattern.into();
        if !path_pattern.starts_with('/') {
            path_pattern.insert(0, '/');
        }

        let mut parts = Vec::new();
        let mut state = State::Literal { start: 0 };

        let mut char_iter = path_pattern
            .char_indices()
            .map(|(i, c)| (i, Some(c)))
            .chain(std::iter::once((path_pattern.len(), None)))
            .peekable();
        while let Some((index, ch)) = char_iter.next() {
            match (ch, state) {
                (Some('{') | None, State::Literal { start }) => {
                    let literal = &path_pattern[start..index];
                    if literal.is_empty() {
                        if index != 0 && ch.is_some() {
                            return Err(PathMatcherError::ConsecutiveParams {
                                pattern: path_pattern.clone(),
                            });
                        }
                    } else {
                        parts.push(PathPart::Literal(literal.to_string()));
                    }
                    state = State::Param { start: index + 1 };
                }
                (Some('{'), State::Param { start }) => {
                    if start == index {
                        // escaped `{`
                        state = State::Literal { start: index };
                    } else {
                        return Err(PathMatcherError::UnclosedParam {
                            pattern: path_pattern.clone(),
                            name: path_pattern[start..index].to_string(),
                        });
                    }
                }
                (Some('}'), State::Literal { start }) => {
                    let next_char = char_iter.peek().map(|(_, ch)| *ch).unwrap_or_default();

                    if next_char == Some('}') {
                        // escaped `}`
                        let literal = &path_pattern[start..=index];
                        parts.push(PathPart::Literal(literal.to_string()));

                        char_iter.next();
                        state = State::Literal { start: index + 2 };
                    } else {
                        return Err(PathMatcherError::UnmatchedClosingBrace {
                            pattern: path_pattern.clone(),
                        });
                    }
                }
                (Some('}'), State::Param { start }) => {
                    let param_name = &path_pattern[start..index].trim();

                    if let Some(wildcard_name) = param_name.strip_prefix('*') {
                        if !Self::is_param_name_valid(wildcard_name) {
                            return Err(PathMatcherError::InvalidWildcardName {
                                pattern: path_pattern.clone(),
                                name: wildcard_name.to_string(),
                            });
                        }

                        let next_char = char_iter.peek().map(|(_, ch)| *ch).unwrap_or_default();
                        if next_char.is_some() {
                            return Err(PathMatcherError::WildcardNotAtEnd {
                                pattern: path_pattern.clone(),
                                name: wildcard_name.to_string(),
                            });
                        }

                        parts.push(PathPart::Wildcard {
                            name: wildcard_name.to_string(),
                        });
                    } else {
                        if !Self::is_param_name_valid(param_name) {
                            return Err(PathMatcherError::InvalidParamName {
                                pattern: path_pattern.clone(),
                                name: param_name.to_string(),
                            });
                        }

                        parts.push(PathPart::Param {
                            name: param_name.to_string(),
                        });
                    }
                    state = State::Literal { start: index + 1 };
                }
                (Some('/') | None, State::Param { start }) => {
                    return Err(PathMatcherError::UnclosedParam {
                        pattern: path_pattern.clone(),
                        name: path_pattern[start..index].to_string(),
                    });
                }
                _ => {}
            }
        }

        Ok(Self { parts })
    }

    fn is_param_name_valid(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }
        let first_char = name.chars().next().expect("Empty string");
        if !first_char.is_alphabetic() && first_char != '_' {
            return false;
        }
        for ch in name.chars() {
            if !ch.is_alphanumeric() && ch != '_' {
                return false;
            }
        }
        true
    }

    pub(crate) fn reverse(&self, params: &ReverseParamMap) -> Result<String, ReverseError> {
        let mut result = String::new();

        for part in &self.parts {
            match part {
                PathPart::Literal(s) => result.push_str(s),
                PathPart::Param { name } | PathPart::Wildcard { name } => {
                    let value = params
                        .get(name)
                        .ok_or_else(|| ReverseError::MissingParam(name.clone()))?;
                    result.push_str(value);
                }
            }
        }

        Ok(result)
    }

    #[cfg(feature = "openapi")]
    pub(super) fn param_names(&self) -> impl Iterator<Item = &str> {
        self.parts.iter().filter_map(|part| match part {
            PathPart::Literal(..) => None,
            PathPart::Param { name } | PathPart::Wildcard { name } => Some(name.as_str()),
        })
    }

    pub(super) fn parts(&self) -> &[PathPart] {
        &self.parts
    }
}

impl TryFrom<Arc<PathMatcher>> for MatchitPattern {
    type Error = PathMatcherError;

    fn try_from(value: Arc<PathMatcher>) -> Result<Self, Self::Error> {
        let mut pattern = String::new();
        for part in &value.parts {
            match part {
                PathPart::Literal(s) if s.contains(['{', '}']) => {
                    return Err(PathMatcherError::UnsupportedLiteralBrace {
                        pattern: value.to_string(),
                    });
                }
                PathPart::Literal(s) => pattern.push_str(s),
                PathPart::Param { name } => {
                    let _ = write!(pattern, "{{{name}}}");
                }
                PathPart::Wildcard { name } => {
                    let _ = write!(pattern, "{{*{name}}}");
                }
            }
        }

        Ok(MatchitPattern::new(pattern))
    }
}

impl Display for PathMatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for part in &self.parts {
            write!(f, "{part}")?;
        }
        Ok(())
    }
}

/// A map of parameters for the [`crate::router::Router::reverse`] method.
///
/// Typically, it's only used internally via the [`crate::reverse`] macro.
///
/// # Examples
///
/// ```
/// use cot::router::path::ReverseParamMap;
///
/// let mut map = ReverseParamMap::new();
/// map.insert("id", "123");
/// map.insert("post_id", "456");
/// ```
#[derive(Debug)]
pub struct ReverseParamMap {
    params: HashMap<String, String>,
}

impl Default for ReverseParamMap {
    fn default() -> Self {
        Self::new()
    }
}

impl ReverseParamMap {
    /// Creates a new instance of [`ReverseParamMap`].
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::router::path::ReverseParamMap;
    ///
    /// let mut map = ReverseParamMap::new();
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            params: HashMap::new(),
        }
    }

    /// Inserts a value into the map. If the key already exists, the value will
    /// be overwritten.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::router::path::ReverseParamMap;
    ///
    /// let mut map = ReverseParamMap::new();
    /// map.insert("id", "123");
    /// map.insert("id", "456");
    /// ```
    #[expect(clippy::needless_pass_by_value)]
    pub fn insert<K: ToString, V: ToString>(&mut self, key: K, value: V) {
        self.params.insert(key.to_string(), value.to_string());
    }

    #[must_use]
    fn get(&self, key: &str) -> Option<&str> {
        self.params.get(key).map(String::as_str)
    }
}

#[doc(hidden)]
#[macro_export]
macro_rules! reverse_param_map {
    () => {{
        $crate::router::path::ReverseParamMap::new()
    }};
    ($($key:ident = $value:expr),*) => {{
        let mut map = $crate::router::path::ReverseParamMap::new();
        $( map.insert(stringify!($key), &$value); )*
        map
    }};
}

const REVERSE_ERROR_PREFIX: &str = "failed to reverse route:";
/// An error that occurs when reversing a path with missing parameters.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ReverseError {
    /// A parameter is missing for the reverse operation.
    #[error("{REVERSE_ERROR_PREFIX} missing parameter for reverse: `{0}`")]
    #[non_exhaustive]
    MissingParam(String),
}
impl_into_cot_error!(ReverseError);

#[derive(Debug, Clone)]
pub(super) enum PathPart {
    Literal(String),
    Param { name: String },
    Wildcard { name: String },
}

impl Display for PathPart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PathPart::Literal(s) => {
                let s = s.replace('{', "{{").replace('}', "}}");
                write!(f, "{s}")
            }
            PathPart::Param { name } => write!(f, "{{{name}}}"),
            PathPart::Wildcard { name } => write!(f, "{{*{name}}}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reverse_param_map_default() {
        let map = ReverseParamMap::default();
        assert_eq!(map.params.len(), 0);
    }

    #[test]
    fn path_parser_no_params() {
        let path_parser = PathMatcher::new("/users");
        assert_eq!(path_parser.to_string(), "/users");
        assert_eq!(
            path_parser.param_names().collect::<Vec<_>>(),
            Vec::<&str>::new()
        );
    }

    #[test]
    fn path_parser_adds_missing_leading_slash() {
        let path_parser = PathMatcher::new("users/{id}");
        let mut params = ReverseParamMap::new();
        params.insert("id", "123");

        assert_eq!(path_parser.reverse(&params).unwrap(), "/users/123");
        assert_eq!(path_parser.to_string(), "/users/{id}");
        assert_eq!(path_parser.param_names().collect::<Vec<_>>(), vec!["id"]);
    }

    #[test]
    fn path_parser_escaped() {
        let path_parser = PathMatcher::new("/users/{{{{{{escaped}}}}}}");
        assert_eq!(path_parser.to_string(), "/users/{{{{{{escaped}}}}}}");
        assert_eq!(
            path_parser.reverse(&ReverseParamMap::new()).unwrap(),
            "/users/{{{escaped}}}"
        );
    }

    #[test]
    fn path_parser_single_param() {
        let path_parser = PathMatcher::new("/users/{id}");
        assert_eq!(path_parser.to_string(), "/users/{id}");
        assert_eq!(path_parser.param_names().collect::<Vec<_>>(), vec!["id"]);
    }

    #[test]
    fn path_parser_param_whitespace() {
        let path_parser = PathMatcher::new("/users/{ id }");

        assert_eq!(path_parser.to_string(), "/users/{id}");
        assert_eq!(path_parser.param_names().collect::<Vec<_>>(), vec!["id"]);
    }

    #[test]
    fn path_parser_multiple_params() {
        let path_parser = PathMatcher::new("/users/{id}/posts/{post_id}");
        assert_eq!(
            path_parser.param_names().collect::<Vec<_>>(),
            vec!["id", "post_id"]
        );
    }

    #[test]
    #[should_panic(
        expected = "route conflict error: consecutive parameters are not allowed in pattern `/users/{id}{post_id}`"
    )]
    fn path_parser_consecutive_params() {
        let _ = PathMatcher::new("/users/{id}{post_id}");
    }

    #[test]
    #[should_panic(
        expected = "route conflict error: invalid parameter name `` in pattern `/users/{}`; parameter names must start with a letter or underscore and contain only letters, digits, or underscores"
    )]
    fn path_parser_invalid_name_empty() {
        let _ = PathMatcher::new("/users/{}");
    }

    #[test]
    #[should_panic(
        expected = "route conflict error: invalid parameter name `123` in pattern `/users/{123}`; parameter names must start with a letter or underscore and contain only letters, digits, or underscores"
    )]
    fn path_parser_invalid_name_numeric() {
        let _ = PathMatcher::new("/users/{123}");
    }

    #[test]
    #[should_panic(
        expected = "route conflict error: invalid parameter name `abc#$%` in pattern `/users/{abc#$%}`; parameter names must start with a letter or underscore and contain only letters, digits, or underscores"
    )]
    fn path_parser_invalid_name_non_alphanumeric() {
        let _ = PathMatcher::new("/users/{abc#$%}");
    }

    #[test]
    #[should_panic(
        expected = "route conflict error: unclosed parameter `{foo` in pattern `/users/{foo`; expected a closing `}`"
    )]
    fn path_parser_unclosed() {
        let _ = PathMatcher::new("/users/{foo");
    }

    #[test]
    #[should_panic(
        expected = "route conflict error: closing brace `}` without a matching opening `{` in pattern `/users/foo}`"
    )]
    fn path_parser_missing_opening_brace() {
        let _ = PathMatcher::new("/users/foo}");
    }

    #[test]
    #[should_panic(
        expected = "route conflict error: unclosed parameter `{foo` in pattern `/users/{foo/bar`; expected a closing `}`"
    )]
    fn path_parser_unclosed_slash() {
        let _ = PathMatcher::new("/users/{foo/bar");
    }

    #[test]
    #[should_panic(
        expected = "route conflict error: unclosed parameter `{foo` in pattern `/users/{foo{bar`; expected a closing `}`"
    )]
    fn path_parser_unclosed_double() {
        let _ = PathMatcher::new("/users/{foo{bar");
    }

    #[test]
    #[should_panic(
        expected = "route conflict error: closing brace `}` without a matching opening `{` in pattern `/users/{{{foo}}/bar`"
    )]
    fn path_parser_escaping_unclosed() {
        let _ = PathMatcher::new("/users/{{{foo}}/bar");
    }

    #[test]
    fn path_parser_display() {
        let path_parser = PathMatcher::new("/users/{id}/posts/{{escaped}}");
        assert_eq!(format!("{path_parser}"), "/users/{id}/posts/{{escaped}}");
    }

    #[test]
    fn reverse_with_valid_params() {
        let path_parser = PathMatcher::new("/users/{id}/posts/{post_id}");
        let mut params = ReverseParamMap::new();
        params.insert("id", "123");
        params.insert("post_id", "456");
        assert_eq!(
            path_parser.reverse(&params).unwrap(),
            "/users/123/posts/456"
        );
    }

    #[test]
    fn reverse_with_missing_param() {
        let path_parser = PathMatcher::new("/users/{id}/posts/{post_id}");
        let mut params = ReverseParamMap::new();
        params.insert("id", "123");
        let result = path_parser.reverse(&params);
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "failed to reverse route: missing parameter for reverse: `post_id`"
        );
    }

    #[test]
    fn reverse_with_extra_param() {
        let path_parser = PathMatcher::new("/users/{id}/posts/{post_id}");
        let mut params = ReverseParamMap::new();
        params.insert("id", "123");
        params.insert("post_id", "456");
        params.insert("extra", "789");
        assert_eq!(
            path_parser.reverse(&params).unwrap(),
            "/users/123/posts/456"
        );
    }

    #[test]
    fn non_ascii_path_pattern() {
        let path_parser = PathMatcher::new("/café/{id}");
        let mut params = ReverseParamMap::new();
        params.insert("id", "123");
        assert_eq!(path_parser.reverse(&params).unwrap(), "/café/123");
    }

    #[test]
    fn non_ascii_path_literal() {
        let path_parser = PathMatcher::new("/café/test");
        let params = ReverseParamMap::new();
        assert_eq!(path_parser.reverse(&params).unwrap(), "/café/test");
    }

    #[test]
    fn path_parser_wildcard() {
        let path_parser = PathMatcher::new("/static/{*path}");
        assert_eq!(path_parser.to_string(), "/static/{*path}");
        assert_eq!(path_parser.param_names().collect::<Vec<_>>(), vec!["path"]);
    }

    #[test]
    fn reverse_with_wildcard() {
        let path_parser = PathMatcher::new("/static/{*path}");
        let mut params = ReverseParamMap::new();
        params.insert("path", "css/app.css");

        assert_eq!(path_parser.reverse(&params).unwrap(), "/static/css/app.css");
    }

    #[test]
    #[should_panic(
        expected = "route conflict error: wildcard parameter `{*rest}` must be the last segment of pattern `/users/{*rest}/edit`; a wildcard consumes the rest of the path, so nothing can follow it"
    )]
    fn path_parser_no_path_allowed_after_wildcard() {
        let _ = PathMatcher::new("/users/{*rest}/edit");
    }

    #[test]
    #[should_panic(
        expected = "route conflict error: wildcard parameter `{*rest}` must be the last segment of pattern `/users/{*rest}/`; a wildcard consumes the rest of the path, so nothing can follow it"
    )]
    fn path_parser_trail_slash_not_allowed_after_wildcard() {
        let _ = PathMatcher::new("/users/{*rest}/");
    }

    #[test]
    #[should_panic(
        expected = "route conflict error: invalid wildcard name `` in pattern `/users/{*}`; wildcard names must start with a letter or underscore and contain only letters, digits, or underscores"
    )]
    fn path_parser_invalid_wildcard_name_empty() {
        let _ = PathMatcher::new("/users/{*}");
    }

    #[test]
    fn absolute_path_join_root_with_root() {
        assert_eq!(
            AbsolutePath::root().join(&AbsolutePath::root()).as_str(),
            "/"
        );
    }

    #[test]
    fn absolute_path_join_root_is_identity() {
        let x = AbsolutePath::new("/foo/bar");
        assert_eq!(AbsolutePath::root().join(&x), x);
    }

    #[test]
    fn absolute_path_join_trims_doubled_slash() {
        let prefix = AbsolutePath::new("/api/");
        let suffix = AbsolutePath::new("/inner");
        assert_eq!(prefix.join(&suffix).as_str(), "/api/inner");
    }

    #[test]
    fn absolute_path_join_no_trailing_slash_on_prefix() {
        let prefix = AbsolutePath::new("/api");
        let suffix = AbsolutePath::new("/inner");
        assert_eq!(prefix.join(&suffix).as_str(), "/api/inner");
    }

    #[test]
    fn absolute_path_new_normalizes_missing_leading_slash() {
        assert_eq!(AbsolutePath::new("foo").as_str(), "/foo");
    }
}
