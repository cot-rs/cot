use std::fmt::{Display, Formatter};

/// Represents the HTML `step` attribute for `<input>` elements:
/// - `Any` → `step="any"`
/// - `Value(T)` → `step="<value>"` where `T` is converted appropriately
#[derive(Debug, Copy, Clone)]
pub enum Step<T> {
    /// Indicates that the user may enter any value (no fixed “step” interval).
    ///
    /// Corresponds to `step="any"` in HTML.
    Any,

    /// Indicates a fixed interval (step size) of type `T`.
    ///
    /// When rendered to HTML, this becomes `step="<value>"`, where `<value>` is
    /// obtained by converting the enclosed `T` to a string in the format the
    /// browser expects.
    Value(T),
}

impl<T: Display> Display for Step<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Step::Any => write!(f, "any"),
            Step::Value(value) => Display::fmt(value, f),
        }
    }
}
