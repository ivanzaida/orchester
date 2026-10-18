mod admin;
mod admin_browsers;
mod admin_sign_in;
mod auth;
mod cloud_join;
mod code_join;

use std::sync::{Arc, atomic::AtomicI64};

pub(crate) use cloud_join::CloudJoined;
use orchester_api::{CertificateInfo, CertificateMode};

use crate::{
  clock::Clock,
  config::{Config, TlsMode},
  db::Database,
  error::ServerError,
  presented::Presented,
  rate_limit::JoinLimits,
  server_keys::ServerKeys,
  tls::Tls,
};

#[derive(Clone)]
pub struct Server {
  pub(crate) inner: Arc<State>,
}

pub(crate) struct State {
  pub config: Config,
  pub clock: Arc<dyn Clock>,
  pub db: Arc<Database>,
  pub keys: ServerKeys,
  /// The server's own TLS; `None` when it serves plain HTTP behind a proxy.
  pub tls: Option<Tls>,
  /// The certificate key proofs name: `tls`'s, or the proxy's.
  pub presented: Presented,
  pub limits: JoinLimits,
  pub sharing_limits: crate::sharing::limits::SharingLimits,
  pub event_prune_at: AtomicI64,
}

impl Server {
  /// Opens the server's data, migrating it, and its certificate, generating one on the first start.
  pub async fn open(config: Config, clock: Arc<dyn Clock>) -> Result<Self, ServerError> {
    config.validate()?;
    let now = clock.now_ms();
    let opened = config.clone();
    let (db, keys, tls) = tokio::task::spawn_blocking(move || {
      let db = Database::open(&opened.data_dir, now)?;
      let keys = db.read_now(ServerKeys::load)?;
      let tls = crate::tls::load(&opened, now)?;
      Ok::<_, ServerError>((db, keys, tls))
    })
    .await
    .map_err(|_| ServerError::Worker)??;
    let presented = Presented::new(&config, tls.as_ref().map(|tls| tls.pin), clock.clone())?;
    Ok(Self {
      inner: Arc::new(State {
        config,
        clock,
        db: Arc::new(db),
        keys,
        tls,
        presented,
        limits: JoinLimits::default(),
        sharing_limits: crate::sharing::limits::SharingLimits::default(),
        event_prune_at: AtomicI64::new(i64::MIN),
      }),
    })
  }

  pub(crate) fn now(&self) -> i64 {
    self.inner.clock.now_ms()
  }

  /// A write of accounts, codes, invites or settings, at the time its transaction starts. Unlike a sharing write it
  /// does not prune events.
  pub(crate) async fn account_write<T: Send + 'static>(
    &self,
    operation: impl FnOnce(&rusqlite::Connection, i64) -> Result<T, ServerError> + Send + 'static,
  ) -> Result<T, ServerError> {
    let clock = self.inner.clock.clone();
    self.inner.db.write(move |db| operation(db, clock.now_ms())).await
  }

  /// The check code of the certificate the server presents; `None` when it serves plain HTTP.
  pub fn check_code(&self) -> Option<String> {
    self.inner.tls.as_ref().map(|tls| tls.pin.check_code())
  }

  /// Where the private listener for the server admin app's API calls is bound, when it is on.
  pub fn admin_api_listen(&self) -> Option<std::net::SocketAddr> {
    self.inner.config.admin_api_listen
  }

  /// The certificate the server presents, as a browser compares it; `None` when it serves plain HTTP.
  pub fn certificate(&self) -> Option<CertificateInfo> {
    self.inner.tls.as_ref().map(|tls| CertificateInfo {
      check_code: tls.pin.check_code(),
      sha256: tls.pin.to_hex(),
      mode: match self.inner.config.tls {
        TlsMode::Pem => CertificateMode::Pem,
        TlsMode::SelfSigned | TlsMode::Off => CertificateMode::SelfSigned,
      },
    })
  }
}
