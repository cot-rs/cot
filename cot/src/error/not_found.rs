use thiserror::Error;

#[non_exhaustive]
#[derive(Debug, Error)]
#[error(transparent)]
pub struct NotFound {
    pub kind: Kind,
}

impl NotFound {
    #[must_use]
    pub fn new() -> Self {
        Self::with_kind(Kind::Custom)
    }

    #[must_use]
    pub fn with_message<T: ToString>(message: T) -> Self {
        Self::with_kind(Kind::WithMessage(message.to_string()))
    }

    #[must_use]
    pub(crate) fn from_router() -> Self {
        Self::with_kind(Kind::FromRouter)
    }

    fn with_kind(kind: Kind) -> Self {
        NotFound { kind }
    }
}

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum Kind {
    #[error("Not Found")]
    FromRouter,
    #[error("Not Found")]
    Custom,
    #[error("Not Found: {0}")]
    WithMessage(String),
}
