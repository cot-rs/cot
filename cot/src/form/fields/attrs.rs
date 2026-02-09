use std::fmt::{Display, Formatter};

/// Represents the HTML [`step`] attribute for `<input>` elements:
/// - `Any` → `step="any"`
/// - `Value(T)` → `step="<value>"` where `T` is converted appropriately
///
/// [`step`]: https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Attributes/step
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

/// Represents the HTML [`list`] attribute for `<input>` elements.
/// Used to provide a set of predefined options for the input.
///
/// [`list`]: https://developer.mozilla.org/en-US/docs/Web/HTML/Element/input#list
#[derive(Debug, Clone, Default)]
pub struct List(Vec<String>);

impl List {
    /// Creates a new `List` from any iterator of string-like items.
    ///
    /// # Examples
    /// ```
    /// use cot::form::fields::List;
    /// let list = List::new(vec!["Option 1", "Option 2", "Option 3"]);
    /// ```
    pub fn new<I, S>(iter: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let v = iter.into_iter().map(|s| s.as_ref().to_string()).collect();
        Self(v)
    }

    /// Returns an iterator over the items in the list.
    ///
    /// # Examples
    /// ```
    /// use cot::form::fields::List;
    /// let list = List::new(vec!["Option 1", "Option 2", "Option 3"]);
    /// for item in list.iter() {
    ///     println!("{item:?}");
    /// }
    /// ```
    pub fn iter(&self) -> std::slice::Iter<'_, String> {
        self.0.iter()
    }
}

impl IntoIterator for List {
    type Item = String;
    type IntoIter = std::vec::IntoIter<String>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a List {
    type Item = &'a String;
    type IntoIter = std::slice::Iter<'a, String>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

/// Represents the HTML [`autocomplete`] attribute for form fields.
///
/// [`autocomplete`]: https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Attributes/autocomplete
#[derive(Debug, Clone)]
pub enum AutoComplete {
    /// Enables autocomplete.
    On,
    /// Disables autocomplete.
    Off,
    /// Custom autocomplete value.
    Value(String),
}

impl AutoComplete {
    /// Returns the string representation for use in HTML.
    #[must_use]
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

/// Represents the HTML [`autocapitalize`] attribute for form fields.
///
/// [`autocapitalize`]: https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Global_attributes/autocapitalize
#[derive(Debug, Clone, Copy)]
pub enum AutoCapitalize {
    /// No capitalization.
    Off,
    /// Capitalize all letters.
    On,
    /// Capitalize the first letter of each word.
    Words,
    /// Capitalize all characters.
    Characters,
}

impl AutoCapitalize {
    /// Returns the string representation for use in HTML.
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

/// Represents the HTML [`dir`] attribute for text direction.
///
/// [`dir`]: https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Global_attributes/dir
#[derive(Debug, Clone, Copy)]
pub enum Dir {
    /// Right-to-left text direction.
    Rtl,
    /// Left-to-right text direction.
    Ltr,
    /// User agent auto-detects the text direction.
    Auto,
}

impl Dir {
    /// Returns the string representation for use in HTML.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rtl => "rtl",
            Self::Ltr => "ltr",
            Self::Auto => "auto",
        }
    }
}

impl Display for Dir {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Represents the HTML [`capture`] attribute for file inputs.
/// Used to specify the preferred source for file capture.
///
/// [`capture`]: https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Elements/input/file#capture
#[derive(Debug, Clone, Copy)]
pub enum Capture {
    /// Use the user-facing camera or microphone.
    User,
    /// Use the environment-facing camera or microphone.
    Environment,
}

impl Capture {
    /// Returns the string representation for use in HTML.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Environment => "environment",
        }
    }
}

impl Display for Capture {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autocapitalize_as_str() {
        assert_eq!(AutoCapitalize::Off.as_str(), "off");
        assert_eq!(AutoCapitalize::On.as_str(), "on");
        assert_eq!(AutoCapitalize::Words.as_str(), "words");
        assert_eq!(AutoCapitalize::Characters.as_str(), "characters");
    }

    #[test]
    fn autocapitalize_to_string() {
        assert_eq!(AutoCapitalize::Off.to_string(), "off");
        assert_eq!(AutoCapitalize::On.to_string(), "on");
        assert_eq!(AutoCapitalize::Words.to_string(), "words");
        assert_eq!(AutoCapitalize::Characters.to_string(), "characters");
    }

    #[test]
    fn autocomplete_as_str() {
        assert_eq!(AutoComplete::Off.as_str(), "off");
        assert_eq!(AutoComplete::On.as_str(), "on");
        let custom = AutoComplete::Value("email".to_string());
        assert_eq!(custom.as_str(), "email");
    }

    #[test]
    fn dir_as_str() {
        assert_eq!(Dir::Rtl.as_str(), "rtl");
        assert_eq!(Dir::Ltr.as_str(), "ltr");
        assert_eq!(Dir::Auto.as_str(), "auto");
    }

    #[test]
    fn dir_to_string() {
        assert_eq!(Dir::Rtl.to_string(), "rtl");
        assert_eq!(Dir::Ltr.to_string(), "ltr");
        assert_eq!(Dir::Auto.to_string(), "auto");
    }

    #[test]
    fn list_iter() {
        let list = List::new(["Option 1", "Option 2", "Option 3"]);
        let collected: Vec<&str> = list.iter().map(|s| s.as_str()).collect();
        assert_eq!(collected, vec!["Option 1", "Option 2", "Option 3"]);
    }
}
