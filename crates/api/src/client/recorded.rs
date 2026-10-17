//! Trust through the operating system's store, as reqwest's own verifier gives it (`rustls-platform-verifier`), that
//! also keeps the leaf certificate of the last connection it verified. A team server's key proof names the certificate
//! its connections present; the app compares it with the one this client's own connection presented, after the chain
//! and the name were checked, so a proof passed on from another server over another connection is caught
//! (`docs/architecture/server.md`, "Codes").

use std::sync::{Arc, OnceLock};

use parking_lot::Mutex;
use rustls::{
  DigitallySignedStruct, Error, SignatureScheme,
  client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
  pki_types::{CertificateDer, ServerName, UnixTime},
};
use rustls_platform_verifier::Verifier;

use crate::certificate::CertificatePin;

/// Certificate authorities the store does not hold that a test trusts besides it ([`trust_for_tests`]).
static TEST_ROOTS: OnceLock<Vec<CertificateDer<'static>>> = OnceLock::new();

/// Trusts `roots` besides the operating system's store in every client of this process made afterwards: for tests
/// that serve a certificate from a test authority; a second call changes nothing.
#[cfg(feature = "test-support")]
pub fn trust_for_tests(roots: Vec<CertificateDer<'static>>) {
  // A process trusts one set of test roots; a later set is ignored, as the comment above says.
  let _ = TEST_ROOTS.set(roots);
}

/// The leaf certificate of the last connection a [`Recorded`] verifier accepted.
pub(super) type Presented = Arc<Mutex<Option<CertificatePin>>>;

#[derive(Debug)]
pub(super) struct Recorded {
  inner: Verifier,
  presented: Presented,
}

impl Recorded {
  /// A TLS client configuration that trusts the operating system's store (and the tests' roots) and records each
  /// accepted leaf in `presented`.
  pub fn config(presented: Presented) -> Result<rustls::ClientConfig, Error> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let inner = match TEST_ROOTS.get() {
      Some(roots) => Verifier::new_with_extra_roots(roots.iter().cloned(), provider.clone())?,
      None => Verifier::new(provider.clone())?,
    };
    Ok(
      rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(Self { inner, presented }))
        .with_no_client_auth(),
    )
  }
}

impl ServerCertVerifier for Recorded {
  fn verify_server_cert(
    &self,
    end_entity: &CertificateDer<'_>,
    intermediates: &[CertificateDer<'_>],
    server_name: &ServerName<'_>,
    ocsp_response: &[u8],
    now: UnixTime,
  ) -> Result<ServerCertVerified, Error> {
    let verified = self
      .inner
      .verify_server_cert(end_entity, intermediates, server_name, ocsp_response, now)?;
    *self.presented.lock() = Some(CertificatePin::of(end_entity));
    Ok(verified)
  }

  fn verify_tls12_signature(
    &self,
    message: &[u8],
    certificate: &CertificateDer<'_>,
    signature: &DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, Error> {
    self.inner.verify_tls12_signature(message, certificate, signature)
  }

  fn verify_tls13_signature(
    &self,
    message: &[u8],
    certificate: &CertificateDer<'_>,
    signature: &DigitallySignedStruct,
  ) -> Result<HandshakeSignatureValid, Error> {
    self.inner.verify_tls13_signature(message, certificate, signature)
  }

  fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
    self.inner.supported_verify_schemes()
  }
}
