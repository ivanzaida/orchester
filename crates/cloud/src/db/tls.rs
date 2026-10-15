use std::{net::IpAddr, sync::Arc};

use rustls::ClientConfig;
use rustls_platform_verifier::BuilderVerifierExt;
use tokio_postgres::config::{Config, Host, SslMode};
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

/// The connection string must say how it is protected (review 1, L3): `sslmode=require`, or `sslmode=disable` for a
/// database on the local network only, never `prefer` (the default), which falls back to plain text when an attacker
/// in between refuses TLS. Local means a Unix socket, a loopback or private address, or a single-label host name
/// such as a compose service (`postgres`), which public DNS does not answer; a `hostaddr`, which decides where the
/// connection goes, must be a loopback or private address too.
pub(super) fn check_transport(config: &Config) -> Result<(), CloudError> {
  match config.get_ssl_mode() {
    SslMode::Require => Ok(()),
    SslMode::Disable if config.get_hosts().iter().all(local) && config.get_hostaddrs().iter().all(local_ip) => Ok(()),
    SslMode::Disable => Err(CloudError::Configuration(
      "DATABASE_URL has sslmode=disable for a host outside the local network; use sslmode=require",
    )),
    _ => Err(CloudError::Configuration(
      "DATABASE_URL must say sslmode=require (or sslmode=disable for a database on the local network)",
    )),
  }
}

fn local(host: &Host) -> bool {
  match host {
    Host::Tcp(name) => match name.trim_matches(['[', ']']).parse::<IpAddr>() {
      Ok(ip) => local_ip(&ip),
      Err(_) => name.eq_ignore_ascii_case("localhost") || !name.contains('.'),
    },
    #[cfg(unix)]
    Host::Unix(_) => true,
  }
}

fn local_ip(ip: &IpAddr) -> bool {
  match ip {
    IpAddr::V4(ip) => ip.is_loopback() || ip.is_private(),
    IpAddr::V6(ip) => ip.is_loopback() || ip.is_unique_local(),
  }
}
