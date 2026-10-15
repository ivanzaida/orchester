//! *Connect a server* (`docs/architecture/server.md`, row 39, "Transport and certificates"): what answers at an
//! address before anything is sent to it. A server whose certificate the operating system trusts needs no comparison;
//! one that made its own certificate shows its check code, which only the person confirms
//! ([`super::trust::PresentedCertificate::confirm`]); plain HTTP to another computer needs the person's opt-in first.
//! Only `GET /server` is asked: no code, token or name reaches the server before the person decides.

use orchester_api::{
  ServerInfo,
  client::{Client, ClientError, PlainHttp, ServerAddress},
};

use super::{Servers, trust::PresentedCertificate};

/// What answered at an address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reached {
  pub info: ServerInfo,
  pub trust: Reaching,
}

/// How this computer would trust the server that answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reaching {
  /// The operating system trusts its certificate (a public or the team's own authority): nothing to compare.
  System,
  /// It presents a certificate of its own: the person compares its check code before anything is sent.
  OwnCertificate(PresentedCertificate),
  /// Plain HTTP, to this computer or after the person's opt-in.
  Plain(PlainHttp),
}

impl Servers {
  /// Asks the server at `address` what it is (`GET /server`), trusting nothing yet. Plain HTTP to another computer
  /// without `plain_http` allowed is refused before anything is sent ([`ClientError::PlainHttpRefused`]).
  pub async fn reach(&self, address: &ServerAddress, plain_http: PlainHttp) -> Result<Reached, ClientError> {
    if !address.is_https() {
      let info = Client::new(address.clone(), plain_http)?.server_info().await?;
      return Ok(Reached {
        info,
        trust: Reaching::Plain(plain_http),
      });
    }
    match Client::new(address.clone(), PlainHttp::NotAllowed)?.server_info().await {
      Ok(info) => Ok(Reached {
        info,
        trust: Reaching::System,
      }),
      Err(ClientError::UntrustedCertificate) => {
        let presented = PresentedCertificate::read(address).await?;
        let info = presented.server_info(address).await?;
        Ok(Reached {
          info,
          trust: Reaching::OwnCertificate(presented),
        })
      }
      Err(error) => Err(error),
    }
  }
}
