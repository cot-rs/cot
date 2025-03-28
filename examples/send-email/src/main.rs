use cot::email::{EmailBackend, SmtpConfig, SmtpEmailBackend,SmtpTransportMode};
use lettre::message::header;
use lettre::message::{Message, MultiPart,SinglePart};
/// This example demonstrates how to send an email using the `cot` library with a multi-part message
/// containing both plain text and HTML content.
/// It uses the `lettre` library for email transport and `MailHog` for testing.
/// Make sure you have MailHog running on port 1025 before executing this example.
/// You can run MailHog using Docker with the following command:
/// `docker run -p 1025:1025 -p 8025:8025 mailhog/mailhog`
/// After running the example, you can check the MailHog web interface at `http://localhost:8025`
/// to see the sent email.
fn main() {
    let parts = MultiPart::related()
        .singlepart(
            SinglePart::builder()
                .header(header::ContentType::TEXT_PLAIN)
                .body("This is a test email sent from Rust.".to_string()),
        )
        .singlepart(
            SinglePart::builder()
                .header(header::ContentType::TEXT_HTML)
                .body("This is a test email sent from examples as HTML.".to_string()),
        );
    // Create a test email
    let email = Message::builder()
        .subject("Test Email".to_string())
        .from("<from@cotexample.com>".parse().unwrap())
        .to("<to@cotexample.com>".parse().unwrap())
        .cc("<cc@cotexample.com>".parse().unwrap())
        .bcc("<bcc@cotexample.com>".parse().unwrap())
        .reply_to("<replyto@cotexample.com>".parse().unwrap())
        .multipart(parts)
        .unwrap();
    // Get the port it's running on
    let port = 1025; //Mailhog default smtp port
                     // Create a new email backend
    let config = SmtpConfig {
        mode: SmtpTransportMode::Unencrypted("localhost".to_string()),
        port: Some(port),
        ..Default::default()
    };
    let mut backend = SmtpEmailBackend::new(config);
    let _ = backend.send_message(&email);
}
