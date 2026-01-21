---
title: Sending Emails
---

Cot provides a unified interface for sending emails, allowing you to switch between different email backends (like SMTP, Memory, or Console) easily. This is powered by the popular [`lettre`](https://crates.io/crates/lettre) crate.

## Configuration

To use the email system, you need to enable the `email` feature in `cot` and configure it.

### Enabling the Feature

In your `Cargo.toml`:

```toml
[dependencies]
cot = { version = "0.5", features = ["email"] }
```

### Configuration via TOML

Configure the email transport in your `config/*.toml` files:

```toml
[email]
from = "no-reply@example.com" # Default sender address (optional)

[email.transport]
type = "smtp" # Options: "smtp", "console"
url = "smtp://user:password@localhost:587" # For SMTP
mechanism = "plain" # or "login", "xoauth2"
```

For development, you might want to use the `console` transport, which prints emails to stdout:

```toml
[email.transport]
type = "console"
```

## Sending Emails

You can access the email sender by using the `Email` extractor.

```rust
use cot::common_types::Email;
use cot::email::{Email as EmailService, EmailMessage};
use cot::html::Html;

async fn send_welcome_email(email_sender: EmailService) -> cot::Result<Html> {
    let message = EmailMessage::builder()
        .from(Email::try_from("no-reply@example.com").unwrap())
        .to(vec![Email::try_from("user@example.com").unwrap()])
        .subject("Welcome to Cot!")
        .body("Hello, welcome to our service!")
        .build()?;

    email_sender.send(message).await?;

    Ok(Html::new("Email sent!"))
}
```

## Email Message Builder

The `EmailMessage::builder()` provides a fluent interface to construct emails. It supports:

- **From/To/Cc/Bcc**: Set recipients and sender.
- **Subject**: Set the email subject.
- **Body**: Set the plain text body.
- **Html**: Set the HTML body (if supported).
- **Attachments**: Add file attachments.

See the [API reference](https://docs.rs/cot/0.5/cot/email/struct.EmailMessageBuilder.html)
for more details.
