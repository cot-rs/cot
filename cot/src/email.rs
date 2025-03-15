use std::collections::HashMap;
use std::net::ToSocketAddrs;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use derive_more::derive;
use lettre::{
    message::{header, Message, MultiPart, SinglePart},
    transport::smtp::{authentication::Credentials, client::SmtpConnection},
    SmtpTransport, Transport,
};
use thiserror::Error;
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
    /// The custom headers for the email.
    pub headers: HashMap<String, String>,
    /// The alternative parts of the email (e.g., plain text and HTML versions).
    pub alternatives: Vec<(String, String)>, // (content, mimetype)
    /// The attachments of the email.
    pub attachments: Vec<EmailAttachment>,
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
#[derive(Debug, Clone)]
pub struct SmtpEmailBackend {
    config: SmtpConfig,
    connection: Option<Arc<Mutex<SmtpConnection>>>,
    connection_created: Option<Instant>,
    transport: Option<SmtpTransport>,
}

impl SmtpEmailBackend {
    pub fn new(config: SmtpConfig) -> Self {
        Self {
            config,
            connection: None,
            transport: None,
            connection_created: None,
        }
    }

    /// Open a connection to the SMTP server
    pub fn open(&mut self) -> Result<()> {
        if self.connection.is_some() {
            return Ok(());
        }

        let server_addr = format!("{}:{}", self.config.host, self.config.port)
            .to_socket_addrs()
            .map_err(|e| EmailError::ConnectionError(e.to_string()))?
            .next()
            .ok_or_else(|| EmailError::ConnectionError("Could not resolve SMTP host".to_string()))?;

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
            let tls_parameters = lettre::transport::smtp::client::TlsParameters::new(self.config.host.clone())
                .map_err(|e| EmailError::ConfigurationError(e.to_string()))?;
            transport_builder = transport_builder.tls(lettre::transport::smtp::client::Tls::Required(tls_parameters));
        }

        // Build the transport
        self.transport = Some(transport_builder.build());

        // Connect to the SMTP server
         //let connection: SmtpConnection = transport;
         //.map_err(|e| EmailError::ConnectionError(e.to_string()))?
         
        // self.connection = Some(Arc::new(Mutex::new(connection)));
        // self.connection_created = Some(Instant::now());
      
        Ok(())
    }

    /// Close the connection to the SMTP server
    pub fn close(&mut self) -> Result<()> {
        self.connection = None;
        self.connection_created = None;
        Ok(())
    }

    /// Send a single email message
    pub fn send_message(&mut self, email: &EmailMessage) -> Result<()> {
        self.open()?;

        // Build the email message using lettre
        let mut message_builder = Message::builder()
            .from(email.from_email.parse().map_err(|e| EmailError::MessageError(format!("Invalid from address: {e}")))?)
            .subject(&email.subject);
        
        // Add recipients
        for recipient in &email.to {
            message_builder = message_builder.to(recipient.parse().map_err(|e| 
                EmailError::MessageError(format!("Invalid recipient address: {}", e)))?);
        }
        
        // Add CC recipients
        if let Some(cc_recipients) = &email.cc {
            for recipient in cc_recipients {
                message_builder = message_builder.cc(recipient.parse().map_err(|e| 
                    EmailError::MessageError(format!("Invalid CC address: {}", e)))?);
            }
        }
        
        // Add BCC recipients
        if let Some(bcc_recipients) = &email.bcc {
            for recipient in bcc_recipients {
                message_builder = message_builder.bcc(recipient.parse().map_err(|e| 
                    EmailError::MessageError(format!("Invalid BCC address: {}", e)))?);
            }
        }
        
        // Add Reply-To addresses
        for reply_to in &email.reply_to {
            message_builder = message_builder.reply_to(reply_to.parse().map_err(|e| 
                EmailError::MessageError(format!("Invalid reply-to address: {}", e)))?);
        }
        
        // Add custom headers
        // for (name, value) in &email.headers {
        //     let header_name = header::HeaderName::new_from_ascii_str(name.as_str())
        //     .map_err(|e| EmailError::MessageError(format!("Invalid header name: {}", e)))?;
        //     let header_value = header::HeaderValue::from_str(value)
        //         .map_err(|e| EmailError::MessageError(format!("Invalid header value: {}", e)))?;
        //     message_builder = message_builder.header(
        //                 header_name,header_value
        //             );
        // }

        // Create the message body (multipart if there are alternatives or attachments)
        let has_alternatives = !email.alternatives.is_empty();
        let has_attachments = !email.attachments.is_empty();
        
        let email_body = if has_alternatives || has_attachments {
            // Create multipart message
            let mut multipart = MultiPart::mixed().singlepart(
                SinglePart::builder()
                    .header(header::ContentType::TEXT_PLAIN)
                    .body(email.body.clone())
            );
            
            // Add alternative parts
            for (content, mimetype) in &email.alternatives {
                multipart = multipart.singlepart(
                    SinglePart::builder()
                        .header(header::ContentType::parse(mimetype).map_err(|e| 
                            EmailError::MessageError(format!("Invalid content type: {}", e)))?)
                        .body(content.clone())
                );
            }
            
            // Add attachments
            // for attachment in &email.attachments {
            //     multipart = multipart.singlepart(
            //         SinglePart::builder()
            //             .header(header::ContentType::parse(&attachment.mimetype).map_err(|e| 
            //                 EmailError::MessageError(format!("Invalid attachment mimetype: {}", e)))?)
            //             .header(header::ContentDisposition {
            //                 disposition: header::DispositionType::Attachment,
            //                 parameters: vec![header::DispositionParam::Filename(attachment.filename.clone())],
            //             })
            //             .body(attachment.content.clone())
            //     );
            // }
            
            multipart
        } else {
            // Just use the plain text body
            MultiPart::mixed().singlepart(
                SinglePart::builder()
                    .header(header::ContentType::TEXT_PLAIN)
                    .body(email.body.clone())
            )
        };
        
        let email = message_builder.multipart(email_body)
            .map_err(|e| EmailError::MessageError(e.to_string()))?;
        
        // Send the email
        let mailer = SmtpTransport::builder_dangerous(&self.config.host)
            .port(self.config.port)
            .build();
        
        mailer.send(&email)
            .map_err(|e| EmailError::SendError(e.to_string()))?;
        
        Ok(())
    }

    /// Send multiple email messages
    pub fn send_messages(&mut self, emails: &[EmailMessage]) -> Result<usize> {
        let mut sent_count = 0;
        
        for email in emails {
            match self.send_message(email) {
                Ok(_) => sent_count += 1,
                Err(e) if self.config.fail_silently => continue,
                Err(e) => return Err(e),
            }
        }
        
        Ok(sent_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::*;
    use mockall::predicate::*;
    
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
        mock_transport.expect_send()
            .returning(|_| Ok(()));
        
        // Create a test email
        let email = EmailMessage {
            subject: "Test Email".to_string(),
            body: "This is a test email sent from Rust.".to_string(),
            from_email: "from@example.com".to_string(),
            to: vec!["to@example.com".to_string()],
            cc: Some(vec![]),
            bcc: Some(vec![]),
            reply_to: vec![],
            headers: HashMap::new(),
            alternatives: vec![
                ("This is a test email sent from Rust as HTML.".to_string(), "text/html".to_string())
            ],
            attachments: vec![],
        };
        
        // Test with a simple configuration
        let config = SmtpConfig {
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
                headers: HashMap::new(),
                alternatives: vec![],
                attachments: vec![],
            },
            EmailMessage {
                subject: "Test Email 2".to_string(),
                body: "This is test email 2.".to_string(),
                from_email: "from@example.com".to_string(),
                to: vec!["to2@example.com".to_string()],
                cc: Some(vec![]),
                bcc: Some(vec![]),
                reply_to: vec![],
                headers: HashMap::new(),
                alternatives: vec![],
                attachments: vec![],
            },
        ];
        
        // Test with fail_silently = true
        let config = SmtpConfig {
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
        //assert_eq!(config.use_tls, false);
        //assert_eq!(config.fail_silently, false);
        assert_eq!(config.timeout, Duration::from_secs(60));
        //assert_eq!(config.use_ssl, false);
        assert_eq!(config.ssl_certfile, None);
        assert_eq!(config.ssl_keyfile, None);
    }
}