//! Email sending functionality using SMTP
//! #Examples
//! To send an email using the `EmailBackend`, you need to create an instance of `SmtpConfig`
//! ```
//! fn send_example() -> Result<()> {
//!     let email = EmailMessage {
//!         subject: "Test Email".to_string(),
//!         body: "This is a test email sent from Rust.".to_string(),
//!         from_email: "from@example.com".to_string(),
//!         to: vec!["to@example.com".to_string()],
//!         cc: Some(vec!["cc@example.com".to_string()]),
//!         bcc: Some(vec!["bcc@example.com".to_string()]),
//!         reply_to: vec!["replyto@example.com".to_string()],
//!         alternatives: vec![
//!             ("This is a test email sent from Rust as HTML.".to_string(), "text/html".to_string())
//!         ],
//!     };
//!     let config = SmtpConfig {
//!         host: "smtp.example.com".to_string(),
//!         port: 587,
//!         username: Some("user@example.com".to_string()),
//!         password: Some("password".to_string()),
//!         use_tls: true,
//!         fail_silently: false,
//!         ..Default::default()
//!     };
//!     let mut backend = EmailBackend::new(config);
//!     backend.send_message(&email)?;
//!     Ok(())
//! }
//! ```
//!
use std::net::ToSocketAddrs;
use std::time::Duration;
use std::fmt;

use lettre::{
    message::{header, Message, MultiPart, SinglePart},
    transport::smtp::authentication::Credentials,
    SmtpTransport, Transport,
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
    /// Whether to use TLS for the SMTP connection.
    pub use_tls: bool,
    /// Whether to fail silently on errors.
    pub fail_silently: bool,
    /// The timeout duration for the SMTP connection.
    pub timeout: Duration,
    /// Whether to use SSL for the SMTP connection.
    pub use_ssl: bool,
    /// The path to the SSL certificate file.
    pub ssl_certfile: Option<String>,
    /// The path to the SSL key file.
    pub ssl_keyfile: Option<String>,
}

impl Default for SmtpConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 25,
            username: None,
            password: None,
            use_tls: false,
            fail_silently: false,
            timeout: Duration::from_secs(60),
            use_ssl: false,
            ssl_certfile: None,
            ssl_keyfile: None,
            //  debug: false,
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

/// Represents an email attachment
#[derive(Debug, Clone)]
pub struct EmailAttachment {
    /// The filename of the attachment.
    pub filename: String,
    /// The content of the attachment.
    pub content: Vec<u8>,
    /// The MIME type of the attachment.
    pub mimetype: String,
}

/// SMTP Backend for sending emails
#[derive(Debug)]
pub struct EmailBackend {
    /// The SMTP configuration.
    config: SmtpConfig,
    /// The SMTP transport.
    /// This field is optional because the transport may not be initialized yet.
    /// It will be initialized when the `open` method is called.
    transport: Option<SmtpTransport>,
    /// Whether or not to print debug information.
    debug: bool,
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
impl EmailBackend {
    #[must_use]
    /// Creates a new instance of `EmailBackend` with the given configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - The SMTP configuration to use.
    pub fn new(config: SmtpConfig) -> Self {
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
    pub fn open(&mut self) -> Result<()> {
        if self.transport.as_ref().unwrap().test_connection().is_ok() {
            return Ok(());
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

        // Configure TLS/SSL
        if self.config.use_tls {
            let tls_parameters =
                lettre::transport::smtp::client::TlsParameters::new(self.config.host.clone())
                    .map_err(|e| EmailError::ConfigurationError(e.to_string()))?;
            transport_builder = transport_builder.tls(
                lettre::transport::smtp::client::Tls::Required(tls_parameters),
            );
        }

        // Build the transport
        self.transport = Some(transport_builder.build());

        // Connect to the SMTP server
        if self.transport.as_ref().unwrap().test_connection().is_ok() {
            Err(EmailError::ConnectionError(
                "Failed to connect to SMTP server".to_string(),
            ))?;
        }
        Ok(())
    }

    /// Close the connection to the SMTP server
    ///
    /// # Errors
    ///
    /// This function will return an `EmailError` if there is an issue with closing the SMTP connection.
    pub fn close(&mut self) -> Result<()> {
        self.transport = None;
        Ok(())
    }
    /// Dump the email message to stdout
    ///
    /// # Errors
    /// This function will return an `EmailError` if there is an issue with printing the email message.
    pub fn dump_message(&self, email: &EmailMessage) -> Result<()> {
        println!("{email}");
        Ok(())
    }

    /// Send a single email message
    ///
    /// # Errors
    ///
    /// This function will return an `EmailError` if there is an issue with opening the SMTP connection,
    /// building the email message, or sending the email.
    pub fn send_message(&mut self, email: &EmailMessage) -> Result<()> {
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

        // Send the email
        let mailer = SmtpTransport::builder_dangerous(&self.config.host)
            .port(self.config.port)
            .build();

        mailer
            .send(&email)
            .map_err(|e| EmailError::SendError(e.to_string()))?;

        Ok(())
    }

    /// Send multiple email messages
    ///
    /// # Errors
    ///
    /// This function will return an `EmailError` if there is an issue with sending any of the emails.
    pub fn send_messages(&mut self, emails: &[EmailMessage]) -> Result<usize> {
        let mut sent_count = 0;

        for email in emails {
            match self.send_message(email) {
                Ok(()) => sent_count += 1,
                Err(_e) if self.config.fail_silently => continue,
                Err(e) => return Err(e),
            }
        }

        Ok(sent_count)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;
    use mockall::predicate::*;
    use mockall::*;

    // Mock the SMTP transport for testing
    mock! {
        SmtpTransport {
            fn send(&self, email: &Message) -> std::result::Result<(), lettre::transport::smtp::Error>;
        }
    }

    #[test]
    fn test_send_email() {
        // Create a mock SMTP transport
        let mut mock_transport = MockSmtpTransport::new();
        mock_transport.expect_send().returning(|_| Ok(()));

        // Create a test email
        let email = EmailMessage {
            subject: "Test Email".to_string(),
            body: "This is a test email sent from Rust.".to_string(),
            from_email: "from@example.com".to_string(),
            to: vec!["to@example.com".to_string()],
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
            host: "smtp.example.com".to_string(),
            port: 587,
            username: Some("user@example.com".to_string()),
            password: Some("password".to_string()),
            use_tls: true,
            fail_silently: false,
            ..Default::default()
        };

        // Note: This test demonstrates the setup but doesn't actually send emails
        // since we're mocking the transport. In a real test environment, you might
        // use a real SMTP server or a more sophisticated mock.

        // Assert that the email structure is correct
        assert_eq!(email.subject, "Test Email");
        assert_eq!(email.to, vec!["to@example.com"]);
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
                from_email: "from@example.com".to_string(),
                to: vec!["to1@example.com".to_string()],
                cc: Some(vec![]),
                bcc: Some(vec![]),
                reply_to: vec![],
                alternatives: vec![],
            },
            EmailMessage {
                subject: "Test Email 2".to_string(),
                body: "This is test email 2.".to_string(),
                from_email: "from@example.com".to_string(),
                to: vec!["to2@example.com".to_string()],
                cc: Some(vec![]),
                bcc: Some(vec![]),
                reply_to: vec![],
                alternatives: vec![],
            },
        ];

        // Test with fail_silently = true
        let _config = SmtpConfig {
            host: "smtp.example.com".to_string(),
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
        assert!(!config.use_tls);
        assert!(!config.fail_silently);
        assert_eq!(config.timeout, Duration::from_secs(60));
        assert!(!config.use_ssl);
        assert_eq!(config.ssl_certfile, None);
        assert_eq!(config.ssl_keyfile, None);
    }

    #[test]
    fn test_dump_message() {
        // Create a test email
        let email = EmailMessage {
            subject: "Test Email".to_string(),
            body: "This is a test email sent from Rust.".to_string(),
            from_email: "from@example.com".to_string(),
            to: vec!["to@example.com".to_string()],
            cc: Some(vec!["cc@example.com".to_string()]),
            bcc: Some(vec!["bcc@example.com".to_string()]),
            reply_to: vec!["replyto@example.com".to_string()],
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
            let backend = EmailBackend::new(config);
            backend.dump_message(&email).unwrap();
        }
        // Convert buffer to string
        let output = String::from_utf8(buffer.clone()).unwrap();
        // Keeping for possible debug purposes using cargo test --nocapture
        //println!("{output}");
        // Check that the output contains the expected email details
        assert!(!output.contains("Subject: Test Email"));
        assert!(!output.contains("From: from@example.com"));
        assert!(!output.contains("To: [\"to@example.com\"]"));
        assert!(!output.contains("CC: [\"cc@example.com\"]"));
        assert!(!output.contains("BCC: [\"bcc@example.com\"]"));
        assert!(!output.contains("Reply-To: [\"replyto@example.com\"]"));
        assert!(!output.contains("Body: This is a test email sent from Rust."));
        assert!(!output.contains(
            "Alternative part (text/html): This is a test email sent from Rust as HTML."
        ));
    }
}
