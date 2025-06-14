//! Email sending functionality using SMTP and other backends
//!
//! #Examples
//! To send an email using the `EmailBackend`, you need to create an instance of
//! `SmtpConfig`
//! ```
//! use cot::email::{EmailBackend, EmailMessage, SmtpConfig, SmtpEmailBackend};
//! fn test_send_email_localhsot() {
//!     // Create a test email
//!     let email = EmailMessage {
//!         subject: "Test Email".to_string(),
//!         from: String::from("<from@cotexample.com>").into(),
//!         to: vec!["<to@cotexample.com>".to_string()],
//!         body: "This is a test email sent from Rust.".to_string(),
//!         alternative_html: Some(
//!             "<p>This is a test email sent from Rust as HTML.</p>".to_string(),
//!         ),
//!         ..Default::default()
//!     };
//!     let config = SmtpConfig::default();
//!     // Create a new email backend
//!     let mut backend = SmtpEmailBackend::new(config);
//!     let _ = backend.send_message(&email);
//! }
//! ```
use std::time::Duration;

use derive_builder::Builder;
use lettre::message::{Mailbox, Message, MultiPart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{SmtpTransport, Transport};
#[cfg(test)]
use mockall::{automock, predicate::*};
use serde::{Deserialize, Serialize};

/// Represents errors that can occur when sending an email.
#[derive(Debug, thiserror::Error)]
pub enum EmailError {
    /// An error occurred while building the email message.
    #[error("Message error: {0}")]
    MessageError(String),
    /// The email configuration is invalid.
    #[error("Invalid email configuration: {0}")]
    ConfigurationError(String),
    /// An error occurred while sending the email.
    #[error("Send error: {0}")]
    SendError(String),
    /// An error occurred while connecting to the SMTP server.
    #[error("Connection error: {0}")]
    ConnectionError(String),
}

type Result<T> = std::result::Result<T, EmailError>;

/// Represents the mode of SMTP transport to initialize the backend with.
#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SmtpTransportMode {
    /// No SMTP transport.
    None,
    /// Use the default SMTP transport for localhost.
    #[default]
    Localhost,
    /// Use an unencrypted SMTP connection to the specified host.
    Unencrypted(String),
    /// Use a relay SMTP connection to the specified host.
    Relay(String),
    /// Use a STARTTLS relay SMTP connection to the specified host.
    StartTlsRelay(String),
}

/// Represents the state of a transport mechanism for SMTP communication.
///
/// The `TransportState` enum is used to define whether the transport is
/// uninitialized (default state) or initialized with specific settings.
///
/// # Examples
///
/// ```
/// use cot::email::TransportState;
///
/// let state = TransportState::Uninitialized; // Default state
/// match state {
///     TransportState::Uninitialized => println!("Transport is not initialized."),
///     TransportState::Initialized => println!("Transport is initialized."),
/// }
/// ```
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportState {
    /// Use the default SMTP transport for localhost.
    #[default]
    Uninitialized,
    /// Use an unencrypted SMTP connection to the specified host.
    Initialized,
}
/// Represents an email address with an optional name.
#[derive(Debug, Clone, Default)]
pub struct EmailAddress {
    /// The email address.
    pub address: String,
    /// The optional name associated with the email address.
    pub name: Option<String>,
}
/// Holds the contents of the email prior to converting to
/// a lettre Message.
#[derive(Debug, Clone, Default)]
pub struct EmailMessage {
    /// The subject of the email.
    pub subject: String,
    /// The body of the email.
    pub body: String,
    /// The email address of the sender.
    pub from: EmailAddress,
    /// The list of recipient email addresses.
    pub to: Vec<String>,
    /// The list of CC (carbon copy) recipient email addresses.
    pub cc: Option<Vec<String>>,
    /// The list of BCC (blind carbon copy) recipient email addresses.
    pub bcc: Option<Vec<String>>,
    /// The list of reply-to email addresses.
    pub reply_to: Option<Vec<String>>,
    /// The alternative parts of the email (e.g., plain text and HTML versions).
    pub alternative_html: Option<String>, // (content, mimetype)
}

/// Configuration for SMTP email backend
#[derive(Debug, Builder, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmtpConfig {
    /// The SMTP server host address.
    /// Defaults to "localhost".
    pub mode: SmtpTransportMode,
    /// The SMTP server port.
    /// Overwrites the default standard port when specified.
    pub port: Option<u16>,
    /// The username for SMTP authentication.
    pub username: Option<String>,
    /// The password for SMTP authentication.
    pub password: Option<String>,
    /// The timeout duration for the SMTP connection.
    pub timeout: Option<Duration>,
}

/// SMTP Backend for sending emails
//#[allow(missing_debug_implementations)]
#[derive(Debug)]
pub struct SmtpEmailBackend {
    /// The SMTP configuration.
    config: SmtpConfig,
    /// The SMTP transport.
    /// This field is optional because the transport may not be initialized yet.
    /// It will be initialized when the `open` method is called.
    transport: Option<Box<dyn EmailTransport>>,
    /// Whether or not to print debug information.
    debug: bool,
    transport_state: TransportState,
}
impl std::fmt::Debug for dyn EmailTransport + 'static {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmailTransport").finish()
    }
}
/// Default implementation for `SmtpConfig`.
/// This provides default values for the SMTP configuration fields.
/// The default mode is `Localhost`, with no port, username, or password.
/// The default timeout is set to 60 seconds.
/// This allows for easy creation of a default SMTP configuration
/// without needing to specify all the fields explicitly.
impl Default for SmtpConfig {
    fn default() -> Self {
        Self {
            mode: SmtpTransportMode::None,
            port: None,
            username: None,
            password: None,
            timeout: Some(Duration::from_secs(60)),
        }
    }
}

impl SmtpConfig {
    /// Create a new instance of the SMTP configuration with the given mode.
    #[must_use]
    pub fn new(mode: SmtpTransportMode) -> Self {
        Self {
            mode,
            ..Default::default()
        }
    }
    fn validate(&self) -> Result<&Self> {
        // Check if username and password are both provided both must be Some or both
        // None
        if self.username.is_some() && self.password.is_none()
            || self.username.is_none() && self.password.is_some()
        {
            return Err(EmailError::ConfigurationError(
                "Both username and password must be provided for SMTP authentication".to_string(),
            ));
        }
        let host = match &self.mode {
            SmtpTransportMode::Unencrypted(host) => host,
            SmtpTransportMode::Relay(host_relay) => host_relay,
            SmtpTransportMode::StartTlsRelay(host_tls) => host_tls,
            SmtpTransportMode::Localhost => &"localhost".to_string(),
            SmtpTransportMode::None => &String::new(),
        };
        if host.is_empty() && self.mode != SmtpTransportMode::None {
            return Err(EmailError::ConfigurationError(
                "Host cannot be empty or blank".to_string(),
            ));
        }
        Ok(self)
    }
}
/// Convert ``AddressError`` to ``EmailError`` using ``From`` trait
impl From<lettre::address::AddressError> for EmailError {
    fn from(error: lettre::address::AddressError) -> Self {
        EmailError::MessageError(format!("Invalid email address: {error}"))
    }
}
/// Convert ``EmailAddress`` to ``Mailbox`` using ``TryFrom`` trait
impl TryFrom<&EmailAddress> for Mailbox {
    type Error = EmailError;

    fn try_from(email: &EmailAddress) -> Result<Self> {
        if email.address.is_empty() {
            return Err(EmailError::ConfigurationError(
                "Email address cannot be empty".to_string(),
            ));
        }

        if email.name.is_none() {
            Ok(format!("<{}>", email.address).parse()?)
        } else {
            Ok(format!("\"{}\" <{}>", email.name.as_ref().unwrap(), email.address).parse()?)
        }
    }
}
/// Convert ``String`` to ``EmailAddress`` using ``From`` trait
impl From<String> for EmailAddress {
    fn from(address: String) -> Self {
        Self {
            address,
            name: None,
        }
    }
}
/// Convert ``SmtpConfig`` to Credentials using ``TryFrom`` trait
impl TryFrom<&SmtpConfig> for Credentials {
    type Error = EmailError;

    fn try_from(config: &SmtpConfig) -> Result<Self> {
        match (&config.username, &config.password) {
            (Some(username), Some(password)) => {
                Ok(Credentials::new(username.clone(), password.clone()))
            }
            (Some(_), None) | (None, Some(_)) => Err(EmailError::ConfigurationError(
                "Both username and password must be provided for SMTP authentication".to_string(),
            )),
            (None, None) => Ok(Credentials::new(String::new(), String::new())),
        }
    }
}
/// Convert ``EmailMessage`` to ``Message`` using ``TryFrom`` trait
impl TryFrom<&EmailMessage> for Message {
    type Error = EmailError;

    fn try_from(email: &EmailMessage) -> Result<Self> {
        // Create a simple email for testing
        let mut builder = Message::builder()
            .subject(email.subject.clone())
            .from(Mailbox::try_from(&email.from)?);

        // Add recipients
        for to in &email.to {
            builder = builder.to(to.parse()?);
        }
        if let Some(cc) = &email.cc {
            for c in cc {
                builder = builder.cc(c.parse()?);
            }
        }

        // Add BCC recipients if present
        if let Some(bcc) = &email.bcc {
            for bc in bcc {
                builder = builder.cc(bc.parse()?);
            }
        }

        // Add reply-to if present
        if let Some(reply_to) = &email.reply_to {
            for r in reply_to {
                builder = builder.reply_to(r.parse()?);
            }
        }
        if email.alternative_html.is_some() {
            builder
                .multipart(MultiPart::alternative_plain_html(
                    String::from(email.body.clone()),
                    String::from(email.alternative_html.clone().unwrap()),
                ))
                .map_err(|e| {
                    EmailError::MessageError(format!("Failed to create email message: {e}"))
                })
        } else {
            builder
                .body(email.body.clone())
                .map_err(|e| EmailError::MessageError(format!("Failed email body:{e}")))
        }
    }
}
/// Trait for sending emails using SMTP transport
/// This trait provides methods for testing connection,
/// sending a single email, and building the transport.
/// It is implemented for `SmtpTransport`.
/// This trait is useful for abstracting the email sending functionality
/// and allows for easier testing and mocking.
/// It can be used in applications that need to send emails
/// using SMTP protocol.
/// #Errors
/// `EmailError::ConnectionError` if there is an issue with the SMTP connection.
/// `EmailError::SendError` if there is an issue with sending the email.
/// `EmailError::ConfigurationError` if the SMTP configuration is invalid.
#[cfg_attr(test, automock)]
pub trait EmailTransport: Send + Sync {
    /// Test the connection to the SMTP server.
    /// # Errors
    /// Returns Ok(true) if the connection is successful, otherwise
    /// ``EmailError::ConnectionError``.
    fn test_connection(&self) -> Result<bool>;

    /// Send an email message.
    /// # Errors
    /// Returns Ok(true) if the connection is successful, otherwise
    /// ``EmailError::ConnectionError or SendError``.
    fn send_email(&self, email: &Message) -> Result<()>;
}

impl EmailTransport for SmtpTransport {
    fn test_connection(&self) -> Result<bool> {
        Ok(self.test_connection().is_ok())
    }

    fn send_email(&self, email: &Message) -> Result<()> {
        // Call the actual Transport::send method
        match self.send(email) {
            Ok(_) => Ok(()),
            Err(e) => Err(EmailError::SendError(e.to_string())),
        }
    }
}

/// Trait representing an email backend for sending emails.
pub trait EmailBackend: Send + Sync + 'static {
    /// Creates a new instance of the email backend with the given
    /// configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The SMTP configuration to use.
    fn new(config: SmtpConfig) -> Self;

    /// Initialize the backend for any specialization for any backend such as
    /// `FileTransport` ``SmtpTransport``
    ///
    /// # Errors
    ///
    /// `EmailError::ConfigurationError`:
    /// If the SMTP configuration is invalid (e.g., missing required fields like
    /// username and password).
    /// If the host is empty or blank in the configuration.
    /// If the credentials cannot be created from the configuration.
    ///
    /// `EmailError::ConnectionError`:
    /// If the transport cannot be created for the specified mode (e.g.,
    /// invalid host or unsupported configuration).
    /// If the transport fails to connect to the SMTP server.
    fn init(&mut self) -> Result<()>;

    /// Open a connection to the SMTP server.
    ///
    /// # Errors
    ///
    /// This function will return an `EmailError` if there is an issue with
    /// resolving the SMTP host,
    /// creating the TLS parameters, or connecting to the SMTP server.
    fn open(&mut self) -> Result<&Self>;
    /// Close the connection to the SMTP server.
    ///
    /// # Errors
    ///
    /// This function will return an `EmailError` if there is an issue with
    /// closing the SMTP connection.
    ///
    /// # Errors
    ///
    /// This function will return an `EmailError` if there is an issue with
    /// closing the SMTP connection.
    fn close(&mut self) -> Result<()>;

    /// Send a single email message
    ///
    /// # Errors
    ///
    /// This function will return an `EmailError` if there is an issue with
    /// opening the SMTP connection,
    /// building the email message, or sending the email.
    fn send_message(&mut self, message: &EmailMessage) -> Result<()>;

    /// Send multiple email messages
    ///
    /// # Errors
    ///
    /// This function will return an `EmailError` if there is an issue with
    /// sending any of the emails.
    fn send_messages(&mut self, emails: &[EmailMessage]) -> Result<usize> {
        let mut sent_count = 0;

        for email in emails {
            match self.send_message(email) {
                Ok(()) => sent_count += 1,
                Err(e) => return Err(e),
            }
        }

        Ok(sent_count)
    }
}

impl EmailBackend for SmtpEmailBackend {
    /// Creates a new instance of `EmailBackend` with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The SMTP configuration to use.
    fn new(config: SmtpConfig) -> Self {
        Self {
            config,
            transport: None,
            debug: false,
            transport_state: TransportState::Uninitialized,
        }
    }

    /// Safely initializes the SMTP transport based on the configured mode.
    ///
    /// This function validates the SMTP configuration and creates the
    /// appropriate transport based on the mode (e.g., Localhost,
    /// Unencrypted, Relay, or ``StartTlsRelay``).
    /// It also sets the timeout, port, and credentials if provided.
    ///
    /// # Errors
    ///
    /// `EmailError::ConfigurationError`:
    ///  If the SMTP configuration is invalid (e.g., missing required fields
    ///  like username and password).
    ///  If the host is empty or blank in the configuration.
    ///  If the credentials cannot be created from the configuration.
    ///
    /// `EmailError::ConnectionError`:
    ///  If the transport cannot be created for the specified mode (e.g.,
    ///  invalid host or unsupported configuration).
    ///  If the transport fails to connect to the SMTP server.
    fn init(&mut self) -> Result<()> {
        if self.transport_state == TransportState::Initialized {
            return Ok(());
        }
        self.config.validate().map_err(|e| {
            EmailError::ConfigurationError(format!(
                "Failed to validate SMTP configuration,error: {e}"
            ))
        })?;
        let mut transport_builder = match &self.config.mode {
            SmtpTransportMode::None => {
                return Err(EmailError::ConfigurationError(
                    "SMTP transport mode is not specified".to_string(),
                ));
            }
            SmtpTransportMode::Localhost => SmtpTransport::relay("localhost").map_err(|e| {
                EmailError::ConnectionError(format!(
                    "Failed to create SMTP localhost transport,error: {e}"
                ))
            })?,
            SmtpTransportMode::Unencrypted(host) => SmtpTransport::builder_dangerous(host),
            SmtpTransportMode::Relay(host) => SmtpTransport::relay(host).map_err(|e| {
                EmailError::ConnectionError(format!(
                    "Failed to create SMTP relay transport host:{host},error: {e}"
                ))
            })?,
            SmtpTransportMode::StartTlsRelay(host) => {
                SmtpTransport::starttls_relay(host).map_err(|e| {
                    EmailError::ConnectionError(format!(
                        "Failed to create SMTP tls_relay transport host:{host},error: {e}"
                    ))
                })?
            }
        };
        // Set the timeout for the transport
        transport_builder = transport_builder.timeout(self.config.timeout);

        // Set the port if provided in the configuration
        // The port is optional, so we check if it's Some before setting it
        // If the port is None, the default port for the transport will be used
        if self.config.port.is_some() {
            transport_builder = transport_builder.port(self.config.port.unwrap());
        }

        // Create the credentials using the provided configuration
        let credentials = Credentials::try_from(&self.config).map_err(|e| {
            EmailError::ConfigurationError(format!("Failed to create SMTP credentials,error: {e}"))
        })?;

        // Add authentication if credentials provided
        let transport = if self.config.username.is_some() && self.config.password.is_some() {
            transport_builder.credentials(credentials).build()
        } else {
            transport_builder.build()
        };
        self.transport = Some(Box::new(transport));
        self.transport_state = TransportState::Initialized;
        Ok(())
    }
    /// Opens a connection to the SMTP server or return the active connection.
    ///
    /// This method ensures that the SMTP transport is properly initialized and
    /// tests the connection to the SMTP server. If the transport is already
    /// initialized and the connection is working, it will reuse the existing
    /// transport. Otherwise, it will initialize a new transport and test the
    /// connection.
    ///
    /// # Errors
    ///
    /// This function can return the following errors:
    ///
    /// `EmailError::ConfigurationError`:
    ///  If the SMTP configuration is invalid (e.g., missing required fields
    ///  like username and password).
    ///  If the host is empty or blank in the configuration.
    ///  If the credentials cannot be created from the configuration.
    ///
    /// `EmailError::ConnectionError`:
    ///   If the transport cannot be created for the specified mode (e.g.,
    ///   invalid host or unsupported configuration).
    ///   If the transport fails to connect to the SMTP server.
    fn open(&mut self) -> Result<&Self> {
        // Test if self.transport is None or if the connection is not working
        if self.transport.is_some() && self.transport.as_ref().unwrap().test_connection().is_ok() {
            return Ok(self);
        }
        // Initialize the transport
        self.init()?;
        // Test connection to the SMTP server
        if self.transport.as_ref().unwrap().test_connection().is_err() {
            return Err(EmailError::ConnectionError(
                "Failed to connect to SMTP server".to_string(),
            ));
        }
        Ok(self)
    }

    /// Close the connection to the SMTP server
    ///
    /// # Errors
    ///
    /// This function will return an `EmailError` if there is an issue with
    /// closing the SMTP connection.
    ///
    /// # Errors
    ///
    /// This function will return an `EmailError` if there is an issue with
    /// closing the SMTP connection.
    fn close(&mut self) -> Result<()> {
        self.transport = None;
        self.transport_state = TransportState::Uninitialized;
        Ok(())
    }

    /// Send a single email message
    ///
    /// # Errors
    ///
    /// This function will return an `EmailError` if there is an issue with
    /// opening the SMTP connection,
    /// building the email message, or sending the email.
    fn send_message(&mut self, email: &EmailMessage) -> Result<()> {
        self.open()?;
        if self.debug {
            println!("Dump email: {email:#?}");
        }
        // Send the email
        self.transport
            .as_ref()
            .ok_or(EmailError::ConnectionError(
                "SMTP transport is not initialized".to_string(),
            ))?
            .send_email(&email.try_into()?)
            .map_err(|e| EmailError::SendError(e.to_string()))?;

        Ok(())
    }
}
impl SmtpEmailBackend {
    /// Creates a new instance of `SmtpEmailBackend` from the given
    /// configuration and transport.
    ///
    /// # Arguments
    ///
    /// * `config` - The SMTP configuration to use.
    /// * `transport` - An optional transport to use for sending emails.
    ///
    /// # Returns
    ///
    /// A new instance of `SmtpEmailBackend`.
    #[allow(clippy::must_use_candidate)]
    pub fn from_config(config: SmtpConfig, transport: Box<dyn EmailTransport>) -> Self {
        Self {
            config,
            transport: Some(transport),
            debug: false,
            transport_state: TransportState::Uninitialized,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults_values() {
        let config = SmtpConfig::default();

        assert_eq!(config.mode, SmtpTransportMode::None);
        assert_eq!(config.port, None);
        assert_eq!(config.username, None);
        assert_eq!(config.password, None);
        assert_eq!(config.timeout, Some(Duration::from_secs(60)));
    }

    #[test]
    fn test_config_default_ok() {
        let config = SmtpConfig::default();
        let result = config.validate();
        assert!(result.is_ok());
    }
    #[test]
    fn test_config_unencrypted_localhost_ok() {
        let config = SmtpConfig::new(SmtpTransportMode::Unencrypted("localhost".to_string()));
        let result = config.validate();
        assert!(result.is_ok());
    }

    #[test]
    fn test_config_blankhost_unencrypted_ok() {
        let config = SmtpConfig::new(SmtpTransportMode::Unencrypted(String::new()));
        let result = config.validate();
        assert!(matches!(result, Err(EmailError::ConfigurationError(_))));
    }

    #[test]
    fn test_config_blankhost_relay_ok() {
        let config = SmtpConfig::new(SmtpTransportMode::Relay(String::new()));
        let result = config.validate();
        assert!(matches!(result, Err(EmailError::ConfigurationError(_))));
    }

    #[test]
    fn test_config_blankhost_starttls_ok() {
        let config = SmtpConfig::new(SmtpTransportMode::StartTlsRelay(String::new()));
        let result = config.validate();
        assert!(matches!(result, Err(EmailError::ConfigurationError(_))));
    }

    #[test]
    fn test_config_relay_password_failure() {
        // Create the backend with our mock transport
        let config = SmtpConfig {
            mode: SmtpTransportMode::Relay("127.0.0.1".to_string()),
            username: Some("user@cotexample.com".to_string()),
            ..Default::default()
        };
        let result = config.validate();
        assert!(matches!(result, Err(EmailError::ConfigurationError(_))));
    }

    #[test]
    fn test_config_credentials_password_failure() {
        // Create the backend with our mock transport
        let config = SmtpConfig {
            mode: SmtpTransportMode::Relay("127.0.0.1".to_string()),
            username: Some("user@cotexample.com".to_string()),
            ..Default::default()
        };
        let result = Credentials::try_from(&config);
        assert!(matches!(result, Err(EmailError::ConfigurationError(_))));
    }
    #[test]
    fn test_config_credentials_username_failure() {
        // Create the backend with our mock transport
        let config = SmtpConfig {
            mode: SmtpTransportMode::Relay("127.0.0.1".to_string()),
            password: Some("user@cotexample.com".to_string()),
            ..Default::default()
        };
        let result = Credentials::try_from(&config);
        assert!(matches!(result, Err(EmailError::ConfigurationError(_))));
    }

    #[test]
    fn test_config_credentials_ok() {
        // Create the backend with our mock transport
        let config = SmtpConfig {
            mode: SmtpTransportMode::Relay("127.0.0.1".to_string()),
            username: Some("user@cotexample.com".to_string()),
            password: Some("asdDSasd87".to_string()),
            ..Default::default()
        };
        let result = Credentials::try_from(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_config_credentials_err() {
        // Create the backend with our mock transport
        let config = SmtpConfig {
            mode: SmtpTransportMode::Relay("127.0.0.1".to_string()),
            username: None,
            password: None,
            ..Default::default()
        };
        let result = Credentials::try_from(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_backend_config_ok() {
        // Create the backend with our mock transport
        let config = SmtpConfig::default();
        let backend = SmtpEmailBackend::new(config);
        assert!(backend.transport.is_none());
    }

    #[test]
    fn test_config_localhost_username_failure() {
        // Create the backend with our mock transport
        let config = SmtpConfig {
            mode: SmtpTransportMode::Localhost,
            password: Some("asdDSasd87".to_string()),
            ..Default::default()
        };
        let result = config.validate();
        assert!(matches!(result, Err(EmailError::ConfigurationError(_))));
    }

    #[test]
    fn test_send_email() {
        // Create a mock transport
        let mut mock_transport = MockEmailTransport::new();

        // Set expectations on the mock
        // Expect test_connection to be called once and return Ok(true)
        mock_transport
            .expect_test_connection()
            .times(1)
            .returning(|| Ok(true));

        // Expect send_email to be called once with any Message and return Ok(())
        mock_transport
            .expect_send_email()
            .times(1)
            .returning(|_| Ok(()));

        // Create a simple email for testing
        let email = EmailMessage {
            subject: "Test Email".to_string(),
            from: EmailAddress {
                address: "from@cotexample.com".to_string(),
                name: None,
            },
            to: vec!["to@cotexample.com".to_string()],
            body: "This is a test email sent from Rust.".to_string(),
            ..Default::default()
        };
        // Create SmtpConfig (the actual config doesn't matter as we're using a mock)
        let config = SmtpConfig::default();

        // Create the backend with our mock transport
        let mut backend = SmtpEmailBackend::from_config(config, Box::new(mock_transport));

        // Try to send the email - this should succeed
        let result = backend.send_message(&email);

        // Verify that the email was sent successfully
        assert!(result.is_ok());
    }

    #[test]
    fn test_send_email_send_ok() {
        // Create a mock transport
        let mut mock_transport = MockEmailTransport::new();

        // Set expectations - test_connection succeeds but send_email fails
        mock_transport
            .expect_test_connection()
            .times(1)
            .returning(|| Ok(true));

        mock_transport
            .expect_send_email()
            .times(1)
            .returning(|_| Ok(()));

        // Create a simple email for testing
        let email = EmailMessage {
            subject: "Test Email".to_string(),
            from: EmailAddress {
                address: "from@cotexample.com".to_string(),
                name: None,
            },
            to: vec!["to@cotexample.com".to_string()],
            body: "This is a test email #1.".to_string(),
            ..Default::default()
        };

        // Create SmtpConfig (the actual config doesn't matter as we're using a mock)
        let config = SmtpConfig::default();

        // Create the backend with our mock transport
        let mut backend = SmtpEmailBackend::from_config(config, Box::new(mock_transport));

        // Send the email - this should succeed with our mock
        let result = backend.send_message(&email);
        eprintln!("Result: {:?}", result);

        // Assert that the email was sent successfully
        assert!(result.is_ok());
    }

    #[test]
    fn test_backend_close() {
        // Create a mock transport
        let mock_transport = MockEmailTransport::new();
        let config = SmtpConfig::default();
        let mut backend = SmtpEmailBackend::from_config(config, Box::new(mock_transport));

        let result = backend.close();
        assert!(result.is_ok());
    }

    #[test]
    fn test_send_email_send_failure() {
        // Create a mock transport
        let mut mock_transport = MockEmailTransport::new();

        // Set expectations - test_connection succeeds but send_email fails
        mock_transport
            .expect_test_connection()
            .times(1)
            .returning(|| Ok(true));

        mock_transport
            .expect_send_email()
            .times(1)
            .returning(|_| Err(EmailError::SendError("Mock send failure".to_string())));

        // Create a simple email for testing
        let email = EmailMessage {
            subject: "Test Email".to_string(),
            from: String::from("from@cotexample.com").into(),
            to: vec!["to@cotexample.com".to_string()],
            cc: Some(vec!["cc@cotexample.com".to_string()]),
            bcc: Some(vec!["bcc@cotexample.com".to_string()]),
            reply_to: Some(vec!["anonymous@cotexample.com".to_string()]),
            body: "This is a test email sent from Rust.".to_string(),
            alternative_html: Some("This is a test email sent from Rust as HTML.".to_string()),
        };

        // Create the backend with our mock transport
        let config = SmtpConfig {
            mode: SmtpTransportMode::Relay("invalid-host".to_string()),
            port: Some(587),
            username: Some("user@cotexample.com".to_string()),
            ..Default::default()
        };

        let mut backend = SmtpEmailBackend::from_config(config, Box::new(mock_transport));

        // Try to send the email - this should fail
        let result = backend.send_message(&email);
        eprintln!("Result: {:?}", result);

        // Verify that we got a send error
        assert!(matches!(result, Err(EmailError::SendError(_))));
    }

    #[test]
    fn test_send_multiple_emails() {
        // Create a mock transport
        let mut mock_transport = MockEmailTransport::new();

        // Set expectations - test_connection succeeds and send_email succeeds for both
        // emails
        mock_transport
            .expect_test_connection()
            .times(1..)
            .returning(|| Ok(true));

        mock_transport
            .expect_send_email()
            .times(2)
            .returning(|_| Ok(()));

        // Create test emails
        let emails = vec![
            EmailMessage {
                subject: "Test Email".to_string(),
                from: String::from("from@cotexample.com").into(),
                to: vec!["to@cotexample.com".to_string()],
                body: "This is a test email #1.".to_string(),
                ..Default::default()
            },
            EmailMessage {
                subject: "Test Email".to_string(),
                from: String::from("from@cotexample.com").into(),
                to: vec!["to@cotexample.com".to_string()],
                body: "This is a test email #2.".to_string(),
                ..Default::default()
            },
        ];

        // Create the backend with our mock transport
        let config = SmtpConfig::default();
        let mut backend = SmtpEmailBackend::from_config(config, Box::new(mock_transport));

        // Send the emails
        let result = backend.send_messages(&emails);

        // Verify that both emails were sent successfully
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 2);
    }

    // An integration test to send an email to localhost using the default
    // configuration. Dependent on the mail server running on localhost, this
    // test may fail/hang if the server is not available.
    #[test]
    #[ignore]
    fn test_send_email_localhost() {
        // Create a test email
        let email = EmailMessage {
            subject: "Test Email".to_string(),
            from: String::from("from@cotexample.com").into(),
            to: vec!["to@cotexample.com".to_string()],
            cc: Some(vec!["cc@cotexample.com".to_string()]),
            bcc: Some(vec!["bcc@cotexample.com".to_string()]),
            reply_to: Some(vec!["anonymous@cotexample.com".to_string()]),
            body: "This is a test email sent from Rust.".to_string(),
            alternative_html: Some("This is a test email sent from Rust as HTML.".to_string()),
        };

        // Get the port it's running on
        let port = 1025; //Mailhog default smtp port
        let config = SmtpConfig {
            mode: SmtpTransportMode::Unencrypted("localhost".to_string()),
            port: Some(port),
            ..Default::default()
        };
        // Create a new email backend
        let mut backend = SmtpEmailBackend::new(config);

        let result = backend.send_message(&email);
        assert!(result.is_ok());
    }
    #[test]
    fn test_open_method_with_existing_working_transport() {
        // Create a mock transport that will pass connection test
        let mut mock_transport = MockEmailTransport::new();
        mock_transport
            .expect_test_connection()
            .times(2)
            .returning(|| Ok(true));

        // Create config and backend
        let config = SmtpConfig::default();
        let mut backend = SmtpEmailBackend::from_config(config, Box::new(mock_transport));

        // First open should succeed
        let result = backend.open();
        assert!(result.is_ok());

        // Second open should also succeed without reinitializing
        let result = backend.open();
        assert!(result.is_ok());
    }

    #[test]
    fn test_open_method_with_failed_connection() {
        // Create a mock transport that will fail connection test
        let mut mock_transport = MockEmailTransport::new();
        mock_transport
            .expect_test_connection()
            .times(1)
            .returning(|| {
                Err(EmailError::ConnectionError(
                    "Mock connection failure".to_string(),
                ))
            });
        // Mock the from_config method to return a transport
        // Create config and backend
        let config = SmtpConfig::default();
        let mut backend = SmtpEmailBackend::from_config(config, Box::new(mock_transport));
        // Open should fail due to connection error
        let result = backend.open();
        assert!(result.is_err());
        assert!(backend.transport_state == TransportState::Uninitialized);
    }

    #[test]
    fn test_init_only_username_connection() {
        // Create a mock transport that will fail connection test
        let mock_transport = MockEmailTransport::new();
        // Mock the from_config method to return a transport
        // Create config and backend
        let config = SmtpConfig {
            mode: SmtpTransportMode::Unencrypted("localhost".to_string()),
            username: Some("justtheruser".to_string()),
            ..Default::default()
        };
        let mut backend = SmtpEmailBackend::from_config(config, Box::new(mock_transport));
        assert!(backend.transport_state == TransportState::Uninitialized);
        let result = backend.init();
        assert!(matches!(result, Err(EmailError::ConfigurationError(_))));
        assert!(backend.transport_state == TransportState::Uninitialized);
    }

    #[test]
    fn test_init_ok_unencrypted_connection() {
        // Create a mock transport that will fail connection test
        let mock_transport = MockEmailTransport::new();
        // Create config and backend
        let config = SmtpConfig {
            mode: SmtpTransportMode::Unencrypted("localhost".to_string()),
            ..Default::default()
        };
        let mut backend = SmtpEmailBackend::from_config(config, Box::new(mock_transport));
        assert!(backend.transport_state == TransportState::Uninitialized);
        let result = backend.init();
        assert!(result.is_ok());
        assert!(backend.transport_state == TransportState::Initialized);
    }

    #[test]
    fn test_init_with_relay_credentials() {
        // Create a mock transport that will fail connection test
        let mock_transport = MockEmailTransport::new();
        // Mock the from_config method to return a transport
        // Create config and backend
        let config = SmtpConfig {
            mode: SmtpTransportMode::Relay("localhost".to_string()),
            username: Some("justtheruser".to_string()),
            password: Some("asdf877DF".to_string()),
            port: Some(25),
            ..Default::default()
        };
        let mut backend = SmtpEmailBackend::from_config(config, Box::new(mock_transport));
        // Open should fail due to connection error
        assert!(backend.transport_state == TransportState::Uninitialized);
        let result = backend.init();
        assert!(result.is_ok());
        assert!(backend.transport_state == TransportState::Initialized);
    }

    #[test]
    fn test_init_with_tlsrelay_credentials() {
        // Create a mock transport that will fail connection test
        let mock_transport = MockEmailTransport::new();
        // Mock the from_config method to return a transport
        // Create config and backend
        let config = SmtpConfig {
            mode: SmtpTransportMode::StartTlsRelay("junkyhost".to_string()),
            username: Some("justtheruser".to_string()),
            password: Some("asdf877DF".to_string()),
            ..Default::default()
        };
        let mut backend = SmtpEmailBackend::from_config(config, Box::new(mock_transport));
        assert!(backend.transport_state == TransportState::Uninitialized);
        let result = backend.init();
        assert!(result.is_ok());
        assert!(backend.transport_state == TransportState::Initialized);
    }

    #[test]
    fn test_email_error_variants() {
        let message_error = EmailError::MessageError("Invalid message".to_string());
        assert_eq!(format!("{message_error}"), "Message error: Invalid message");

        let config_error = EmailError::ConfigurationError("Invalid config".to_string());
        assert_eq!(
            format!("{config_error}"),
            "Invalid email configuration: Invalid config"
        );

        let send_error = EmailError::SendError("Failed to send".to_string());
        assert_eq!(format!("{send_error}"), "Send error: Failed to send");

        let connection_error = EmailError::ConnectionError("Failed to connect".to_string());
        assert_eq!(
            format!("{connection_error}"),
            "Connection error: Failed to connect"
        );
    }
}
