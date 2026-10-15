//! A TLS client verifier for tests that trusts the one certificate whose check code it was given.

use std::sync::Arc;

use orchester_api::certificate::CertificatePin;
use tokio_rustls::rustls::{
  CertificateError, DigitallySignedStruct, Error, SignatureScheme,
  client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
  crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature},
  pki_types::{CertificateDer, ServerName, UnixTime},
};

#[derive(Debug)]
pub struct Pinned {
  check_code: String,
  provider: Arc<CryptoProvider>,
}

impl Pinned {
  pub fn new(check_code: String) -> Self {
    Self {
      check_code,
      provider: Arc::new(tokio_rustls::rustls::crypto::ring::default_provider()),
    }
  }
}

impl ServerCertVerifier for Pinned {
  fn verify_server_cert(
    &self,
    end_entity: &CertificateDer<'_>,
    _intermediates: &[CertificateDer<'_>],
    _server_name: &ServerName<'_>,
    _ocsp: &[u8],
    _now: UnixTime,
  ) -> Result<ServerCertVerified, Error> {
    if CertificatePin::of(end_entity).check_code() == self.check_code {
      Ok(ServerCertVerified::assertion())
    } else {
      Err(Error::InvalidCertificate(
        CertificateError::ApplicationVerificationFailure,
      ))
    }
  }

  fn verify_tls12_signature(
    &self,
    message: &[u8],
    certificate: &CertificateDer<'_>,
    signature: &DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, Error> {
    verify_tls12_signature(
      message,
      certificate,
      signature,
      &self.provider.signature_verification_algorithms,
    )
  }

  fn verify_tls13_signature(
    &self,
    message: &[u8],
    certificate: &CertificateDer<'_>,
    signature: &DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, Error> {
    verify_tls13_signature(
      message,
      certificate,
      signature,
      &self.provider.signature_verification_algorithms,
    )
  }

  fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
    self.provider.signature_verification_algorithms.supported_schemes()
  }
}
