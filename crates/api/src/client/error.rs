//! What can go wrong between the desktop and a server, typed so the desktop says it in its own words
//! (`ApiError::message` is English for logs). No variant carries a request or response body,
//! a header or a token.

use std::{error::Error as StdError, io};

use thiserror::Error;

use super::AddressError;
use crate::{
  ErrorCode, SharingErrorDetails,
  cloud::{CloudErrorCode, SessionEnd},
};

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ClientError {
  #[error(transparent)]
  Address(#[from] AddressError),
  /// Plain HTTP to another computer without the person's opt-in for this server.
  #[error("plain HTTP to another computer needs the person's opt-in")]
  PlainHttpRefused,
  /// The address answered with a redirect, which is never followed; `location` is its target.
  #[error("the server answered with a redirect to {location:?}")]
  Redirect { location: Option<String> },
  /// No answer within the connect or request deadline.
  #[error("the server did not answer in time")]
  TimedOut,
  /// The connection could not be made: no such host, refused, or broken off.
  #[error("the server cannot be reached")]
  Unreachable,
  /// The operating system does not trust the server's certificate. There is no way to skip this.
  #[error("the server's certificate is not trusted")]
  UntrustedCertificate,
  /// Something answered that does not speak the server's API, with this HTTP status.
  #[error("not an Orchester server (HTTP {status})")]
  NotOrchester { status: u16 },
  /// The server refused the request with one of its error codes; `retry_after` comes with
  /// `rate_limited`, in seconds, and `details` with the refusals of shared projects that say more:
  /// the current task of a conflict, the holder of a claim, the owners of a project.
  #[error("the server refused the request: {code:?} (HTTP {status})")]
  Api {
    status: u16,
    code: ErrorCode,
    retry_after: Option<u64>,
    details: Option<Box<SharingErrorDetails>>,
  },
  /// Orchester Cloud refused the request with one of its error codes; `retry_after` comes with `rate_limited`, and
  /// `ended` with an `unauthorized` for a token the cloud ended: why, and when (milliseconds).
  #[error("Orchester Cloud refused the request: {code:?} (HTTP {status})")]
  Cloud {
    status: u16,
    code: CloudErrorCode,
    retry_after: Option<u64>,
    ended: Option<(SessionEnd, Option<i64>)>,
  },
  /// The HTTP client could not be set up, e.g. the operating system's certificate store failed.
  #[error("the HTTP client cannot be set up")]
  Setup,
}

impl ClientError {
  /// The server's error code, when it refused the request with one.
  pub fn code(&self) -> Option<ErrorCode> {
    match self {
      Self::Api { code, .. } => Some(*code),
      _ => None,
    }
  }

  /// What a refusal of a shared project's request says beyond its code.
  pub fn details(&self) -> Option<&SharingErrorDetails> {
    match self {
      Self::Api { details, .. } => details.as_deref(),
      _ => None,
    }
  }

  /// Whether the request could not reach the server at all: it may work again later.
  pub fn is_connection(&self) -> bool {
    matches!(self, Self::TimedOut | Self::Unreachable)
  }

  /// A transport failure of reqwest, classified. Its text is not kept: it names the URL and
  /// lower layers' details, never needed to tell the person what happened.
  pub(super) fn transport(error: &reqwest::Error) -> Self {
    if error.is_builder() {
      return Self::Setup;
    }
    if certificate_refused(error) {
      return Self::UntrustedCertificate;
    }
    if error.is_timeout() {
      return Self::TimedOut;
    }
    Self::Unreachable
  }
}

/// Whether rustls refused the server's certificate somewhere down `error`'s sources. rustls'
/// error reaches reqwest inside nested `io::Error`s, whose own `source` skips the error they wrap.
fn certificate_refused(error: &(dyn StdError + 'static)) -> bool {
  let mut pending = vec![error];
  while let Some(error) = pending.pop() {
    if is_certificate(error) {
      return true;
    }
    if let Some(inner) = error.downcast_ref::<io::Error>().and_then(io::Error::get_ref) {
      pending.push(inner);
    }
    pending.extend(error.source());
  }
  false
}

fn is_certificate(error: &(dyn StdError + 'static)) -> bool {
  matches!(
    error.downcast_ref::<rustls::Error>(),
    Some(rustls::Error::InvalidCertificate(_) | rustls::Error::InvalidCertRevocationList(_))
  )
}
