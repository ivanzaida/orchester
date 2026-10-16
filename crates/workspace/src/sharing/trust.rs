//! How this computer trusts a team server (`docs/architecture/server.md`, row 39, "Transport and certificates"): the
//! server's own certificate, pinned only after the person compared its check code with the one on the server's console
//! or the inviter's screen; the operating system's trust store for a certificate from a CA; or plain HTTP to this
//! computer or after the person's opt-in. A pin cannot be made any other way than by that confirmation.

use orchester_api::{
  certificate::CertificatePin,
  client::{Client, ClientError, PlainHttp, ServerAddress},
  cloud::CloudServer,
};
use orchester_store::{StoreError, StoredServer};

use super::SharingError;

/// The certificate a server presented before anything was sent to it, for the person to compare its check code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentedCertificate {
  pin: CertificatePin,
}

impl PresentedCertificate {
  /// Reads the certificate the team server at the `https` `address` presents, without trusting it: no request, code
  /// or token reaches the server.
  pub async fn read(address: &ServerAddress) -> Result<Self, ClientError> {
    Client::presented_certificate(address).await.map(|pin| Self { pin })
  }

  /// What the server at `address` says it is (`GET /server`), over a connection to exactly this certificate, before
  /// the person compared it: nothing but the question is sent.
  pub async fn server_info(&self, address: &ServerAddress) -> Result<orchester_api::ServerInfo, ClientError> {
    Client::pinned(address.clone(), self.pin)?.server_info().await
  }

  /// What the person compares: `3F7A-92C1-KD8M-QX4T`.
  pub fn check_code(&self) -> String {
    self.pin.check_code()
  }

  /// The person confirmed that the check code matches the server's: from now on this computer trusts exactly this
  /// certificate there.
  pub fn confirm(self) -> Trust {
    Trust::Pinned(ConfirmedPin(self.pin))
  }
}

/// A certificate pin the person confirmed, or one Orchester Cloud named with the server (row 39: a cloud invite carries
/// the fingerprint, so the person compares nothing); there is no other way to make one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConfirmedPin(CertificatePin);

impl ConfirmedPin {
  pub fn pin(self) -> CertificatePin {
    self.0
  }
}

/// How this computer trusts a server it connects to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trust {
  /// Exactly the certificate the person confirmed ([`PresentedCertificate::confirm`]).
  Pinned(ConfirmedPin),
  /// The operating system's trust store; plain HTTP only to this computer or with the person's opt-in.
  System(PlainHttp),
}

impl Trust {
  /// The trust of a server as Orchester Cloud names it: its certificate pinned when the cloud has its fingerprint,
  /// the operating system's trust store otherwise.
  pub(super) fn named_by_cloud(server: &CloudServer) -> Result<Self, SharingError> {
    match &server.fingerprint {
      Some(hex) => CertificatePin::parse(hex)
        .map(|pin| Self::Pinned(ConfirmedPin(pin)))
        .ok_or(SharingError::InvalidAnswer("certificate fingerprint")),
      None => Ok(Self::System(PlainHttp::NotAllowed)),
    }
  }

  /// The trust of a server in the list, as stored when it was connected.
  pub(super) fn stored(server: &StoredServer) -> Result<Self, StoreError> {
    match &server.certificate_pin {
      Some(hex) => CertificatePin::parse(hex)
        .map(|pin| Self::Pinned(ConfirmedPin(pin)))
        .ok_or_else(|| StoreError::Corrupt {
          what: "certificate pin",
          detail: "not 64 hex characters".into(),
        }),
      None if server.plain_http_allowed => Ok(Self::System(PlainHttp::Allowed)),
      None => Ok(Self::System(PlainHttp::NotAllowed)),
    }
  }

  /// The pin to store with the server, in hex, and whether plain HTTP is allowed there.
  pub(super) fn columns(self) -> (Option<String>, bool) {
    match self {
      Self::Pinned(pin) => (Some(pin.0.to_hex()), false),
      Self::System(plain_http) => (None, plain_http == PlainHttp::Allowed),
    }
  }

  /// What a refusal of a server trusted this way means: a pinned server that presents another certificate (a new
  /// one, or another server at its address) is said so, with nothing sent to it.
  pub(super) fn refusal(self, error: ClientError) -> SharingError {
    match (self, error) {
      (Self::Pinned(_), ClientError::UntrustedCertificate) => SharingError::CertificateChanged,
      (_, error) => SharingError::Client(error),
    }
  }

  /// The fingerprint (hex) of the certificate the server at `address` presents as this computer trusts it: the pinned
  /// one, or for the trust store the one it presents now, read without sending anything; `None` over plain HTTP.
  pub(super) async fn certificate(self, address: &ServerAddress) -> Result<Option<String>, ClientError> {
    match self {
      Self::Pinned(pin) => Ok(Some(pin.0.to_hex())),
      Self::System(_) if address.is_https() => Client::presented_certificate(address)
        .await
        .map(|pin| Some(pin.to_hex())),
      Self::System(_) => Ok(None),
    }
  }

  pub(super) fn client(self, address: ServerAddress) -> Result<Client, ClientError> {
    match self {
      Self::Pinned(pin) => Client::pinned(address, pin.0),
      Self::System(plain_http) => Client::new(address, plain_http),
    }
  }
}
