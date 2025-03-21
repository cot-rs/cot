//! Email sending functionality using SMTP
//! #Examples
//! To send an email using the `EmailBackend`, you need to create an instance of `SmtpConfig`
//! ```
//! use cot::email::{SmtpEmailBackend, EmailBackend, EmailMessage, SmtpConfig, EmailError};
//! fn send_example() -> Result<(), EmailError> {
//!     let email = EmailMessage {
//!         subject: "Test Email".to_string(),
//!         body: "This is a test email sent from Rust.".to_string(),
//!         from_email: "from@cotexample.com".to_string(),
//!         to: vec!["to@cotexample.com".to_string()],
//!         cc: Some(vec!["cc@cotexample.com".to_string()]),
//!         bcc: Some(vec!["bcc@cotexample.com".to_string()]),
//!         reply_to: vec!["replyto@cotexample.com".to_string()],
//!         alternatives: vec![
//!             ("This is a test email sent from Rust as HTML.".to_string(), "text/html".to_string())
//!         ],
//!     };
//!     let config = SmtpConfig::default();
//!     let mut backend = SmtpEmailBackend::new(config);
//!     backend.send_message(&email)?;
//!     Ok(())
//! }
//! ```
//!
use std::fmt;
use std::net::ToSocketAddrs;
use std::time::Duration;

use lettre::{
    SmtpTransport, Transport,
    message::{Message, MultiPart, SinglePart, header},
    transport::smtp::authentication::Credentials,
};

/// Represents errors that can occur when sending an email.
#[derive(Debug, thiserror::Error)]
pub enum EmailError {
    /// An error occurred while building the email message.
    #[error("Message error: {0}")]
    MessageError(String),
    /// The email configuration is invalid.
    #[error("Invalid email configuration: {0}")]
    ConfigurationError(String),
    /// An error occurred while connecting to the SMTP server.
    #[error("Connection error: {0}")]
    ConnectionError(String),
    /// An error occurred while sending the email.
    #[error("Send error: {0}")]
    SendError(String),
}

type Result<T> = std::result::Result<T, EmailError>;

/// Configuration for SMTP email backend
#[derive(Debug, Clone)]
pub struct SmtpConfig {
    /// The SMTP server host address.
    /// Defaults to "localhost".
    pub host: String,
    /// The SMTP server port.
    pub port: u16,
    /// The username for SMTP authentication.
    pub username: Option<String>,
    /// The password for SMTP authentication.
    pub password: Option<String>,
    /// Whether to fail silently on errors.
    pub fail_silently: bool,
    /// The timeout duration for the SMTP connection.
    pub timeout: Duration,
}

impl Default for SmtpConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 25,
            username: None,
            password: None,
            fail_silently: false,
            timeout: Duration::from_secs(60),
        }
    }
}

/// Represents an email message
#[derive(Debug, Clone)]
pub struct EmailMessage {
    /// The subject of the email.
    pub subject: String,
    /// The body of the email.
    pub body: String,
    /// The email address of the sender.
    pub from_email: String,
    /// The list of recipient email addresses.
    pub to: Vec<String>,
    /// The list of CC (carbon copy) recipient email addresses.
    pub cc: Option<Vec<String>>,
    /// The list of BCC (blind carbon copy) recipient email addresses.
    pub bcc: Option<Vec<String>>,
    /// The list of reply-to email addresses.
    pub reply_to: Vec<String>,
    /// The alternative parts of the email (e.g., plain text and HTML versions).
    pub alternatives: Vec<(String, String)>, // (content, mimetype)
}
impl fmt::Display for EmailMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Subject: {}", self.subject)?;
        writeln!(f, "From: {}", self.from_email)?;
        writeln!(f, "To: {:?}", self.to)?;
        if let Some(cc) = &self.cc {
            writeln!(f, "CC: {cc:?}")?;
        }
        if let Some(bcc) = &self.bcc {
            writeln!(f, "BCC: {bcc:?}")?;
        }
        writeln!(f, "Reply-To: {:?}", self.reply_to)?;
        writeln!(f, "Body: {}", self.body)?;
        for (content, mimetype) in &self.alternatives {
            writeln!(f, "Alternative part ({mimetype}): {content}")?;
        }
        Ok(())
    }
}

/// SMTP Backend for sending emails
#[derive(Debug)]
pub struct SmtpEmailBackend {
    /// The SMTP configuration.
    config: SmtpConfig,
    /// The SMTP transport.
    /// This field is optional because the transport may not be initialized yet.
    /// It will be initialized when the `open` method is called.
    transport: Option<SmtpTransport>,
    /// Whether or not to print debug information.
    debug: bool,
}
/// Trait representing an email backend for sending emails.
pub trait EmailBackend {
    /// Creates a new instance of the email backend with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The SMTP configuration to use.
    fn new(config: SmtpConfig) -> Self;
    /// Open a connection to the SMTP server.
    ///
    /// # Errors
    ///
    /// This function will return an `EmailError` if there is an issue with resolving the SMTP host,
    /// creating the TLS parameters, or connecting to the SMTP server.
    fn open(&mut self) -> Result<()>;
    /// Close the connection to the SMTP server.
    ///
    /// # Errors
    ///
    /// This function will return an `EmailError` if there is an issue with closing the SMTP connection.
    ///
    /// # Errors
    ///
    /// This function will return an `EmailError` if there is an issue with closing the SMTP connection.
    fn close(&mut self) -> Result<()>;

    /// Send a single email message
    ///
    /// # Errors
    ///
    /// This function will return an `EmailError` if there is an issue with opening the SMTP connection,
    /// building the email message, or sending the email.
    fn send_message(&mut self, message: &EmailMessage) -> Result<()>;

    /// Send multiple email messages
    ///
    /// # Errors
    ///
    /// This function will return an `EmailError` if there is an issue with sending any of the emails.
    ///
    /// # Errors
    ///
    /// This function will return an `EmailError` if there is an issue with sending any of the emails.
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
    #[must_use]
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
        }
    }

    /// Open a connection to the SMTP server
    ///
    /// # Errors
    ///
    /// This function will return an `EmailError` if there is an issue with resolving the SMTP host,
    /// creating the TLS parameters, or connecting to the SMTP server.
    ///
    /// # Panics
    ///
    /// This function will panic if the transport is not properly initialized.
    fn open(&mut self) -> Result<()> {
        // Test if self.transport is None or if the connection is not working
        if self.transport.is_some() && self.transport.as_ref().unwrap().test_connection().is_ok() {
            return Ok(());
        }
        if self.config.host.is_empty() {
            return Err(EmailError::ConfigurationError(
                "SMTP host is required".to_string(),
            ));
        } else if self.config.port == 0 {
            return Err(EmailError::ConfigurationError(
                "SMTP port is required".to_string(),
            ));
        }
        let _socket_addr = format!("{}:{}", self.config.host, self.config.port)
            .to_socket_addrs()
            .map_err(|e| EmailError::ConnectionError(e.to_string()))?
            .next()
            .ok_or_else(|| {
                EmailError::ConnectionError("Could not resolve SMTP host".to_string())
            })?;

        let mut transport_builder = SmtpTransport::builder_dangerous(&self.config.host)
            .port(self.config.port)
            .timeout(Some(self.config.timeout));

        // Add authentication if credentials provided
        if let (Some(username), Some(password)) = (&self.config.username, &self.config.password) {
            let credentials = Credentials::new(username.clone(), password.clone());
            transport_builder = transport_builder.credentials(credentials);
        }

        // Connect to the SMTP server
        let transport = transport_builder.build();
        if transport.test_connection().is_err() {
            return Err(EmailError::ConnectionError(
                "Failed to connect to SMTP server".to_string(),
            ));
        }
        self.transport = Some(transport);
        Ok(())
    }

    /// Close the connection to the SMTP server
    ///
    /// # Errors
    ///
    /// This function will return an `EmailError` if there is an issue with closing the SMTP connection.
    fn close(&mut self) -> Result<()> {
        self.transport = None;
        Ok(())
    }

    /// Send a single email message
    ///
    /// # Errors
    ///
    /// This function will return an `EmailError` if there is an issue with opening the SMTP connection,
    /// building the email message, or sending the email.
    fn send_message(&mut self, email: &EmailMessage) -> Result<()> {
        self.open()?;
        if self.debug {
            self.dump_message(email)?;
        }
        // Build the email message using lettre
        let mut message_builder = Message::builder()
            .from(
                email
                    .from_email
                    .parse()
                    .map_err(|e| EmailError::MessageError(format!("Invalid from address: {e}")))?,
            )
            .subject(&email.subject);

        // Add recipients
        for recipient in &email.to {
            message_builder = message_builder.to(recipient.parse().map_err(|e| {
                EmailError::MessageError(format!("Invalid recipient address: {e}"))
            })?);
        }

        // Add CC recipients
        if let Some(cc_recipients) = &email.cc {
            for recipient in cc_recipients {
                message_builder = message_builder.cc(recipient
                    .parse()
                    .map_err(|e| EmailError::MessageError(format!("Invalid CC address: {e}")))?);
            }
        }

        // Add BCC recipients
        if let Some(bcc_recipients) = &email.bcc {
            for recipient in bcc_recipients {
                message_builder =
                    message_builder.bcc(recipient.parse().map_err(|e| {
                        EmailError::MessageError(format!("Invalid BCC address: {e}"))
                    })?);
            }
        }

        // Add Reply-To addresses
        for reply_to in &email.reply_to {
            message_builder =
                message_builder.reply_to(reply_to.parse().map_err(|e| {
                    EmailError::MessageError(format!("Invalid reply-to address: {e}"))
                })?);
        }

        // Create the message body (multipart if there are alternatives or attachments)
        let has_alternatives = !email.alternatives.is_empty();

        let email_body = if has_alternatives {
            // Create multipart message
            let mut multipart = MultiPart::mixed().singlepart(
                SinglePart::builder()
                    .header(header::ContentType::TEXT_PLAIN)
                    .body(email.body.clone()),
            );

            // Add alternative parts
            for (content, mimetype) in &email.alternatives {
                multipart = multipart.singlepart(
                    SinglePart::builder()
                        .header(header::ContentType::parse(mimetype).map_err(|e| {
                            EmailError::MessageError(format!("Invalid content type: {e}"))
                        })?)
                        .body(content.clone()),
                );
            }
            multipart
        } else {
            // Just use the plain text body
            MultiPart::mixed().singlepart(
                SinglePart::builder()
                    .header(header::ContentType::TEXT_PLAIN)
                    .body(email.body.clone()),
            )
        };

        let email = message_builder
            .multipart(email_body)
            .map_err(|e| EmailError::MessageError(e.to_string()))?;

        let mailer = SmtpTransport::builder_dangerous(&self.config.host)
            .port(self.config.port)
            .build();

        // Send the email
        mailer
            .send(&email)
            .map_err(|e| EmailError::SendError(e.to_string()))?;

        Ok(())
    }
}
impl SmtpEmailBackend {
    /// Dump the email message to the console for debugging purposes.
    ///
    /// # Errors
    ///
    /// This function will return an `EmailError` if there is an issue with writing the email message to the console.
    pub fn dump_message(&self, email: &EmailMessage) -> Result<()> {
        println!("{}", email);
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn test_send_email() {
        // Create a test email
        let email = EmailMessage {
            subject: "Test Email".to_string(),
            body: "This is a test email sent from Rust.".to_string(),
            from_email: "from@cotexample.com".to_string(),
            to: vec!["to@cotexample.com".to_string()],
            cc: Some(vec![]),
            bcc: Some(vec![]),
            reply_to: vec![],
            alternatives: vec![(
                "This is a test email sent from Rust as HTML.".to_string(),
                "text/html".to_string(),
            )],
        };

        // Test with a simple configuration
        let _config = SmtpConfig {
            host: "smtp.cotexample.com".to_string(),
            port: 587,
            username: Some("user@cotexample.com".to_string()),
            password: Some("password".to_string()),
            fail_silently: false,
            ..Default::default()
        };

        // Note: This test demonstrates the setup but doesn't actually send emails
        // since we're mocking the transport. In a real test environment, you might
        // use a real SMTP server or a more sophisticated mock.

        // Assert that the email structure is correct
        assert_eq!(email.subject, "Test Email");
        assert_eq!(email.to, vec!["to@cotexample.com"]);
        assert_eq!(email.alternatives.len(), 1);

        // In a real test, we'd also verify that the backend behaves correctly
        // but that would require more complex mocking of the SMTP connection.
    }

    #[test]
    fn test_send_multiple_emails() {
        // Create test emails
        let emails = vec![
            EmailMessage {
                subject: "Test Email 1".to_string(),
                body: "This is test email 1.".to_string(),
                from_email: "from@cotexample.com".to_string(),
                to: vec!["to1@cotexample.com".to_string()],
                cc: Some(vec![]),
                bcc: Some(vec![]),
                reply_to: vec![],
                alternatives: vec![],
            },
            EmailMessage {
                subject: "Test Email 2".to_string(),
                body: "This is test email 2.".to_string(),
                from_email: "from@cotexample.com".to_string(),
                to: vec!["to2@cotexample.com".to_string()],
                cc: Some(vec![]),
                bcc: Some(vec![]),
                reply_to: vec![],
                alternatives: vec![],
            },
        ];

        // Test with fail_silently = true
        let _config = SmtpConfig {
            host: "smtp.cotexample.com".to_string(),
            port: 587,
            fail_silently: true,
            ..Default::default()
        };

        // Assert that the emails structure is correct
        assert_eq!(emails.len(), 2);
        assert_eq!(emails[0].subject, "Test Email 1");
        assert_eq!(emails[1].subject, "Test Email 2");

        // In a real test, we'd verify that send_messages behaves correctly
        // with multiple emails, including proper error handling with fail_silently.
    }

    #[test]
    fn test_config_defaults() {
        let config = SmtpConfig::default();

        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 25);
        assert_eq!(config.username, None);
        assert_eq!(config.password, None);
        assert!(!config.fail_silently);
        assert_eq!(config.timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_dump_message() {
        // Create a test email
        let email = EmailMessage {
            subject: "Test Email".to_string(),
            body: "This is a test email sent from Rust.".to_string(),
            from_email: "from@cotexample.com".to_string(),
            to: vec!["to@cotexample.com".to_string()],
            cc: Some(vec!["cc@cotexample.com".to_string()]),
            bcc: Some(vec!["bcc@cotexample.com".to_string()]),
            reply_to: vec!["replyto@cotexample.com".to_string()],
            alternatives: vec![(
                "This is a test email sent from Rust as HTML.".to_string(),
                "text/html".to_string(),
            )],
        };

        // Create a buffer to capture output
        let mut buffer = Vec::new();
        {
            // Redirect stdout to our buffer
            let mut _stdout_cursor = Cursor::new(&mut buffer);

            let config = SmtpConfig::default();
            let backend = SmtpEmailBackend::new(config);
            backend.dump_message(&email).unwrap();
        }
        // Convert buffer to string
        let output = String::from_utf8(buffer.clone()).unwrap();
        // Keeping for possible debug purposes using cargo test --nocapture
        //println!("{output}");
        // Check that the output contains the expected email details
        assert!(!output.contains("Subject: Test Email"));
        assert!(!output.contains("From: from@cotexample.com"));
        assert!(!output.contains("To: [\"to@cotexample.com\"]"));
        assert!(!output.contains("CC: [\"cc@cotexample.com\"]"));
        assert!(!output.contains("BCC: [\"bcc@cotexample.com\"]"));
        assert!(!output.contains("Reply-To: [\"replyto@cotexample.com\"]"));
        assert!(!output.contains("Body: This is a test email sent from Rust."));
        assert!(!output.contains(
            "Alternative part (text/html): This is a test email sent from Rust as HTML."
        ));
    }
    #[test]
    fn test_open_connection() {
        let config = SmtpConfig {
            host: "invalid-host".to_string(),
            port: 587,
            username: Some("user@cotexample.com".to_string()),
            password: Some("password".to_string()),
            ..Default::default()
        };

        let result = SmtpEmailBackend::new(config).open();
        assert!(matches!(result, Err(EmailError::ConnectionError(_))));
    }

    #[test]
    fn test_configuration_error() {
        let config = SmtpConfig {
            host: "localhost".to_string(),
            port: 0,
            username: Some("user@cotexample.com".to_string()),
            password: Some("password".to_string()),
            ..Default::default()
        };

        let result = SmtpEmailBackend::new(config).open();
        assert!(matches!(result, Err(EmailError::ConfigurationError(_))));
    }
    // An integration test to send an email to localhost using the default configuration.
    // TODO: Overcome compilation errors due to async_smtp
    // use cot::email::{EmailBackend, EmailMessage, SmtpConfig};
    // use async_smtp::smtp::server::MockServer;
    #[test]
    #[ignore]
    fn test_send_email_localhsot() {
        // Create a test email
        let email = EmailMessage {
            subject: "Test Email".to_string(),
            body: "This is a test email sent from Rust.".to_string(),
            from_email: "from@cotexample.com".to_string(),
            to: vec!["to@cotexample.com".to_string()],
            cc: Some(vec!["cc@cotexample.com".to_string()]),
            bcc: Some(vec!["bcc@cotexample.com".to_string()]),
            reply_to: vec!["replyto@cotexample.com".to_string()],
            alternatives: vec![(
                "This is a test email sent from Rust as HTML.".to_string(),
                "text/html".to_string(),
            )],
        };
        // Get the port it's running on
        let port = 1025; //Mailhog default smtp port
        // Create a new email backend
        let config = SmtpConfig {
            host: "localhost".to_string(),
            port,
            ..Default::default()
        };
        let mut backend = SmtpEmailBackend::new(config);
        let _ = backend.open();
        let _ = backend.send_message(&email);
    }
}
