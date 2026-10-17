//! `proxy_certificate = "auto"`: the certificate the proxy in front presents, read by connecting to the server's own
//! public `address`, as an app does, over TLS the operating system's trust store checks, chain and name, and asking it
//! `GET /server` (`docs/server-hosting.md`, "The server admin"). So a renewal at the proxy needs no change here.
//!
//! The server reads it on a schedule, again when a proof is made and the last read is older than [`FRESH_FOR_MS`], and
//! when an app says its connection presented one the server has not seen (the proxy renewed it since). After a renewal
//! the one before is still named, as `previous_certificate`, for [`GRACE_MS`]: an app whose connection was made just
//! before the switch still presents it. A read that fails (no route, no answer, a certificate the store does not trust
//! for that name) forgets both, so proofs name none and no app enters a code until a read succeeds again.

use std::{sync::Arc, time::Duration};

use orchester_api::{
  certificate::CertificatePin,
  client::{Client, ClientError, PlainHttp, ServerAddress},
};
use parking_lot::Mutex;

use super::Named;
use crate::clock::Clock;

/// How often the server reads it on its own.
pub(super) const REFRESH_EVERY: Duration = Duration::from_secs(60 * 60);
const MINUTE_MS: i64 = 60 * 1000;
/// How old the last read may be when a proof is made before it is read again first.
const FRESH_FOR_MS: i64 = 10 * MINUTE_MS;
/// How long after the server saw a renewal a proof still names the certificate before it.
const GRACE_MS: i64 = 60 * MINUTE_MS;
/// The least time between two reads a proof asks for, after a failure or for a certificate not seen yet, so requests
/// cannot make the server connect out at will.
const RETRY_AFTER_MS: i64 = MINUTE_MS / 2;
/// How often a failing read is logged.
const LOG_EVERY_MS: i64 = 10 * MINUTE_MS;
/// How long one read may take, which a proof waiting for it waits at most.
const READ_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) struct AutoCertificate {
  address: ServerAddress,
  clock: Arc<dyn Clock>,
  seen: Mutex<Seen>,
  /// Held for the length of one read, so reads that fall due together make one connection.
  reading: tokio::sync::Mutex<()>,
}

impl AutoCertificate {
  pub fn new(address: ServerAddress, clock: Arc<dyn Clock>) -> Self {
    Self {
      address,
      clock,
      seen: Mutex::new(Seen::default()),
      reading: tokio::sync::Mutex::new(()),
    }
  }

  /// What a proof names for an app whose connections last presented `presented`, read again first when due. A read
  /// already running is waited for, so a proof made while it runs (the first one, at start) names what it finds.
  pub async fn for_proof(&self, presented: Option<CertificatePin>) -> Named {
    self.read(|seen, now| seen.due(now, presented)).await;
    self.seen.lock().named(self.clock.now_ms())
  }

  /// Reads it now: the server's schedule.
  pub async fn refresh(&self) {
    self.read(|_, _| true).await;
  }

  /// Reads it if `due` still says so once no other read runs.
  async fn read(&self, due: impl Fn(&Seen, i64) -> bool) {
    let _reading = self.reading.lock().await;
    let started = self.clock.now_ms();
    if !due(&self.seen.lock(), started) {
      return;
    }
    self.seen.lock().attempted_at = Some(started);
    let read = read_presented(&self.address).await;
    self.seen.lock().record(read, self.clock.now_ms());
  }
}

/// The leaf `address` presents, its chain and name checked by the trust store, on a connection that answered
/// `GET /server` as an Orchester server. A new client makes a new connection, so the leaf is the proxy's current one.
async fn read_presented(address: &ServerAddress) -> Result<CertificatePin, ReadError> {
  let client = Client::new(address.clone(), PlainHttp::NotAllowed)?;
  tokio::time::timeout(READ_TIMEOUT, client.server_info())
    .await
    .map_err(|_| ReadError::TimedOut)??;
  client.certificate().ok_or(ReadError::NoCertificate)
}

/// Why a read found no certificate. Its text is for the host's log and holds no secret: only the reason, the address
/// is the server's configured one.
#[derive(Debug, thiserror::Error)]
enum ReadError {
  #[error(transparent)]
  Client(#[from] ClientError),
  #[error("no answer within {} seconds", READ_TIMEOUT.as_secs())]
  TimedOut,
  #[error("the connection presented no certificate")]
  NoCertificate,
}

/// What the reads found.
#[derive(Default)]
struct Seen {
  /// The certificate the last read found, and when; `None` before one succeeded and after one failed.
  current: Option<(CertificatePin, i64)>,
  /// The certificate a renewal replaced, and when the server saw that.
  previous: Option<(CertificatePin, i64)>,
  /// When the last read started.
  attempted_at: Option<i64>,
  /// When a failed read was last logged; `None` after a read succeeded.
  failure_logged_at: Option<i64>,
}

impl Seen {
  /// Whether a proof for an app whose connections presented `presented` should wait for a read first.
  fn due(&self, now: i64, presented: Option<CertificatePin>) -> bool {
    let may_retry = self.attempted_at.is_none_or(|at| now - at >= RETRY_AFTER_MS);
    match self.current {
      None => may_retry,
      Some((_, read_at)) if now - read_at >= FRESH_FOR_MS => true,
      Some(_) => may_retry && presented.is_some_and(|pin| !self.knows(pin, now)),
    }
  }

  fn previous(&self, now: i64) -> Option<CertificatePin> {
    self
      .previous
      .filter(|(_, replaced_at)| now - replaced_at < GRACE_MS)
      .map(|(pin, _)| pin)
  }

  fn knows(&self, pin: CertificatePin, now: i64) -> bool {
    self.current.is_some_and(|(current, _)| current == pin) || self.previous(now) == Some(pin)
  }

  fn named(&self, now: i64) -> Named {
    match self.current {
      None => Named::default(),
      Some((current, _)) => Named {
        current: Some(current),
        previous: self.previous(now),
      },
    }
  }

  fn record(&mut self, read: Result<CertificatePin, ReadError>, now: i64) {
    let error = match read {
      Ok(pin) => {
        match self.current.replace((pin, now)) {
          Some((before, _)) if before != pin => {
            self.previous = Some((before, now));
            tracing::info!(sha256 = %pin.to_hex(), "the proxy presents a renewed certificate; key proofs name it");
          }
          Some(_) => {}
          None => tracing::info!(sha256 = %pin.to_hex(), "read the certificate the proxy presents at ADDRESS"),
        }
        self.failure_logged_at = None;
        return;
      }
      Err(error) => error,
    };
    // Fail closed: what the route showed before may no longer be what apps are shown.
    self.current = None;
    self.previous = None;
    if self.failure_logged_at.is_none_or(|at| now - at >= LOG_EVERY_MS) {
      self.failure_logged_at = Some(now);
      tracing::warn!(
        %error,
        "cannot read the certificate the proxy presents at ADDRESS (proxy_certificate = auto): key proofs name none, \
         so apps enter no codes until a read succeeds"
      );
    }
  }
}
