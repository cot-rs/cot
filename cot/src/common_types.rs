//! Form Field Types for Cot
//!
//! This module provides a collection of form field types and utilities for
//! validating, parsing, and converting user input within Cot. It includes
//! general-purpose newtype wrappers and associated trait implementations to
//! ensure consistent and safe processing of form data.

use std::fmt;
use std::fmt::{Debug, Display, Formatter};
use std::str::FromStr;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime, ParseError};
use cot::db;
#[cfg(feature = "mysql")]
use cot::db::impl_mysql::MySqlValueRef;
#[cfg(feature = "postgres")]
use cot::db::impl_postgres::PostgresValueRef;
#[cfg(feature = "sqlite")]
use cot::db::impl_sqlite::SqliteValueRef;
use cot::db::{DbFieldValue, ToDbFieldValue};
use email_address::EmailAddress;

#[cfg(feature = "db")]
use crate::db::{ColumnType, DatabaseField, DbValue, FromDbValue, SqlxValueRef, ToDbValue};

// Maximum email length as specified in the RFC 5321
const MAX_EMAIL_LENGTH: u32 = 254;

/// A password.
///
/// It is always recommended to store passwords in memory using this newtype
/// instead of a raw String, as it has a [`Debug`] implementation that hides
/// the password value.
///
/// For persisting passwords in the database, and verifying passwords against
/// the hash, use [`PasswordHash`].
///
/// # Security
///
/// The implementation of the [`Debug`] trait for this type hides the password
/// value to prevent it from being leaked in logs or other debug output.
///
/// ## Password Comparison
///
/// When comparing passwords, there are two recommended approaches:
///
/// 1. The most secure approach is to use [`PasswordHash::from_password`] to
///    create a hash from one password, and then use [`PasswordHash::verify`] to
///    compare it with the other password. This method uses constant-time
///    equality comparison, which protects against timing attacks.
///
/// 2. An alternative is to use the [`Password::as_str`] method and compare the
///    strings directly. This approach uses non-constant-time comparison, which
///    is less secure but may be acceptable in certain legitimate use cases
///    where the security tradeoff is understood, e.g., when you're creating a
///    user registration form with the "retype your password" field, where both
///    passwords come from the same source anyway.
///
/// # Examples
///
/// ```
/// use cot::auth::Password;
///
/// let password = Password::new("pass");
/// assert_eq!(&format!("{:?}", password), "Password(\"**********\")");
/// ```
#[derive(Clone)]
pub struct Password(String);

impl Debug for Password {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Password").field(&"**********").finish()
    }
}

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
        Self(password.into())
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
        &self.0
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
        self.0
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

/// A validated email address.
///
/// This is a newtype wrapper around
/// [`EmailAddress`](email_address::EmailAddress) that provides validation and
/// integration with Cot's database system. It ensures email addresses
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
#[derive(Clone, Debug)]
pub struct Email(EmailAddress);

impl Email {
    /// Creates a new `Email` from a string, validating that it's a proper email
    /// address.
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
    pub fn new<S: AsRef<str>>(email: S) -> Result<Email, email_address::Error> {
        EmailAddress::from_str(email.as_ref()).map(Self)
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
    type Err = email_address::Error;

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
    type Error = email_address::Error;

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
    type Error = email_address::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Email::new(value)
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
    fn from_sqlite(value: SqliteValueRef<'_>) -> db::Result<Self>
    where
        Self: Sized,
    {
        Email::new(value.get::<String>()?).map_err(db::DatabaseError::value_decode)
    }

    #[cfg(feature = "postgres")]
    fn from_postgres(value: PostgresValueRef<'_>) -> db::Result<Self>
    where
        Self: Sized,
    {
        Email::new(value.get::<String>()?).map_err(db::DatabaseError::value_decode)
    }

    #[cfg(feature = "mysql")]
    fn from_mysql(value: MySqlValueRef<'_>) -> db::Result<Self>
    where
        Self: Sized,
    {
        Email::new(value.get::<String>()?).map_err(db::DatabaseError::value_decode)
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

/// A validated date and time without timezone information.
///
/// This is a newtype wrapper around [`NaiveDateTime`](chrono::NaiveDateTime)
/// that provides consistent parsing and integration with Cot's database system.
/// It ensures date-time values are properly validated and formatted for use
/// in forms and database operations.
///
/// The type primarily expects RFC 3339-style local datetime strings in the
/// format `YYYY-MM-DDTHH:MM:SS`, but also provides convenience methods for
/// HTML5 `datetime-local` input format.
///
/// # Examples
///
/// ```
/// use std::str::FromStr;
///
/// use cot::common_types::DateTime;
///
/// // Parse from ISO 8601 format
/// let dt = DateTime::from_str("2025-05-27T13:03:00").unwrap();
///
/// // Parse from HTML datetime-local format (no seconds)
/// let dt = DateTime::from_datetime_local("2025-05-27T13:03").unwrap();
///
/// // Convert using TryFrom
/// let dt = DateTime::try_from("2025-05-27T13:03:00").unwrap();
/// ```
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DateTime(NaiveDateTime);

impl DateTime {
    /// Creates a new `DateTime` from a string in RFC 3339 local format.
    ///
    /// Expects the format `YYYY-MM-DDTHH:MM:SS` where seconds are mandatory.
    /// This is the standard ISO 8601 format without timezone information.
    ///
    /// # Errors
    ///
    /// Returns a [`ParseError`] if the input string doesn't match the expected
    /// format or contains invalid date/time values.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::common_types::DateTime;
    ///
    /// let dt = DateTime::new("2025-05-27T13:03:00").unwrap();
    /// assert!(DateTime::new("invalid").is_err());
    /// assert!(DateTime::new("2025-05-27T13:03").is_err()); // missing seconds
    /// ```
    pub fn new<S: AsRef<str>>(s: S) -> Result<Self, ParseError> {
        Self::with_format(s.as_ref(), "%Y-%m-%dT%H:%M:%S")
    }

    /// Creates a new `DateTime` from a string using a custom format.
    ///
    /// This method allows parsing datetime strings in formats other than the
    /// default RFC 3339 format. The format string uses the same syntax as
    /// [`chrono::NaiveDateTime::parse_from_str`].
    ///
    /// # Errors
    ///
    /// Returns a [`ParseError`] if the input string doesn't match the specified
    /// format or contains invalid date/time values.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::common_types::DateTime;
    ///
    /// // Parse US format
    /// let dt = DateTime::with_format("05/27/2025 1:03:00 PM", "%m/%d/%Y %I:%M:%S %p").unwrap();
    ///
    /// // Parse without seconds
    /// let dt = DateTime::with_format("2025-05-27T13:03", "%Y-%m-%dT%H:%M").unwrap();
    /// ```
    pub fn with_format<S: AsRef<str>>(s: S, format: &str) -> Result<Self, ParseError> {
        NaiveDateTime::parse_from_str(s.as_ref(), format).map(Self)
    }

    /// Creates a new `DateTime` from HTML5 `datetime-local` input format.
    ///
    /// This is a convenience method for parsing the format used by HTML5
    /// `<input type="datetime-local">` elements, which uses `YYYY-MM-DDTHH:MM`
    /// (without seconds). The method automatically appends `:00` for seconds.
    ///
    /// # Errors
    ///
    /// Returns a [`ParseError`] if the input string doesn't match the expected
    /// `datetime-local` format or contains invalid date/time values.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::common_types::DateTime;
    ///
    /// let dt = DateTime::from_datetime_local("2025-05-27T13:03").unwrap();
    /// assert_eq!(dt.to_local_string(), "2025-05-27T13:03:00");
    /// ```
    pub fn from_datetime_local<S: AsRef<str>>(s: S) -> Result<Self, ParseError> {
        // parse without seconds, then append ":00"
        let mut buf = s.as_ref().to_string();
        buf.push_str(":00");
        Self::new(buf)
    }

    /// Formats the datetime back to RFC 3339 local format.
    ///
    /// Returns a string in the format `YYYY-MM-DDTHH:MM:SS`, which is the
    /// same format expected by the [`new`](Self::new) method.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::str::FromStr;
    ///
    /// use cot::common_types::DateTime;
    ///
    /// let dt = DateTime::from_str("2025-05-27T13:03:00").unwrap();
    /// assert_eq!(dt.to_local_string(), "2025-05-27T13:03:00");
    /// ```
    #[must_use]
    pub fn to_local_string(&self) -> String {
        self.0.format("%Y-%m-%dT%H:%M:%S").to_string()
    }

    /// Returns a reference to the underlying `NaiveDateTime`.
    ///
    /// This provides access to the full chrono API for advanced date/time
    /// operations like arithmetic, comparisons, and custom formatting.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::str::FromStr;
    ///
    /// use chrono::Duration;
    /// use cot::common_types::DateTime;
    ///
    /// let dt = DateTime::from_str("2025-05-27T13:03:00").unwrap();
    /// let tomorrow = *dt.inner() + Duration::days(1);
    /// ```
    #[must_use]
    pub fn inner(&self) -> &NaiveDateTime {
        &self.0
    }
}

/// Implements string parsing for `DateTime`.
///
/// Uses the default RFC 3339 format (`YYYY-MM-DDTHH:MM:SS`).
///
/// # Examples
///
/// ```
/// use std::str::FromStr;
///
/// use cot::common_types::DateTime;
///
/// let dt = DateTime::from_str("2025-05-27T13:03:00").unwrap();
/// ```
impl FromStr for DateTime {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

/// Implements conversion from string references to `DateTime`.
///
/// # Examples
///
/// ```
/// use cot::common_types::DateTime;
///
/// let dt = DateTime::try_from("2025-05-27T13:03:00").unwrap();
/// ```
impl TryFrom<&str> for DateTime {
    type Error = ParseError;
    fn try_from(v: &str) -> Result<Self, Self::Error> {
        Self::new(v)
    }
}

/// Implements conversion from `String` to `DateTime`.
///
/// # Examples
///
/// ```
/// use cot::common_types::DateTime;
///
/// let dt = DateTime::try_from(String::from("2025-05-27T13:03:00")).unwrap();
/// ```
impl TryFrom<String> for DateTime {
    type Error = ParseError;
    fn try_from(v: String) -> Result<Self, Self::Error> {
        Self::new(&v)
    }
}

/// Implements display formatting for `DateTime`.
///
/// Uses the same format as [`to_local_string`](Self::to_local_string).
impl Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_local_string())
    }
}

/// Defines the database field type for `DateTime`.
///
/// DateTime values are stored using the database's native datetime type.
#[cfg(feature = "db")]
impl DatabaseField for DateTime {
    const TYPE: ColumnType = ColumnType::DateTime;
}

/// Implements database value conversion for `DateTime`.
///
/// This allows a `DateTime` to be stored in the database using the underlying
/// `NaiveDateTime` representation.
#[cfg(feature = "db")]
impl ToDbValue for DateTime {
    fn to_db_value(&self) -> DbValue {
        self.0.into()
    }
}

/// Implements database value conversion for retrieving `DateTime` from the
/// database.
///
/// This allows `DateTime` to be retrieved from the database and properly
/// converted from the stored `NaiveDateTime` value.
#[cfg(feature = "db")]
impl FromDbValue for DateTime {
    #[cfg(feature = "sqlite")]
    fn from_sqlite(value: SqliteValueRef<'_>) -> db::Result<Self>
    where
        Self: Sized,
    {
        let date_time = value.get::<NaiveDateTime>();
        date_time.map(Self).map_err(db::DatabaseError::value_decode)
    }

    #[cfg(feature = "postgres")]
    fn from_postgres(value: PostgresValueRef<'_>) -> db::Result<Self>
    where
        Self: Sized,
    {
        let date_time = value.get::<NaiveDateTime>();
        date_time.map(Self).map_err(db::DatabaseError::value_decode)
    }

    #[cfg(feature = "mysql")]
    fn from_mysql(value: MySqlValueRef<'_>) -> db::Result<Self>
    where
        Self: Sized,
    {
        let date_time = value.get::<NaiveDateTime>();
        date_time.map(Self).map_err(db::DatabaseError::value_decode)
    }
}

/// A validated time without date or timezone information.
///
/// This is a newtype wrapper around [`NaiveTime`](chrono::NaiveTime) that
/// provides consistent parsing and integration with Cot's database system.
/// It ensures time values are properly validated and formatted for use in
/// forms and database operations.
///
/// The type primarily expects time strings in 24-hour format `HH:MM:SS`,
/// but also provides convenience methods for HTML5 `time` input format
/// which omits seconds.
///
/// # Examples
///
/// ```
/// use std::str::FromStr;
///
/// use cot::common_types::Time;
///
/// // Parse from full time format
/// let time = Time::from_str("13:03:00").unwrap();
///
/// // Parse from HTML time input format (no seconds)
/// let time = Time::from_time_local("13:03").unwrap();
///
/// // Convert using TryFrom
/// let time = Time::try_from("13:03:00").unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct Time(NaiveTime);

impl Time {
    /// Creates a new `Time` from a string in 24-hour format.
    ///
    /// Expects the format `HH:MM:SS` where seconds are mandatory.
    /// Hours are in 24-hour format (00-23).
    ///
    /// # Errors
    ///
    /// Returns a [`ParseError`] if the input string doesn't match the expected
    /// format or contains invalid time values.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::common_types::Time;
    ///
    /// let time = Time::new("13:03:00").unwrap();
    /// let midnight = Time::new("00:00:00").unwrap();
    /// assert!(Time::new("invalid").is_err());
    /// assert!(Time::new("13:03").is_err()); // missing seconds
    /// ```
    pub fn new<T: AsRef<str>>(time: T) -> Result<Self, ParseError> {
        Self::with_format(time.as_ref(), "%H:%M:%S")
    }

    /// Creates a new `Time` from a string using a custom format.
    ///
    /// This method allows parsing time strings in formats other than the
    /// default `HH:MM:SS` format. The format string uses the same syntax as
    /// [`chrono::NaiveTime::parse_from_str`].
    ///
    /// # Errors
    ///
    /// Returns a [`ParseError`] if the input string doesn't match the specified
    /// format or contains invalid time values.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::common_types::Time;
    ///
    /// // Parse 12-hour format
    /// let time = Time::with_format("1:03:00 PM", "%I:%M:%S %p").unwrap();
    ///
    /// // Parse without seconds
    /// let time = Time::with_format("13:03", "%H:%M").unwrap();
    /// ```
    pub fn with_format<S: AsRef<str>>(s: S, fmt: &str) -> Result<Self, ParseError> {
        NaiveTime::parse_from_str(s.as_ref(), fmt).map(Self)
    }

    /// Creates a new `Time` from HTML5 `time` input format.
    ///
    /// This is a convenience method for parsing the format used by HTML5
    /// `<input type="time">` elements, which uses `HH:MM` (without seconds).
    /// The method automatically appends `:00` for seconds.
    ///
    /// # Errors
    ///
    /// Returns a [`ParseError`] if the input string doesn't match the expected
    /// `time` format or contains invalid time values.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::common_types::Time;
    ///
    /// let time = Time::from_time_local("13:03").unwrap();
    /// assert_eq!(time.to_local_string(), "13:03:00");
    /// ```
    pub fn from_time_local<T: AsRef<str>>(time: T) -> Result<Self, ParseError> {
        let mut buf = time.as_ref().to_string();
        buf.push_str(":00");
        Self::new(buf)
    }

    /// Formats the time back to 24-hour format.
    ///
    /// Returns a string in the format `HH:MM:SS`, which is the same format
    /// expected by the [`new`](Self::new) method.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::str::FromStr;
    ///
    /// use cot::common_types::Time;
    ///
    /// let time = Time::from_str("13:03:00").unwrap();
    /// assert_eq!(time.to_local_string(), "13:03:00");
    /// ```
    #[must_use]
    pub fn to_local_string(&self) -> String {
        self.0.format("%H:%M:%S").to_string()
    }

    /// Returns a reference to the underlying `NaiveTime`.
    ///
    /// This provides access to the full chrono API for advanced time
    /// operations like arithmetic, comparisons, and custom formatting.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::str::FromStr;
    ///
    /// use chrono::Duration;
    /// use cot::common_types::Time;
    ///
    /// let time = Time::from_str("13:03:00").unwrap();
    /// let later = *time.inner() + Duration::hours(2);
    /// ```
    #[must_use]
    pub fn inner(&self) -> &NaiveTime {
        &self.0
    }
}

/// Implements string parsing for `Time`.
///
/// Uses the default 24-hour format (`HH:MM:SS`).
///
/// # Examples
///
/// ```
/// use std::str::FromStr;
///
/// use cot::common_types::Time;
///
/// let time = Time::from_str("13:03:00").unwrap();
/// ```
impl FromStr for Time {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

/// Implements conversion from string references to `Time`.
///
/// # Examples
///
/// ```
/// use cot::common_types::Time;
///
/// let time = Time::try_from("13:03:00").unwrap();
/// ```
impl TryFrom<&str> for Time {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// Implements conversion from `String` to `Time`.
///
/// # Examples
///
/// ```
/// use cot::common_types::Time;
///
/// let time = Time::try_from(String::from("13:03:00")).unwrap();
/// ```
impl TryFrom<String> for Time {
    type Error = ParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// Implements display formatting for `Time`.
///
/// Uses the same format as [`to_local_string`](Self::to_local_string).
impl Display for Time {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_local_string())
    }
}

/// Implements database value conversion for retrieving `Time` from the
/// database.
///
/// This allows `Time` to be retrieved from the database and properly
/// converted from the stored `NaiveTime` value.
impl FromDbValue for Time {
    #[cfg(feature = "sqlite")]
    fn from_sqlite(value: SqliteValueRef<'_>) -> db::Result<Self>
    where
        Self: Sized,
    {
        let time = value.get::<NaiveTime>();
        time.map(Self).map_err(db::DatabaseError::value_decode)
    }

    #[cfg(feature = "postgres")]
    fn from_postgres(value: PostgresValueRef<'_>) -> db::Result<Self>
    where
        Self: Sized,
    {
        let time = value.get::<NaiveTime>();
        time.map(Self).map_err(db::DatabaseError::value_decode)
    }

    #[cfg(feature = "mysql")]
    fn from_mysql(value: MySqlValueRef<'_>) -> db::Result<Self>
    where
        Self: Sized,
    {
        let time = value.get::<NaiveTime>();
        time.map(Self).map_err(db::DatabaseError::value_decode)
    }
}

/// Implements database field value conversion for `Time`.
///
/// This allows a `Time` to be stored in the database using the underlying
/// `NaiveTime` representation.
impl ToDbFieldValue for Time {
    fn to_db_field_value(&self) -> DbFieldValue {
        self.0.into()
    }
}

/// Defines the database field type for `Time`.
///
/// Time values are stored using the database's native time type.
#[cfg(feature = "db")]
impl DatabaseField for Time {
    const TYPE: ColumnType = ColumnType::Time;
}

/// A validated date without time or timezone information.
///
/// This is a newtype wrapper around [`NaiveDate`](chrono::NaiveDate) that
/// provides consistent parsing and integration with Cot's database system.
/// It ensures date values are properly validated and formatted for use in
/// forms and database operations.
///
/// The type expects date strings in ISO 8601 format `YYYY-MM-DD`, which is
/// also the standard format used by HTML5 `date` input elements.
///
/// # Examples
///
/// ```
/// use std::str::FromStr;
///
/// use cot::common_types::Date;
///
/// // Parse from ISO 8601 format
/// let date = Date::from_str("2025-05-27").unwrap();
///
/// // Parse with custom format
/// let date = Date::with_format("05/27/2025", "%m/%d/%Y").unwrap();
///
/// // Convert using TryFrom
/// let date = Date::try_from("2025-05-27").unwrap();
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct Date(NaiveDate);

impl Date {
    /// Creates a new `Date` from a string in ISO 8601 format.
    ///
    /// Expects the format `YYYY-MM-DD` where year is 4 digits, month and day
    /// are 2 digits with leading zeros if necessary. This format is compatible
    /// with HTML5 `<input type="date">` elements.
    ///
    /// # Errors
    ///
    /// Returns a [`ParseError`] if the input string doesn't match the expected
    /// format or contains invalid date values.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::common_types::Date;
    ///
    /// let date = Date::new("2025-05-27").unwrap();
    /// let new_year = Date::new("2025-01-01").unwrap();
    /// assert!(Date::new("invalid").is_err());
    /// assert!(Date::new("05/27/2025").is_err()); // wrong format
    /// ```
    pub fn new<D: AsRef<str>>(date: D) -> Result<Self, ParseError> {
        Self::with_format(date.as_ref(), "%Y-%m-%d")
    }

    /// Creates a new `Date` from a string using a custom format.
    ///
    /// This method allows parsing date strings in formats other than the
    /// default ISO 8601 format. The format string uses the same syntax as
    /// [`chrono::NaiveDate::parse_from_str`].
    ///
    /// # Errors
    ///
    /// Returns a [`ParseError`] if the input string doesn't match the specified
    /// format or contains invalid date values.
    ///
    /// # Examples
    ///
    /// ```
    /// use cot::common_types::Date;
    ///
    /// // Parse US format
    /// let date = Date::with_format("05/27/2025", "%m/%d/%Y").unwrap();
    ///
    /// // Parse European format
    /// let date = Date::with_format("27.05.2025", "%d.%m.%Y").unwrap();
    ///
    /// // Parse with month names
    /// let date = Date::with_format("May 27, 2025", "%B %d, %Y").unwrap();
    /// ```
    pub fn with_format<D: AsRef<str>>(date: D, fmt: &str) -> Result<Self, ParseError> {
        NaiveDate::parse_from_str(date.as_ref(), fmt).map(Self)
    }

    /// Formats the date back to ISO 8601 format.
    ///
    /// Returns a string in the format `YYYY-MM-DD`, which is the same format
    /// expected by the [`new`](Self::new) method and compatible with HTML5
    /// date inputs.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::str::FromStr;
    ///
    /// use cot::common_types::Date;
    ///
    /// let date = Date::from_str("2025-05-27").unwrap();
    /// assert_eq!(date.to_local_string(), "2025-05-27");
    /// ```
    #[must_use]
    pub fn to_local_string(&self) -> String {
        self.0.format("%Y-%m-%d").to_string()
    }

    /// Returns a reference to the underlying `NaiveDate`.
    ///
    /// This provides access to the full chrono API for advanced date
    /// operations like arithmetic, comparisons, weekday calculations,
    /// and custom formatting.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::str::FromStr;
    ///
    /// use chrono::{Duration, Weekday};
    /// use cot::common_types::Date;
    ///
    /// let date = Date::from_str("2025-05-27").unwrap();
    /// let next_week = *date.inner() + Duration::weeks(1);
    /// let weekday = date.inner().weekday();
    /// ```
    #[must_use]
    pub fn inner(&self) -> &NaiveDate {
        &self.0
    }
}

/// Implements string parsing for `Date`.
///
/// Uses the default ISO 8601 format (`YYYY-MM-DD`).
///
/// # Examples
///
/// ```
/// use std::str::FromStr;
///
/// use cot::common_types::Date;
///
/// let date = Date::from_str("2025-05-27").unwrap();
/// ```
impl FromStr for Date {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

/// Implements conversion from string references to `Date`.
///
/// # Examples
///
/// ```
/// use cot::common_types::Date;
///
/// let date = Date::try_from("2025-05-27").unwrap();
/// ```
impl TryFrom<&str> for Date {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// Implements conversion from `String` to `Date`.
///
/// # Examples
///
/// ```
/// use cot::common_types::Date;
///
/// let date = Date::try_from(String::from("2025-05-27")).unwrap();
/// ```
impl TryFrom<String> for Date {
    type Error = ParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// Implements display formatting for `Date`.
///
/// Uses the same format as [`to_local_string`](Self::to_local_string).
impl Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_local_string())
    }
}

/// Implements database value conversion for `Date`.
///
/// This allows a `Date` to be stored in the database using the underlying
/// `NaiveDate` representation.
#[cfg(feature = "db")]
impl ToDbValue for Date {
    fn to_db_value(&self) -> DbValue {
        self.0.into()
    }
}

/// Implements database value conversion for retrieving `Date` from the
/// database.
///
/// This allows `Date` to be retrieved from the database and properly
/// converted from the stored `NaiveDate` value.
#[cfg(feature = "db")]
impl FromDbValue for Date {
    #[cfg(feature = "sqlite")]
    fn from_sqlite(value: SqliteValueRef<'_>) -> db::Result<Self>
    where
        Self: Sized,
    {
        let date = value.get::<NaiveDate>();
        date.map(Self).map_err(db::DatabaseError::value_decode)
    }

    #[cfg(feature = "postgres")]
    fn from_postgres(value: PostgresValueRef<'_>) -> db::Result<Self>
    where
        Self: Sized,
    {
        let date = value.get::<NaiveDate>();
        date.map(Self).map_err(db::DatabaseError::value_decode)
    }

    #[cfg(feature = "mysql")]
    fn from_mysql(value: MySqlValueRef<'_>) -> db::Result<Self>
    where
        Self: Sized,
    {
        let date = value.get::<NaiveDate>();
        date.map(Self).map_err(db::DatabaseError::value_decode)
    }
}

/// Defines the database field type for `Date`.
///
/// Date values are stored using the database's native date type.
#[cfg(feature = "db")]
impl DatabaseField for Date {
    const TYPE: ColumnType = ColumnType::Date;
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;

    use super::*;

    // ------------------------
    // Password tests
    // ------------------------
    #[test]
    fn password_debug() {
        let password = Password::new("password");
        assert_eq!(format!("{password:?}"), "Password(\"**********\")");
    }

    #[test]
    fn password_str() {
        let password = Password::new("password");
        assert_eq!(password.as_str(), "password");
        assert_eq!(password.into_string(), "password");
    }
    // ------------------------
    // Email tests
    // ------------------------
    #[test]
    fn test_valid_email_creation() {
        let email = Email::new("user@example.com").unwrap();
        assert_eq!(email.as_str(), "user@example.com");
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

    // ------------------------
    // DateTime tests
    // ------------------------
    #[test]
    fn datetime_new_valid() {
        let s = "2025-05-27T13:03:00";
        let dt = DateTime::new(s).unwrap();
        assert_eq!(dt.to_local_string(), s);
    }

    #[test]
    fn datetime_new_invalid_format() {
        assert!(DateTime::new("invalid").is_err());
        assert!(DateTime::new("2025-05-27T13:03").is_err()); // missing seconds
    }

    #[test]
    fn datetime_with_format_custom() {
        let s = "05/27/2025 01:03:00 PM";
        let dt = DateTime::with_format(s, "%m/%d/%Y %I:%M:%S %p").unwrap();
        assert_eq!(dt.to_local_string(), "2025-05-27T13:03:00");
    }

    #[test]
    fn datetime_from_datetime_local_appends_seconds() {
        let s_local = "2025-05-27T13:03";
        let dt = DateTime::from_datetime_local(s_local).unwrap();
        assert_eq!(dt.to_local_string(), "2025-05-27T13:03:00");
    }

    #[test]
    fn datetime_from_str_and_try_from() {
        let s = "2025-05-27T13:03:00";
        let dt1: DateTime = s.parse().unwrap();
        let dt2 = DateTime::try_from(s).unwrap();
        assert_eq!(dt1, dt2);
    }

    // ------------------------
    // Time tests
    // ------------------------
    #[test]
    fn time_new_valid() {
        let s = "13:03:00";
        let t = Time::new(s).unwrap();
        assert_eq!(t.to_local_string(), s);
    }

    #[test]
    fn time_new_invalid() {
        assert!(Time::new("invalid").is_err());
        assert!(Time::new("13:03").is_err()); // missing seconds
    }

    #[test]
    fn time_with_format_custom() {
        let s = "1:03:00 PM";
        let t = Time::with_format(s, "%I:%M:%S %p").unwrap();
        assert_eq!(t.to_local_string(), "13:03:00");
    }

    #[test]
    fn time_from_time_local() {
        let s_local = "13:03";
        let t = Time::from_time_local(s_local).unwrap();
        assert_eq!(t.to_local_string(), "13:03:00");
    }

    #[test]
    fn time_from_str_and_try_from() {
        let s = "13:03:00";
        let t1: Time = s.parse().unwrap();
        let t2 = Time::try_from(s).unwrap();
        assert_eq!(t1, t2);
    }

    // ------------------------
    // Date tests
    // ------------------------
    #[test]
    fn date_new_valid() {
        let s = "2025-05-27";
        let d = Date::new(s).unwrap();
        assert_eq!(d.to_local_string(), s);
    }

    #[test]
    fn date_new_invalid() {
        assert!(Date::new("invalid").is_err());
        assert!(Date::new("05/27/2025").is_err()); // wrong format
    }

    #[test]
    fn date_with_format_custom() {
        let s = "05/27/2025";
        let d = Date::with_format(s, "%m/%d/%Y").unwrap();
        assert_eq!(d.to_local_string(), "2025-05-27");
    }

    #[test]
    fn date_from_str_and_try_from() {
        let s = "2025-05-27";
        let d1: Date = s.parse().unwrap();
        let d2 = Date::try_from(s).unwrap();
        assert_eq!(d1, d2);
    }

    #[cfg(feature = "db")]
    mod db_tests {
        use super::*;
        use crate::db::ToDbValue;
        // ------------------------
        // Email tests
        // ------------------------
        #[test]
        fn test_to_db_value() {
            let email = Email::new("user@example.com").unwrap();
            let db_value = email.to_db_value();

            let email_str = email.as_str();
            let db_value_str = format!("{db_value:?}");
            assert!(db_value_str.contains(email_str));
        }

        #[test]
        fn test_to_db_value_is_normalized() {
            let with_display = Email::new("John Doe <user@example.com>").unwrap();
            let bare = Email::new("user@example.com").unwrap();

            let db1 = with_display.to_db_value();
            let db2 = bare.to_db_value();

            assert_eq!(db1, db2);
        }

        // ------------------------
        // DateTime tests
        // ------------------------
        #[test]
        fn datetime_to_db_value_contains_str() {
            let dt = DateTime::new("2025-05-27T13:03:00").unwrap();
            let dbv = dt.to_db_value();
            let s = format!("{:?}", dbv);
            assert!(s.contains("2025-05-27T13:03:00"));
        }

        // ------------------------
        // Time tests
        // ------------------------
        #[test]
        fn time_to_db_field_value_contains_str() {
            let t = Time::new("13:03:00").unwrap();
            let dfv = t.to_db_field_value();
            let s = format!("{:?}", dfv);
            assert!(s.contains("13:03:00"));
        }

        // ------------------------
        // Date tests
        // ------------------------
        #[test]
        fn date_to_db_value_contains_str() {
            let d = Date::new("2025-05-27").unwrap();
            let dbv = d.to_db_value();
            let s = format!("{:?}", dbv);
            assert!(s.contains("2025-05-27"));
        }
    }
}
