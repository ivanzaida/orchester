use std::time::Duration;

use lettre::{
  AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
  message::{Mailbox, header::ContentType},
};
use orchester_api::Credential;

use super::{Email, MailFuture, Mailer};
use crate::error::CloudError;

/// Sends through the person's SMTP relay; TLS through rustls with the operating system's trust store.
pub struct SmtpMailer {
  transport: AsyncSmtpTransport<Tokio1Executor>,
  from: Mailbox,
}

impl SmtpMailer {
  /// `url` is `SMTP_URL` (`Config::smtp_url`), `from` is `SMTP_FROM`.
  pub fn new(url: &Credential, from: &str) -> Result<Self, CloudError> {
    let from = from
      .parse()
      .map_err(|_| CloudError::Configuration("ORCHESTER_CLOUD_SMTP_FROM is not an email address"))?;
    // The parser's error can quote the URL, which holds the relay's password.
    let transport = AsyncSmtpTransport::<Tokio1Executor>::from_url(url.expose())
      .map_err(|_| CloudError::Configuration("ORCHESTER_CLOUD_SMTP_URL is not an smtp:// or smtps:// URL"))?
      .timeout(Some(Duration::from_secs(30)))
      .build();
    Ok(Self { transport, from })
  }
}

impl Mailer for SmtpMailer {
  fn send(&self, email: Email) -> MailFuture<'_> {
    Box::pin(async move {
      // lettre's errors can quote addresses and server replies; the log keeps only the email's purpose.
      let to: Mailbox = email.to.parse().map_err(|_| CloudError::Mail)?;
      let message = Message::builder()
        .from(self.from.clone())
        .to(to)
        .subject(email.subject)
        .header(ContentType::TEXT_PLAIN)
        .body(email.body)
        .map_err(|_| CloudError::Mail)?;
      self.transport.send(message).await.map_err(|_| CloudError::Mail)?;
      Ok(())
    })
  }
}
