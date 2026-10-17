//! The certificate the server's connections present, which it signs into its key proofs (`http::cloud`,
//! `docs/architecture/server.md`, "Codes"): its own; with TLS off its proxy's, as configured or as the server reads it
//! at its own public address (`proxy_certificate = "auto"`); or none.

mod auto;

use std::sync::Arc;

use orchester_api::{certificate::CertificatePin, client::ServerAddress};
use tokio::task::JoinHandle;

use self::auto::AutoCertificate;
use crate::{
  clock::Clock,
  config::{Config, ProxyCertificate},
  error::ServerError,
};

pub(crate) enum Presented {
  /// Plain HTTP with no proxy certificate configured: no app could compare one, so a proof names none.
  None,
  /// The server's own, or the one its proxy was configured with.
  Fixed(CertificatePin),
  /// Its proxy's, read at the server's own public address.
  Auto(Arc<AutoCertificate>),
}

/// What a key proof names.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Named {
  pub current: Option<CertificatePin>,
  /// The proxy's certificate before its last renewal, for a short while after the server saw it.
  pub previous: Option<CertificatePin>,
}

impl Presented {
  /// What the server presents: `own` with its own TLS, else what `proxy_certificate` says.
  pub fn new(config: &Config, own: Option<CertificatePin>, clock: Arc<dyn Clock>) -> Result<Self, ServerError> {
    if let Some(pin) = own {
      return Ok(Self::Fixed(pin));
    }
    Ok(match config.proxy_certificate()? {
      None => Self::None,
      Some(ProxyCertificate::Fixed(pin)) => Self::Fixed(pin),
      Some(ProxyCertificate::Auto) => {
        let address = config
          .address
          .as_deref()
          .and_then(|address| ServerAddress::parse(address).ok())
          .filter(ServerAddress::is_https)
          .ok_or(ServerError::Configuration(
            "PROXY_CERTIFICATE = auto needs an https:// ADDRESS",
          ))?;
        Self::Auto(Arc::new(AutoCertificate::new(address, clock)))
      }
    })
  }

  /// What a key proof names for an app whose connections last presented `seen` (`PRESENTED_CERTIFICATE`); with
  /// `auto`, after reading the proxy's certificate again when that is due.
  pub async fn for_proof(&self, seen: Option<CertificatePin>) -> Named {
    match self {
      Self::None => Named::default(),
      Self::Fixed(pin) => Named {
        current: Some(*pin),
        previous: None,
      },
      Self::Auto(auto) => auto.for_proof(seen).await,
    }
  }

  /// With `auto`, reads the proxy's certificate now and then again every hour, or 30 seconds after a read that failed,
  /// until the caller aborts the task it returns, when it stops serving; `None` otherwise.
  pub fn start_schedule(&self) -> Option<JoinHandle<()>> {
    let Self::Auto(auto) = self else {
      return None;
    };
    let auto = auto.clone();
    Some(tokio::spawn(async move {
      loop {
        let next = auto.refresh().await;
        tokio::time::sleep(next).await;
      }
    }))
  }
}
