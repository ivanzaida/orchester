//! Accounts on Orchester Cloud: sign-up with email confirmation, sign-in from a device, devices, password change and
//! reset (`docs/architecture/server.md`, "Orchester Cloud"; the endpoints in `docs/cloud-api.md`).

mod confirm;
mod delete;
mod devices;
mod input;
mod inspect;
mod links;
mod password;
mod resend;
mod reset;
mod rows;
mod sign_in;
mod sign_up;
mod two_factor;

use std::{sync::Arc, time::Duration};

use orchester_api::identity::PublicKey;
pub(crate) use rows::Authenticated;
pub(crate) use sign_in::SignInAnswer;
use two_factor::{RecoveryKey, require_session_when_on};
pub(crate) use two_factor::{
  require_session as require_two_factor_session, session as two_factor_session, sweep as sweep_two_factor,
};

use crate::{
  clock::Clock,
  config::Config,
  db::Database,
  error::CloudError,
  keys::{KeySealing, open_root},
  limits::Limits,
  mail::{Email, Mailer, Messages},
  passwords::Passwords,
  people::People,
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
  /// The public half of the cloud's root key, which team servers pin.
  pub root: PublicKey,
  pub people: People,
  pub recovery: RecoveryKey,
}

impl Cloud {
  /// Connects to the database and applies its migrations, refusing a database newer than this binary; loads the root
  /// key, or makes it at the first start; then brings the signing keys and the key set up to date, so `GET /keys`
  /// answers from the first request. A root the key-encryption key does not open stops the start.
  pub async fn open(config: Config, clock: Arc<dyn Clock>, mailer: Arc<dyn Mailer>) -> Result<Self, CloudError> {
    config.validate()?;
    let sealing = KeySealing::new(&config.key_encryption_key)?;
    let people = People::new(&config.key_encryption_key)?;
    let recovery = RecoveryKey::new(&config.key_encryption_key)?;
    let db = Database::open(&config.database_url, clock.now_ms()).await?;
    let root = open_root(&db, &sealing, clock.now_ms()).await?;
    // Both are public: the fingerprint team servers show, and the key their admins pin again after a new root.
    tracing::info!(root_public_key = %root.encoded(), root_fingerprint = %root.fingerprint(), "root key ready");
    let messages = Messages::new(config.public_url.clone())?;
    let cloud = Self {
      inner: Arc::new(State {
        config,
        clock,
        db,
        mailer,
        messages,
        passwords: Passwords::new(4, Duration::from_secs(10)),
        limits: Limits::new(),
        sealing,
        root,
        people,
        recovery,
      }),
    };
    cloud.keep_keys().await?;
    Ok(cloud)
  }

  pub(crate) async fn client(&self) -> Result<deadpool_postgres::Object, CloudError> {
    self.inner.db.client().await
  }

  pub(crate) fn now(&self) -> i64 {
    self.inner.clock.now_ms()
  }

  /// Sends in the background, so an answer takes as long whether an email went out or not, and does not depend on
  /// the relay. A failure is logged with the email's purpose only.
  pub(crate) fn send_later(&self, email: Email) {
    let mailer = Arc::clone(&self.inner.mailer);
    tokio::spawn(async move {
      let purpose = email.purpose().as_str();
      if let Err(error) = mailer.send(email).await {
        tracing::error!(error_kind = ?error, purpose, "cannot send email");
      }
    });
  }
}
