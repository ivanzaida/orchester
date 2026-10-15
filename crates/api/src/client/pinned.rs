//! Trust in exactly one certificate (`docs/architecture/server.md`, "Transport and certificates"): a team server's own
//! self-signed certificate, pinned from a cloud invite's fingerprint or after the person confirmed its check code. The
//! presented leaf certificate must hash to the pin; names, dates and issuers do not matter, and nothing else is
//! accepted. There is no switch that skips the check.

use std::sync::{Arc, OnceLock};

use rustls::{
  CertificateError, DigitallySignedStruct, Error, SignatureScheme,
  client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
  crypto::{CryptoProvider, verify_tls12_signature, verify_tls13_signature},
  pki_types::{CertificateDer, ServerName, UnixTime},
};

use crate::certificate::CertificatePin;

/// What the server presents, read without trusting it: it records the leaf certificate's pin and refuses it, so the
/// handshake ends before any request is sent.
#[derive(Debug)]
pub(super) struct Recorder {
  presented: Arc<OnceLock<CertificatePin>>,
  provider: Arc<CryptoProvider>,
}

impl Recorder {
  /// A TLS client configuration that records the presented certificate in `presented` and refuses it.
  pub fn config(presented: Arc<OnceLock<CertificatePin>>) -> Result<rustls::ClientConfig, Error> {
    let recorder = Arc::new(Self {
      presented,
      provider: Arc::new(rustls::crypto::ring::default_provider()),
    });
    Ok(
      rustls::ClientConfig::builder_with_provider(recorder.provider.clone())
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(recorder)
        .with_no_client_auth(),
    )
  }
}

impl ServerCertVerifier for Recorder {
  fn verify_server_cert(
    &self,
    end_entity: &CertificateDer<'_>,
    _intermediates: &[CertificateDer<'_>],
    _server_name: &ServerName<'_>,
    _ocsp_response: &[u8],
    _now: UnixTime,
  ) -> Result<ServerCertVerified, Error> {
    // A second handshake of the same probe would present the same certificate; the first one is kept.
    let _ = self.presented.set(CertificatePin::of(end_entity));
    Err(Error::InvalidCertificate(
      CertificateError::ApplicationVerificationFailure,
    ))
  }

  fn verify_tls12_signature(
    &self,
    _message: &[u8],
    _certificate: &CertificateDer<'_>,
    _signature: &DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, Error> {
    Err(Error::InvalidCertificate(
      CertificateError::ApplicationVerificationFailure,
    ))
  }

  fn verify_tls13_signature(
    &self,
    _message: &[u8],
    _certificate: &CertificateDer<'_>,
    _signature: &DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, Error> {
    Err(Error::InvalidCertificate(
      CertificateError::ApplicationVerificationFailure,
    ))
  }

  fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
    self.provider.signature_verification_algorithms.supported_schemes()
  }
}

#[derive(Debug)]
pub(super) struct PinnedVerifier {
  pin: CertificatePin,
  provider: Arc<CryptoProvider>,
}

impl PinnedVerifier {
  pub fn new(pin: CertificatePin) -> Self {
    Self {
      pin,
      provider: Arc::new(rustls::crypto::ring::default_provider()),
    }
  }

  /// A TLS client configuration that accepts only the pinned certificate.
  pub fn config(pin: CertificatePin) -> Result<rustls::ClientConfig, Error> {
    let verifier = Arc::new(Self::new(pin));
    Ok(
      rustls::ClientConfig::builder_with_provider(verifier.provider.clone())
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_no_client_auth(),
    )
  }
}

impl ServerCertVerifier for PinnedVerifier {
  fn verify_server_cert(
    &self,
    end_entity: &CertificateDer<'_>,
    _intermediates: &[CertificateDer<'_>],
    _server_name: &ServerName<'_>,
    _ocsp_response: &[u8],
    _now: UnixTime,
  ) -> Result<ServerCertVerified, Error> {
    if CertificatePin::of(end_entity) == self.pin {
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
