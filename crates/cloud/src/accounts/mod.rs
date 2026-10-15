//! Accounts on Orchester Cloud: sign-up with email confirmation, sign-in from a device, devices, password change and
//! reset (`docs/architecture/server.md`, "Orchester Cloud"; the endpoints in `docs/cloud-api.md`).

mod devices;
mod input;
mod links;
mod password;
mod reset;
mod rows;
mod sign_in;
mod sign_up;

use std::sync::Arc;

pub(crate) use rows::Authenticated;

use crate::{
  clock::Clock,
  config::Config,
  db::Database,
  error::CloudError,
  keys::KeySealing,
  limits::Limits,
  mail::{Email, Mailer, Messages},
  passwords::Passwords,
};

/// The running cloud: what the HTTP handlers and the host commands act on.
#[derive(Clone)]
pub struct Cloud {
  pub(crate) inner: Arc<State>,
}

pub(crate) struct State {
  pub config: Config,
  pub clock: Arc<dyn Clock>,
  pub db: Database,
  pub mailer: Arc<dyn Mailer>,
  pub messages: Messages,
  pub passwords: Passwords,
  pub limits: Limits,
  pub sealing: KeySealing,
}

impl Cloud {
  /// Connects to the database and applies its migrations; refuses a database newer than this binary.
  pub async fn open(config: Config, clock: Arc<dyn Clock>, mailer: Arc<dyn Mailer>) -> Result<Self, CloudError> {
    config.validate()?;
    let sealing = KeySealing::new(&config.key_encryption_key)?;
    let db = Database::open(&config.database_url, clock.now_ms()).await?;
    let messages = Messages::new(config.public_url.clone());
    Ok(Self {
      inner: Arc::new(State {
        config,
        clock,
        db,
        mailer,
        messages,
        passwords: Passwords::new(),
        limits: Limits::new(),
        sealing,
      }),
    })
  }

  pub(crate) fn now(&self) -> i64 {
    self.inner.clock.now_ms()
  }

  /// Sends in the background, so an answer takes as long whether an email went out or not, and does not depend on
  /// the relay. A failure is logged with the email's purpose only.
  fn send_later(&self, email: Email) {
    let mailer = Arc::clone(&self.inner.mailer);
    tokio::spawn(async move {
      let purpose = email.purpose().as_str();
      if let Err(error) = mailer.send(email).await {
        tracing::error!(error_kind = ?error, purpose, "cannot send email");
      }
    });
  }
}
