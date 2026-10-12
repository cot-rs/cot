//! Form Field Types for Cot
//!
//! This module provides a collection of form field types and utilities for
//! validating, parsing, and converting user input within Cot. It includes
//! general-purpose newtype wrappers and associated trait implementations to
//! ensure consistent and safe processing of form data.

use std::fmt::{Debug, Display};
use std::str::FromStr;

#[cfg(feature = "mysql")]
use cot::db::impl_mysql::MySqlValueRef;
#[cfg(feature = "postgres")]
use cot::db::impl_postgres::PostgresValueRef;
#[cfg(feature = "sqlite")]
use cot::db::impl_sqlite::SqliteValueRef;
use cot::form::FormFieldValidationError;
use email_address::EmailAddress;
use jiff::civil::Weekday;
use securer_string::SecureString;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(feature = "db")]
use crate::db::{
    ColumnType, DatabaseField, DbValue, FromDbValue, SqlxValueRef, TextField, ToDbValue,
};

// Maximum email length as specified in the RFC 5321
const MAX_EMAIL_LENGTH: u32 = 254;

/// A password.
///
/// It is always recommended to store passwords in memory using this newtype
/// instead of a raw String, as it has a [`Debug`] implementation that hides
/// the password value.
///
/// For persisting passwords in the database, and verifying passwords against
/// the hash, use [`PasswordHash`](crate::auth::PasswordHash).
///
/// # Security
///
/// The implementation of the [`Debug`] trait for this type hides the password
/// value to prevent it from being leaked in logs or other debug output.
///
/// ## Password Comparison
///
/// When comparing passwords, there are two approaches, with different security
/// properties:
///
/// 1. **Hash-based verification (recommended).** Use
///    [`PasswordHash::from_password`](crate::auth::PasswordHash::from_password)
///    to derive a hash from one password, and then
///    [`PasswordHash::verify`](crate::auth::PasswordHash::verify) to check the
///    other password against that hash. The constant-time comparison happens on
///    the *hashes*, which are fixed-length, so it reveals nothing about the
///    passwords themselves — not even their length. On top of that, the hashing
///    function is deliberately slow and salted, which both defeats timing
///    attacks and means the stored value is useless to an attacker who obtains
///    it. This is the only approach suitable for authenticating against a
///    persisted password.
///
/// 2. **Direct comparison.** Comparing two [`Password`] instances with
///    [`PartialEq`] also uses a constant-time comparison, so it does not leak
///    *which* characters differ. However, it operates on the raw passwords
///    rather than fixed-length hashes, so the comparison can still leak the
///    *length* of the password through timing, and it performs no expensive key
///    derivation. Use this only when both values are already in memory and
///    neither is a stored credential — for example, checking that the password
///    and the "retype your password" field of a registration form match. It
///    avoids the cost of hashing in that case, but must never be used to verify
///    a password against one loaded from storage.
///
/// # Examples
///
/// ```
/// use cot::common_types::Password;
///
/// let password = Password::new("pass");
/// assert_eq!(&format!("{:?}", password), "Password(..)");
///
/// let another_password = Password::new("pass");
/// assert_eq!(password, another_password);
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct Password(SecureString);

impl Password {
    /// Creates a new password object.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::common_types::Password;
    ///
    /// let password = Password::new("password");
    /// ```
    #[must_use]
    pub fn new<T: Into<String>>(password: T) -> Self {
        Self(SecureString::from(password.into()))
    }

    /// Returns the password as a string.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::common_types::Password;
    ///
    /// let password = Password::new("password");
    /// assert_eq!(password.as_str(), "password");
    /// ```
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.unsecure()
    }

    /// Consumes the object and returns the password as a string.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::common_types::Password;
    ///
    /// let password = Password::new("password");
    /// assert_eq!(password.into_string(), "password");
    /// ```
    #[must_use]
    pub fn into_string(self) -> String {
        self.0.into_unsecure()
    }
}

impl Debug for Password {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Password").finish_non_exhaustive()
    }
}

impl From<&Password> for Password {
    fn from(password: &Password) -> Self {
        password.clone()
    }
}

impl From<&str> for Password {
    fn from(password: &str) -> Self {
        Self::new(password)
    }
}

impl From<String> for Password {
    fn from(password: String) -> Self {
        Self::new(password)
    }
}

/// A validated URL wrapper.
///
/// This structure ensures that the contained URL is correctly formatted and
/// complies with standard URL syntax rules. It wraps [`url::Url`] to provide
/// validation upon construction through methods like [`Url::new`] and
/// [`Url::from_str`], and exposes useful methods for accessing URL components
/// or converting the URL into different representations.
///
/// # Behavior
///
/// - **Validation**: Both `new` and `from_str` ensure the input is a
///   syntactically correct URL as defined by the WHATWG URL specification via
///   the underlying [`url::Url`] parser.
/// - **Normalization**: The internal URL is normalized (e.g., trailing slash
///   added for HTTP URLs) during construction.
///
/// # Examples
///
/// ## Creating a Validated URL Using `new`
///
/// ```
/// use cot::common_types::Url;
///
/// // Successful URL creation
/// let url = Url::new("https://example.com").unwrap();
///
/// // Accessing the normalized URL string
/// assert_eq!(url.as_str(), "https://example.com/");
/// ```
///
/// ## Parsing a URL from a [`String`] Using `from_str`
///
/// ```
/// use std::str::FromStr;
///
/// use cot::common_types::Url;
///
/// // Parse a valid URL string
/// let url = Url::from_str("https://example.com").unwrap();
///
/// // Convert into owned string representation
/// let url_string = url.into_string();
/// assert_eq!(url_string, "https://example.com/");
/// ```
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Url(url::Url);

impl Url {
    /// Creates a new `Url` by parsing the input string.
    ///
    /// # Errors
    ///
    /// Returns [`UrlParseError`] if the input string is not a valid URL.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::common_types::Url;
    ///
    /// let valid_url = Url::new("https://example.com").unwrap();
    /// ```
    pub fn new<S: AsRef<str>>(s: S) -> Result<Url, UrlParseError> {
        url::Url::from_str(s.as_ref())
            .map(Self)
            .map_err(UrlParseError)
    }

    /// Returns a string slice reference to the URL's string representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Converts the `Url` into an owned `String` representation.
    ///
    /// # Examples
    /// ```
    /// use cot::common_types::Url;
    ///
    /// let url = Url::new("https://example.com").unwrap();
    /// let url_string = url.into_string();
    /// assert_eq!(url_string, "https://example.com/");
    /// ```
    #[must_use]
    pub fn into_string(self) -> String {
        self.0.into()
    }
    /// Returns the URL scheme (e.g., "http", "https").
    ///
    /// # Example
    /// ```
    /// use cot::common_types::Url;
    ///
    /// let url = Url::new("https://example.com").unwrap();
    /// assert_eq!(url.scheme(), "https");
    /// ```
    #[must_use]
    pub fn scheme(&self) -> &str {
        self.0.scheme()
    }

    /// Returns the host part of the URL, if present.
    ///
    /// This typically includes the domain name or IP address. It returns
    /// [`None`] if the URL has no host.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::common_types::Url;
    ///
    /// let url = Url::new("https://example.com/path").unwrap();
    /// assert_eq!(url.host(), Some("example.com"));
    /// ```
    #[must_use]
    pub fn host(&self) -> Option<&str> {
        self.0.host_str()
    }

    /// Returns the path component of the URL.
    ///
    /// This includes everything after the host and before the query or
    /// fragment.
    ///
    /// # Example
    /// ```
    /// use cot::common_types::Url;
    ///
    /// let url = Url::new("https://example.com/foo/bar").unwrap();
    /// assert_eq!(url.path(), "/foo/bar");
    /// ```
    #[must_use]
    pub fn path(&self) -> &str {
        self.0.path()
    }

    /// Returns the query string of the URL, if present.
    ///
    /// The query is the part that follows the '?' character.
    ///
    /// # Example
    /// ```
    /// use cot::common_types::Url;
    ///
    /// let url = Url::new("https://example.com?query=1").unwrap();
    /// assert_eq!(url.query(), Some("query=1"));
    /// ```
    #[must_use]
    pub fn query(&self) -> Option<&str> {
        self.0.query()
    }

    /// Returns the fragment identifier of the URL, if present.
    ///
    /// The fragment is the part that follows the '#' character.
    ///
    /// # Example
    /// ```
    /// use cot::common_types::Url;
    ///
    /// let url = Url::new("https://example.com#section-1").unwrap();
    /// assert_eq!(url.fragment(), Some("section-1"));
    /// ```
    #[must_use]
    pub fn fragment(&self) -> Option<&str> {
        self.0.fragment()
    }
}

/// Implements string parsing for `Url`.
///
/// This allows a string to be parsed directly into a validated [`Url`]
/// instance. The parsing process ensures that the input string is a
/// syntactically valid URL.
///
/// # Errors
///
/// Returns [`UrlParseError`] if the input string is not a valid URL format.
///
/// # Examples
///
/// ```
/// use std::str::FromStr;
///
/// use cot::common_types::Url;
///
/// // Parsing a valid URL string
/// let url = Url::from_str("https://example.com").unwrap();
/// assert_eq!(url.as_str(), "https://example.com/");
///
/// // Attempting to parse an invalid URL
/// assert!(Url::from_str("not-a-url").is_err());
/// ```
impl FromStr for Url {
    type Err = UrlParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Url::new(s)
    }
}

impl Display for Url {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A type that represents an error that occurs when parsing a URL.
///
/// This is returned by [`Url::new`] and [`Url::from_str`] when the input string
/// is not a valid URL.
#[derive(Debug, Error)]
#[error(transparent)]
#[expect(missing_copy_implementations)] // implementation detail
pub struct UrlParseError(url::ParseError);

impl From<UrlParseError> for FormFieldValidationError {
    fn from(error: UrlParseError) -> Self {
        FormFieldValidationError::from_string(error.to_string())
    }
}

#[cfg(feature = "db")]
impl ToDbValue for Url {
    fn to_db_value(&self) -> DbValue {
        self.0.clone().to_string().into()
    }
}

#[cfg(feature = "db")]
impl FromDbValue for Url {
    #[cfg(feature = "sqlite")]
    fn from_sqlite(value: SqliteValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        Url::new(value.get::<String>()?).map_err(cot::db::DatabaseError::value_decode)
    }

    #[cfg(feature = "postgres")]
    fn from_postgres(value: PostgresValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        Url::new(value.get::<String>()?).map_err(cot::db::DatabaseError::value_decode)
    }

    #[cfg(feature = "mysql")]
    fn from_mysql(value: MySqlValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        Url::new(value.get::<String>()?).map_err(cot::db::DatabaseError::value_decode)
    }
}

#[cfg(feature = "db")]
impl FromDbValue for Option<Url> {
    #[cfg(feature = "sqlite")]
    fn from_sqlite(value: SqliteValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        value
            .get::<Option<String>>()
            .map(|opt_str| opt_str.map(Url::new))?
            .transpose()
            .map_err(cot::db::DatabaseError::value_decode)
    }

    #[cfg(feature = "postgres")]
    fn from_postgres(value: PostgresValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        value
            .get::<Option<String>>()
            .map(|opt_str| opt_str.map(Url::new))?
            .transpose()
            .map_err(cot::db::DatabaseError::value_decode)
    }

    #[cfg(feature = "mysql")]
    fn from_mysql(value: MySqlValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        value
            .get::<Option<String>>()
            .map(|opt_str| opt_str.map(Url::new))?
            .transpose()
            .map_err(cot::db::DatabaseError::value_decode)
    }
}

#[cfg(feature = "db")]
impl ToDbValue for Option<Url> {
    fn to_db_value(&self) -> DbValue {
        self.clone().map(Url::into_string).into()
    }
}

#[cfg(feature = "db")]
impl DatabaseField for Url {
    const TYPE: ColumnType = ColumnType::Text;
}

#[cfg(feature = "db")]
impl TextField for Url {}

/// A validated email address.
///
/// This is a newtype wrapper around [`EmailAddress`] that provides validation
/// and integration with Cot's database system. It ensures email addresses
/// comply with RFC 5321/5322 standards.
///
/// # Examples
///
/// ```
/// use std::str::FromStr;
///
/// use cot::common_types::Email;
///
/// // Parse from a string
/// let email = Email::from_str("user@example.com").unwrap();
///
/// // Convert using TryFrom
/// let email = Email::try_from("user@example.com").unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Email(EmailAddress);

impl Email {
    /// Creates a new `Email` from a string, validating that it is a proper
    /// email address.
    ///
    /// # Errors
    ///
    /// Returns an error if the email address is invalid according to RFC
    /// standards.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::common_types::Email;
    ///
    /// let email = Email::new("user@example.com").unwrap();
    /// assert!(Email::new("invalid").is_err());
    /// ```
    pub fn new<S: AsRef<str>>(email: S) -> Result<Email, EmailParseError> {
        EmailAddress::from_str(email.as_ref())
            .map(Self)
            .map_err(EmailParseError)
    }

    /// Returns the email address as a string.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::str::FromStr;
    ///
    /// use cot::common_types::Email;
    ///
    /// let email = Email::from_str("user@example.com").unwrap();
    /// assert_eq!(email.as_str(), "user@example.com");
    /// ```
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Returns the domain part of the email address (the part after the '@'
    /// symbol).
    ///
    /// # Examples
    ///
    /// ```
    /// use std::str::FromStr;
    ///
    /// use cot::common_types::Email;
    ///
    /// let email = Email::from_str("user@example.com").unwrap();
    /// assert_eq!(email.domain(), "example.com");
    /// ```
    #[must_use]
    pub fn domain(&self) -> &str {
        self.0.domain()
    }

    /// Formats the email address as a URI, typically for use in `mailto:`
    /// links.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::str::FromStr;
    ///
    /// use cot::common_types::Email;
    ///
    /// let email = Email::from_str("user@example.com").unwrap();
    /// assert_eq!(email.to_uri(), "mailto:user@example.com");
    /// ```
    #[must_use]
    pub fn to_uri(&self) -> String {
        self.0.to_uri()
    }

    /// Formats the email address with a display name.
    ///
    /// This creates a formatted email address with the format: `"Display Name"
    /// <user@example.com>`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::str::FromStr;
    ///
    /// use cot::common_types::Email;
    ///
    /// let email = Email::from_str("user@example.com").unwrap();
    /// assert_eq!(email.to_display("John Doe"), "John Doe <user@example.com>");
    /// ```
    #[must_use]
    pub fn to_display(&self, display_name: &str) -> String {
        self.0.to_display(display_name)
    }

    /// Returns the full email address as a string.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::str::FromStr;
    ///
    /// use cot::common_types::Email;
    ///
    /// let email = Email::from_str("user@example.com").unwrap();
    /// assert_eq!(email.email(), "user@example.com");
    /// ```
    #[must_use]
    pub fn email(&self) -> String {
        self.0.email()
    }

    /// Returns the local part of the email address (the part before the '@'
    /// symbol).
    ///
    /// # Examples
    ///
    /// ```
    /// use std::str::FromStr;
    ///
    /// use cot::common_types::Email;
    ///
    /// let email = Email::from_str("user@example.com").unwrap();
    /// assert_eq!(email.local_part(), "user");
    /// ```
    #[must_use]
    pub fn local_part(&self) -> &str {
        self.0.local_part()
    }

    /// Returns the display part of the email address.
    ///
    /// For simple email addresses, this is typically the same as the local
    /// part. For email addresses with display names, this returns the
    /// display name portion.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::str::FromStr;
    ///
    /// use cot::common_types::Email;
    ///
    /// let email = Email::from_str("Name <name@example.org>").unwrap();
    /// assert_eq!(email.display_part(), "Name".to_owned());
    /// ```
    #[must_use]
    pub fn display_part(&self) -> &str {
        self.0.display_part()
    }
}

/// Implements string parsing for `Email`.
///
/// # Examples
///
/// ```
/// use std::str::FromStr;
///
/// use cot::common_types::Email;
///
/// let email = Email::from_str("user@example.com").unwrap();
/// ```
impl FromStr for Email {
    type Err = EmailParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Email::new(s)
    }
}

/// Implements conversion from string references to `Email`.
///
/// # Examples
///
/// ```
/// use cot::common_types::Email;
///
/// let email = Email::try_from("user@example.com").unwrap();
/// ```
impl TryFrom<&str> for Email {
    type Error = EmailParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Email::new(value)
    }
}

/// Implements conversion from `String` to `Email`.
///
/// # Examples
///
/// ```
/// use cot::common_types::Email;
///
/// let email = Email::try_from(String::from("user@example.com")).unwrap();
/// ```
impl TryFrom<String> for Email {
    type Error = EmailParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Email::new(value)
    }
}

/// A type that represents an error that occurs when parsing an email address.
///
/// This is returned by [`Email::new`] and [`Email::from_str`] when the input
/// string is not a valid email address.
#[derive(Debug, Error)]
#[error(transparent)]
pub struct EmailParseError(email_address::Error);

impl From<EmailParseError> for FormFieldValidationError {
    fn from(error: EmailParseError) -> Self {
        FormFieldValidationError::from_string(error.to_string())
    }
}

/// Implements database value conversion for `Email`.
///
/// This allows a normalized `Email` to be stored in the database as a text
/// value.
#[cfg(feature = "db")]
impl ToDbValue for Email {
    fn to_db_value(&self) -> DbValue {
        self.0.clone().email().into()
    }
}

/// Implements database value conversion for retrieving `Email` from the
/// database.
///
/// This allows `Email` to be retrieved from the database and properly converted
/// and validated.
#[cfg(feature = "db")]
impl FromDbValue for Email {
    #[cfg(feature = "sqlite")]
    fn from_sqlite(value: SqliteValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        Email::new(value.get::<String>()?).map_err(cot::db::DatabaseError::value_decode)
    }

    #[cfg(feature = "postgres")]
    fn from_postgres(value: PostgresValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        Email::new(value.get::<String>()?).map_err(cot::db::DatabaseError::value_decode)
    }

    #[cfg(feature = "mysql")]
    fn from_mysql(value: MySqlValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        Email::new(value.get::<String>()?).map_err(cot::db::DatabaseError::value_decode)
    }
}

#[cfg(feature = "db")]
impl ToDbValue for Option<Email> {
    fn to_db_value(&self) -> DbValue {
        self.clone().map(|email| email.email()).into()
    }
}

#[cfg(feature = "db")]
impl FromDbValue for Option<Email> {
    #[cfg(feature = "sqlite")]
    fn from_sqlite(value: SqliteValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        value
            .get::<Option<String>>()
            .map(|opt_str| opt_str.map(Email::new))?
            .transpose()
            .map_err(cot::db::DatabaseError::value_decode)
    }

    #[cfg(feature = "postgres")]
    fn from_postgres(value: PostgresValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        value
            .get::<Option<String>>()
            .map(|opt_str| opt_str.map(Email::new))?
            .transpose()
            .map_err(cot::db::DatabaseError::value_decode)
    }

    #[cfg(feature = "mysql")]
    fn from_mysql(value: MySqlValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        value
            .get::<Option<String>>()
            .map(|opt_str| opt_str.map(Email::new))?
            .transpose()
            .map_err(cot::db::DatabaseError::value_decode)
    }
}

/// Defines the database field type for `Email`.
///
/// Emails are stored as strings with a maximum length of 254 characters,
/// as specified in RFC 5321.
#[cfg(feature = "db")]
impl DatabaseField for Email {
    const TYPE: ColumnType = ColumnType::String(MAX_EMAIL_LENGTH);
}

impl Display for Email {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0.as_str())
    }
}

#[cfg(feature = "db")]
impl TextField for Email {}

/// A set of weekdays.
///
/// This is useful, for instance, for storing a weekly schedule in the
/// database. The set is stored as a single integer, where each bit represents
/// one weekday.
///
/// # Examples
///
/// ```
/// use cot::common_types::WeekdaySet;
/// use jiff::civil::Weekday;
///
/// let mut set = WeekdaySet::EMPTY;
/// set.insert(Weekday::Monday);
/// set.insert(Weekday::Friday);
///
/// assert!(set.contains(Weekday::Monday));
/// assert!(!set.contains(Weekday::Tuesday));
/// assert_eq!(
///     set.iter().collect::<Vec<_>>(),
///     [Weekday::Monday, Weekday::Friday]
/// );
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WeekdaySet(u8);

impl WeekdaySet {
    /// An empty set.
    pub const EMPTY: Self = Self(0);

    /// A set containing all the weekdays.
    pub const ALL: Self = Self(0b111_1111);

    /// Creates a set containing a single weekday.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::common_types::WeekdaySet;
    /// use jiff::civil::Weekday;
    ///
    /// let set = WeekdaySet::single(Weekday::Sunday);
    /// assert_eq!(set.len(), 1);
    /// assert!(set.contains(Weekday::Sunday));
    /// ```
    #[must_use]
    pub fn single(weekday: Weekday) -> Self {
        Self(1 << weekday.to_monday_zero_offset())
    }

    /// Returns `true` if the set contains the given weekday.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::common_types::WeekdaySet;
    /// use jiff::civil::Weekday;
    ///
    /// assert!(WeekdaySet::ALL.contains(Weekday::Monday));
    /// assert!(!WeekdaySet::EMPTY.contains(Weekday::Monday));
    /// ```
    #[must_use]
    pub fn contains(self, weekday: Weekday) -> bool {
        self.0 & Self::single(weekday).0 != 0
    }

    /// Adds a weekday to the set.
    ///
    /// Returns `true` if the weekday was not already in the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::common_types::WeekdaySet;
    /// use jiff::civil::Weekday;
    ///
    /// let mut set = WeekdaySet::EMPTY;
    /// assert!(set.insert(Weekday::Monday));
    /// assert!(!set.insert(Weekday::Monday));
    /// ```
    pub fn insert(&mut self, weekday: Weekday) -> bool {
        let was_missing = !self.contains(weekday);
        self.0 |= Self::single(weekday).0;
        was_missing
    }

    /// Removes a weekday from the set.
    ///
    /// Returns `true` if the weekday was in the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::common_types::WeekdaySet;
    /// use jiff::civil::Weekday;
    ///
    /// let mut set = WeekdaySet::ALL;
    /// assert!(set.remove(Weekday::Monday));
    /// assert!(!set.remove(Weekday::Monday));
    /// ```
    pub fn remove(&mut self, weekday: Weekday) -> bool {
        let was_present = self.contains(weekday);
        self.0 &= !Self::single(weekday).0;
        was_present
    }

    /// Returns `true` if the set contains no weekdays.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::common_types::WeekdaySet;
    ///
    /// assert!(WeekdaySet::EMPTY.is_empty());
    /// assert!(!WeekdaySet::ALL.is_empty());
    /// ```
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Returns the number of weekdays in the set.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::common_types::WeekdaySet;
    ///
    /// assert_eq!(WeekdaySet::EMPTY.len(), 0);
    /// assert_eq!(WeekdaySet::ALL.len(), 7);
    /// ```
    #[must_use]
    pub const fn len(self) -> u32 {
        self.0.count_ones()
    }

    /// Returns an iterator over the weekdays in the set, starting from Monday.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::common_types::WeekdaySet;
    /// use jiff::civil::Weekday;
    ///
    /// let set: WeekdaySet = [Weekday::Sunday, Weekday::Monday].into_iter().collect();
    /// assert_eq!(
    ///     set.iter().collect::<Vec<_>>(),
    ///     [Weekday::Monday, Weekday::Sunday]
    /// );
    /// ```
    pub fn iter(self) -> impl Iterator<Item = Weekday> {
        Weekday::Monday
            .cycle_forward()
            .take(7)
            .filter(move |&weekday| self.contains(weekday))
    }

    #[cfg(feature = "db")]
    const fn from_bits(bits: u8) -> Self {
        Self(bits & Self::ALL.0)
    }
}

impl FromIterator<Weekday> for WeekdaySet {
    fn from_iter<T: IntoIterator<Item = Weekday>>(iter: T) -> Self {
        let mut set = Self::EMPTY;
        for weekday in iter {
            set.insert(weekday);
        }
        set
    }
}

#[cfg(feature = "fake")]
impl fake::Dummy<fake::Faker> for WeekdaySet {
    fn dummy_with_rng<R: fake::rand::Rng + ?Sized>(_: &fake::Faker, rng: &mut R) -> Self {
        Self(rng.next_u32().to_le_bytes()[0] & Self::ALL.0)
    }
}

/// Weekday sets are stored as a bit mask, where the least significant bit
/// represents Monday.
#[cfg(feature = "db")]
impl DatabaseField for WeekdaySet {
    const TYPE: ColumnType = ColumnType::TinyUnsignedInteger;
}

#[cfg(feature = "db")]
impl ToDbValue for WeekdaySet {
    fn to_db_value(&self) -> DbValue {
        self.0.to_db_value()
    }
}

#[cfg(feature = "db")]
impl ToDbValue for Option<WeekdaySet> {
    fn to_db_value(&self) -> DbValue {
        self.map(|set| set.0).to_db_value()
    }
}

#[cfg(feature = "db")]
impl FromDbValue for WeekdaySet {
    #[cfg(feature = "sqlite")]
    fn from_sqlite(value: SqliteValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        <u8 as FromDbValue>::from_sqlite(value).map(Self::from_bits)
    }

    #[cfg(feature = "postgres")]
    fn from_postgres(value: PostgresValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        <u8 as FromDbValue>::from_postgres(value).map(Self::from_bits)
    }

    #[cfg(feature = "mysql")]
    fn from_mysql(value: MySqlValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        <u8 as FromDbValue>::from_mysql(value).map(Self::from_bits)
    }
}

#[cfg(feature = "db")]
impl FromDbValue for Option<WeekdaySet> {
    #[cfg(feature = "sqlite")]
    fn from_sqlite(value: SqliteValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        <Option<u8> as FromDbValue>::from_sqlite(value).map(|bits| bits.map(WeekdaySet::from_bits))
    }

    #[cfg(feature = "postgres")]
    fn from_postgres(value: PostgresValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        <Option<u8> as FromDbValue>::from_postgres(value)
            .map(|bits| bits.map(WeekdaySet::from_bits))
    }

    #[cfg(feature = "mysql")]
    fn from_mysql(value: MySqlValueRef<'_>) -> cot::db::Result<Self>
    where
        Self: Sized,
    {
        <Option<u8> as FromDbValue>::from_mysql(value).map(|bits| bits.map(WeekdaySet::from_bits))
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;

    use super::*;
    use crate::Template;

    #[test]
    fn weekday_set_empty() {
        assert!(WeekdaySet::EMPTY.is_empty());
        assert_eq!(WeekdaySet::EMPTY.len(), 0);
        assert_eq!(WeekdaySet::EMPTY.iter().count(), 0);
        assert_eq!(WeekdaySet::default(), WeekdaySet::EMPTY);
    }

    #[test]
    fn weekday_set_single() {
        let monday = WeekdaySet::single(Weekday::Monday);
        assert_eq!(monday.0, 0b000_0001);
        assert!(monday.contains(Weekday::Monday));
        assert!(!monday.contains(Weekday::Tuesday));

        let sunday = WeekdaySet::single(Weekday::Sunday);
        assert_eq!(sunday.0, 0b100_0000);
    }

    #[test]
    fn weekday_set_insert_remove() {
        let mut set = WeekdaySet::EMPTY;

        assert!(set.insert(Weekday::Monday));
        assert!(!set.insert(Weekday::Monday));
        assert!(set.insert(Weekday::Friday));
        assert_eq!(set.len(), 2);

        assert!(set.remove(Weekday::Monday));
        assert!(!set.remove(Weekday::Monday));
        assert_eq!(set, WeekdaySet::single(Weekday::Friday));
    }

    #[test]
    fn weekday_set_iter() {
        let set: WeekdaySet = [Weekday::Sunday, Weekday::Wednesday, Weekday::Monday]
            .into_iter()
            .collect();

        assert_eq!(
            set.iter().collect::<Vec<_>>(),
            [Weekday::Monday, Weekday::Wednesday, Weekday::Sunday]
        );
        assert_eq!(WeekdaySet::ALL.iter().count(), 7);
    }

    #[cfg(feature = "db")]
    #[test]
    fn weekday_set_from_bits() {
        assert_eq!(WeekdaySet::from_bits(0b101_0101).0, 0b101_0101);
        // the most significant bit is ignored
        assert_eq!(WeekdaySet::from_bits(0b1111_1111), WeekdaySet::ALL);
    }

    #[cfg(feature = "db")]
    #[test]
    fn weekday_set_to_db_value() {
        let set: WeekdaySet = [Weekday::Monday, Weekday::Friday].into_iter().collect();

        assert_eq!(set.to_db_value(), DbValue::TinyUnsigned(Some(0b001_0001)));
        assert_eq!(
            Some(set).to_db_value(),
            DbValue::TinyUnsigned(Some(0b001_0001))
        );
        assert_eq!(
            None::<WeekdaySet>.to_db_value(),
            DbValue::TinyUnsigned(None)
        );
    }

    #[cfg(feature = "db")]
    #[test]
    fn weekday_set_column_type() {
        assert_eq!(
            <WeekdaySet as DatabaseField>::TYPE,
            ColumnType::TinyUnsignedInteger
        );
    }

    #[test]
    fn url_new() {
        let parse_url = Url::new("https://example.com/").unwrap();
        assert_eq!(parse_url.as_str(), "https://example.com/");
        assert_eq!(parse_url.to_string(), "https://example.com/");
        assert_eq!(parse_url.scheme(), "https");
        assert_eq!(parse_url.host(), Some("example.com"));
    }

    #[test]
    fn url_new_normalize() {
        let parse_url = Url::new("https://example.com").unwrap();
        assert_eq!(parse_url.as_str(), "https://example.com/"); // Normalizes to add trailing slash
    }

    #[test]
    fn askama_renders_url_field() {
        #[derive(Template)]
        #[template(source = "{{ url }}", ext = "html")]
        struct UrlTestTemplate {
            url: Url,
        }

        let url = Url::new("https://example.com").unwrap();
        let tpl = UrlTestTemplate { url };
        let rendered = tpl.render().expect("template failed to render");
        assert_eq!(rendered, "https://example.com/");
    }

    #[test]
    fn password_debug() {
        let password = Password::new("password");
        assert_eq!(format!("{password:?}"), "Password(..)");
    }

    #[test]
    fn password_eq() {
        let password1 = Password::new("password");
        let password2 = Password::new("password");
        assert_eq!(password1, password2);
    }

    #[test]
    fn password_ne() {
        let password1 = Password::new("password");
        let password2 = Password::new("password2");
        assert_ne!(password1, password2);
    }

    #[test]
    fn password_str() {
        let password = Password::new("password");
        assert_eq!(password.as_str(), "password");
        assert_eq!(password.into_string(), "password");
    }

    #[test]
    fn test_valid_email_creation() {
        let email = Email::new("user@example.com").unwrap();
        assert_eq!(email.as_str(), "user@example.com");
        assert_eq!(email.to_string(), "user@example.com");
        assert_eq!(email.domain(), "example.com");
    }

    #[test]
    fn test_invalid_email_creation() {
        let result = Email::new("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_from_str_trait() {
        let email: Email = "user@example.com".parse().unwrap();
        assert_eq!(email.as_str(), "user@example.com");
    }

    #[test]
    fn test_try_from_trait() {
        let email = Email::try_from("user@example.com").unwrap();
        assert_eq!(email.as_str(), "user@example.com");
    }

    #[test]
    fn askama_renders_email_field() {
        #[derive(Template)]
        #[template(source = "{{ email }}", ext = "html")]
        struct EmailTestTemplate {
            email: Email,
        }

        let email = Email::new("foo@example.com").unwrap();
        let tpl = EmailTestTemplate { email };
        let rendered = tpl.render().expect("template failed to render");
        assert_eq!(rendered, "foo@example.com");
    }

    #[cfg(feature = "db")]
    mod db_tests {
        use super::*;
        use crate::db::ToDbValue;

        #[test]
        fn test_email_to_db_value() {
            let email = Email::new("user@example.com").unwrap();
            let db_value = email.to_db_value();

            let email_str = email.as_str();
            let db_value_str = format!("{db_value:?}");
            assert!(db_value_str.contains(email_str));
        }

        #[test]
        fn test_email_to_db_value_is_normalized() {
            let with_display = Email::new("John Doe <user@example.com>").unwrap();
            let bare = Email::new("user@example.com").unwrap();

            let db1 = with_display.to_db_value();
            let db2 = bare.to_db_value();

            assert_eq!(db1, db2);
        }

        #[test]
        fn test_url_to_db_value() {
            let url = Url::new("https://example.com/").unwrap();
            let db_value = url.to_db_value();

            let url_str = url.as_str();
            let db_value_str = format!("{db_value:?}");
            assert!(db_value_str.contains(url_str));
        }
    }
}
