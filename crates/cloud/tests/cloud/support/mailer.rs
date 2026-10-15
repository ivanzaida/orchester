use std::time::Duration;

use orchester_cloud::mail::{Email, EmailPurpose, MailFuture, Mailer};
use parking_lot::Mutex;
use tokio::sync::Notify;

use super::remember;

#[derive(Clone, Debug)]
pub struct Sent {
  pub purpose: EmailPurpose,
  pub to: String,
  pub subject: String,
  pub body: String,
}

impl Sent {
  /// The token of the email's link (`…/confirm/<64 hex>` or `…/reset/<64 hex>`), remembered as a secret for the log
  /// check.
  pub fn link_token(&self) -> String {
    let marker = ["/confirm/", "/reset/"]
      .into_iter()
      .find(|marker| self.body.contains(marker))
      .expect("the email carries a link");
    let start = self.body.find(marker).expect("the link") + marker.len();
    let token = self.body[start..start + 64].to_owned();
    remember(&token);
    token
  }
}

/// The tests' mailer: keeps every email for the test to read.
#[derive(Default)]
pub struct RecordingMailer {
  sent: Mutex<Vec<Sent>>,
  arrived: Notify,
}

impl Mailer for RecordingMailer {
  fn send(&self, email: Email) -> MailFuture<'_> {
    self.sent.lock().push(Sent {
      purpose: email.purpose(),
      to: email.to().to_owned(),
      subject: email.subject().to_owned(),
      body: email.body().to_owned(),
    });
    self.arrived.notify_waiters();
    Box::pin(async { Ok(()) })
  }
}

impl RecordingMailer {
  /// The `index`th email (from 1) to `to`, waiting for it to be sent in the background.
  pub async fn nth(&self, to: &str, index: usize) -> Sent {
    tokio::time::timeout(Duration::from_secs(10), async {
      loop {
        let arrived = self.arrived.notified();
        if let Some(sent) = self.sent.lock().iter().filter(|sent| sent.to == to).nth(index - 1) {
          return sent.clone();
        }
        arrived.await;
      }
    })
    .await
    .expect("the email arrives")
  }

  pub fn count(&self, to: &str) -> usize {
    self.sent.lock().iter().filter(|sent| sent.to == to).count()
  }
}
