use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum UrlParseError {
    #[error("Invalid URL: {0}")]
    InvalidUrl(url::ParseError),
}
