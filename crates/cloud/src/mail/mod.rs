//! Email sending, behind the `Mailer` boundary: SMTP through the person's relay in the service
//! (`SmtpMailer`), a recording mailer in the tests.

mod messages;
mod smtp;

use std::{fmt, future::Future, pin::Pin};

pub(crate) use messages::{InviteMail, Messages};
pub use smtp::SmtpMailer;

use crate::error::CloudError;

pub type MailFuture<'a> = Pin<Box<dyn Future<Output = Result<(), CloudError>> + Send + 'a>>;

pub trait Mailer: Send + Sync {
  fn send(&self, email: Email) -> MailFuture<'_>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmailPurpose {
  /// A new account's confirmation link.
  Confirm,
  /// A sign-up with an email that already has an account.
  AlreadyRegistered,
  /// A password reset link.
  Reset,
  /// An invite from a team server.
  Invite,
}

impl EmailPurpose {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Confirm => "confirm",
      Self::AlreadyRegistered => "already_registered",
      Self::Reset => "reset",
      Self::Invite => "invite",
    }
  }
}

/// One plain-text email. Its body can hold a one-time link, so `Debug` shows the purpose only.
pub struct Email {
  purpose: EmailPurpose,
  to: String,
  subject: String,
  body: String,
}

impl Email {
  pub fn purpose(&self) -> EmailPurpose {
    self.purpose
  }

  pub fn to(&self) -> &str {
    &self.to
  }

  pub fn subject(&self) -> &str {
    &self.subject
  }

  pub fn body(&self) -> &str {
    &self.body
  }
}

impl fmt::Debug for Email {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "Email({}, <redacted>)", self.purpose.as_str())
  }
}
