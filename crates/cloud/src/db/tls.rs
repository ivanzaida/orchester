use std::sync::Arc;

use rustls::ClientConfig;
use rustls_platform_verifier::BuilderVerifierExt;
use tokio_postgres_rustls::MakeRustlsConnect;

use crate::error::CloudError;

/// TLS to PostgreSQL with ring and the operating system's trust store. The connection string's `sslmode` decides
/// whether it is used: `require` for a database across a network, `disable` for one on the same private network.
pub(super) fn connector() -> Result<MakeRustlsConnect, CloudError> {
  let config = ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
    .with_safe_default_protocol_versions()
    .and_then(BuilderVerifierExt::with_platform_verifier)
    .map_err(|_| CloudError::Configuration("cannot set up TLS for the database"))?
    .with_no_client_auth();
  Ok(MakeRustlsConnect::new(config))
}
