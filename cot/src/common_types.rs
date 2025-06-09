//! Form Field Types for Cot
//!
//! This module provides a collection of form field types and utilities for
//! validating, parsing, and converting user input within Cot. It includes
//! general-purpose newtype wrappers and associated trait implementations to
//! ensure consistent and safe processing of form data.

use std::fmt::Debug;
use std::str::FromStr;

#[cfg(feature = "mysql")]
use cot::db::impl_mysql::MySqlValueRef;
#[cfg(feature = "postgres")]
use cot::db::impl_postgres::PostgresValueRef;
#[cfg(feature = "sqlite")]
use cot::db::impl_sqlite::SqliteValueRef;
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
/// the hash, use [`PasswordHash`](crate::auth::PasswordHash).
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
/// 1. The most secure approach is to use
///    [`PasswordHash::from_password`](crate::auth::PasswordHash::from_password)
///    to create a hash from one password, and then use
///    [`PasswordHash::verify`](crate::auth::PasswordHash::verify) to compare it
///    with the other password. This method uses constant-time equality
///    comparison, which protects against timing attacks.
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
#[cfg(feature = "db")]
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

/// Defines the database field type for `Email`.
///
/// Emails are stored as strings with a maximum length of 254 characters,
/// as specified in RFC 5321.
#[cfg(feature = "db")]
impl DatabaseField for Email {
    const TYPE: ColumnType = ColumnType::String(MAX_EMAIL_LENGTH);
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;

    use super::*;

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

    #[cfg(feature = "db")]
    mod db_tests {
        use super::*;
        use crate::db::ToDbValue;

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
    }
}
