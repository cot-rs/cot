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

#[derive(Debug, Clone, Default)]
pub struct List(Vec<String>);

impl List {
    pub fn new<I, S>(iter: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let v = iter.into_iter().map(|s| s.as_ref().to_string()).collect();
        Self(v)
    }
}

impl From<List> for Vec<String> {
    fn from(value: List) -> Self {
        value.0
    }
}

#[derive(Debug, Clone)]
pub enum AutoComplete {
    On,
    Off,
    Value(String),
}

impl AutoComplete {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Off => "off",
            Self::On => "on",
            Self::Value(value) => value.as_str(),
        }
    }
}
impl Display for AutoComplete {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
#[derive(Debug, Clone, Copy)]
pub enum AutoCapitalize {
    Off,
    On,
    Words,
    Characters,
}

impl AutoCapitalize {
    fn as_str(self) -> &'static str {
        match self {
            AutoCapitalize::Off => "off",
            AutoCapitalize::On => "on",
            AutoCapitalize::Words => "words",
            AutoCapitalize::Characters => "characters",
        }
    }
}

impl Display for AutoCapitalize {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Dir {
    Rtl,
    Ltr,
}

impl Dir {
    pub fn as_str(&self) -> &'static str {
        match self{
            Self::Rtl => "rtl",
            Self::Ltr => "ltr"
        }
    }
}

impl Display for Dir {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}



#[derive(Debug, Clone, Copy)]
pub enum Capture {
    User,
    Environment
}


impl Capture{
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Environment => "environment"
        }
    }
}

impl Display for Capture {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}