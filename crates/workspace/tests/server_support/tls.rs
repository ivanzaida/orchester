//! Set-up requests to a test server that presents its own certificate: a TLS client that trusts exactly that
//! certificate, as the desktop does once the person compared its check code.

use std::sync::Arc;

use orchester_api::certificate::CertificatePin;
use tokio::net::TcpStream;
use tokio_rustls::{
  TlsConnector,
  client::TlsStream,
  rustls::{
    ClientConfig, DigitallySignedStruct, Error, SignatureScheme,
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    crypto::{CryptoProvider, ring, verify_tls12_signature, verify_tls13_signature},
    pki_types::{CertificateDer, ServerName, UnixTime},
  },
};

/// A connection to `address` that trusts only the certificate `pin`.
pub async fn connect(address: std::net::SocketAddr, pin: CertificatePin) -> TlsStream<TcpStream> {
  let provider = Arc::new(ring::default_provider());
  let config = ClientConfig::builder_with_provider(provider.clone())
    .with_safe_default_protocol_versions()
    .expect("protocol versions")
    .dangerous()
    .with_custom_certificate_verifier(Arc::new(Pinned { pin, provider }))
    .with_no_client_auth();
  let stream = TcpStream::connect(address).await.expect("connect");
  let name = ServerName::try_from("localhost").expect("server name");
  TlsConnector::from(Arc::new(config))
    .connect(name, stream)
    .await
    .expect("TLS handshake")
}

#[derive(Debug)]
struct Pinned {
  pin: CertificatePin,
  provider: Arc<CryptoProvider>,
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
    if CertificatePin::of(end_entity) == self.pin {
      Ok(ServerCertVerified::assertion())
    } else {
      Err(Error::General("another certificate".into()))
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
