use std::{
  net::SocketAddr,
  path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::{addresses::TrustedProxy, error::ServerError};

/// How the server's connections are secured (`docs/architecture/server.md`, "Transport and certificates").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TlsMode {
  /// The server's own certificate, generated on its first start and pinned by the desktop.
  #[default]
  SelfSigned,
  /// The team's certificate and key (`tls_cert`, `tls_key`).
  Pem,
  /// Plain HTTP, for a server behind a proxy that holds the certificate.
  Off,
}

impl std::str::FromStr for TlsMode {
  type Err = ServerError;

  fn from_str(value: &str) -> Result<Self, Self::Err> {
    match value {
      "self_signed" => Ok(Self::SelfSigned),
      "pem" => Ok(Self::Pem),
      "off" => Ok(Self::Off),
      _ => Err(ServerError::Configuration("TLS must be self_signed, pem or off")),
    }
  }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
  pub listen: SocketAddr,
  pub data_dir: PathBuf,
  pub name: String,
  /// The server's public base address (`https://orchester.example.com`); projects are at `<address>/<slug>`. Without
  /// it, the address a request was sent to is used.
  pub address: Option<String>,
  pub tls: TlsMode,
  /// Names and addresses the generated certificate carries besides `localhost` and the loopback addresses.
  pub tls_names: Vec<String>,
  pub tls_cert: Option<PathBuf>,
  pub tls_key: Option<PathBuf>,
  pub trusted_proxies: Vec<TrustedProxy>,
  /// The server admin app on a private address (`http://host:port`); every path outside `/api` is forwarded to it.
  pub admin_url: String,
  /// The private plain-HTTP listener only the admin app's own server side calls the API on; `None` turns it off.
  pub admin_api_listen: Option<SocketAddr>,
}

impl Default for Config {
  fn default() -> Self {
    Self {
      listen: SocketAddr::from(([127, 0, 0, 1], 8080)),
      data_dir: PathBuf::from("data"),
      name: "Orchester".into(),
      address: None,
      tls: TlsMode::SelfSigned,
      tls_names: Vec::new(),
      tls_cert: None,
      tls_key: None,
      trusted_proxies: Vec::new(),
      admin_url: "http://127.0.0.1:3100".into(),
      admin_api_listen: Some(SocketAddr::from(([127, 0, 0, 1], 8741))),
    }
  }
}

impl Config {
  pub fn load(path: Option<&Path>) -> Result<Self, ServerError> {
    let mut config = match path {
      Some(path) => toml::from_str(&std::fs::read_to_string(path)?)
        .map_err(|_| ServerError::Configuration("cannot parse server TOML"))?,
      None if Path::new("server.toml").exists() => toml::from_str(&std::fs::read_to_string("server.toml")?)
        .map_err(|_| ServerError::Configuration("cannot parse server.toml"))?,
      None => Self::default(),
    };
    config.apply_environment()?;
    config.validate()?;
    Ok(config)
  }

  fn apply_environment(&mut self) -> Result<(), ServerError> {
    if let Some(value) = environment("LISTEN")? {
      self.listen = value
        .parse()
        .map_err(|_| ServerError::Configuration("invalid LISTEN"))?;
    }
    if let Some(value) = environment("DATA_DIR")? {
      self.data_dir = value.into();
    }
    if let Some(value) = environment("NAME")? {
      self.name = value;
    }
    if let Some(value) = environment("ADDRESS")? {
      self.address = Some(value);
    }
    if let Some(value) = environment("TLS")? {
      self.tls = value.parse()?;
    }
    if let Some(value) = environment("TLS_NAMES")? {
      self.tls_names = list(&value).map(str::to_owned).collect();
    }
    if let Some(value) = environment("TLS_CERT")? {
      self.tls_cert = Some(value.into());
    }
    if let Some(value) = environment("TLS_KEY")? {
      self.tls_key = Some(value.into());
    }
    if let Some(value) = environment("ADMIN_URL")? {
      self.admin_url = value;
    }
    if let Some(value) = environment("ADMIN_API_LISTEN")? {
      self.admin_api_listen = match value.trim() {
        "off" => None,
        address => Some(
          address
            .parse()
            .map_err(|_| ServerError::Configuration("invalid ADMIN_API_LISTEN"))?,
        ),
      };
    }
    if let Some(value) = environment("TRUSTED_PROXIES")? {
      self.trusted_proxies = list(&value)
        .map(str::parse)
        .collect::<Result<_, _>>()
        .map_err(|_| ServerError::Configuration("invalid TRUSTED_PROXIES"))?;
    }
    Ok(())
  }

  pub fn validate(&self) -> Result<(), ServerError> {
    let pem_files = self.tls_cert.is_some() || self.tls_key.is_some();
    match self.tls {
      TlsMode::Pem if self.tls_cert.is_none() || self.tls_key.is_none() => {
        return Err(ServerError::Configuration(
          "tls = \"pem\" needs both TLS_CERT and TLS_KEY",
        ));
      }
      TlsMode::SelfSigned | TlsMode::Off if pem_files => {
        return Err(ServerError::Configuration(
          "TLS_CERT and TLS_KEY are used only with tls = \"pem\"",
        ));
      }
      _ => {}
    }
    if self.name.trim().is_empty() || self.data_dir.as_os_str().is_empty() {
      return Err(ServerError::Configuration("NAME and DATA_DIR must not be empty"));
    }
    if self.address.as_deref().is_some_and(|address| !valid_address(address)) {
      return Err(ServerError::Configuration(
        "ADDRESS must be an http:// or https:// base address without a path",
      ));
    }
    if !valid_admin_url(&self.admin_url) {
      return Err(ServerError::Configuration(
        "ADMIN_URL must be an http:// address with a host and port and no path",
      ));
    }
    if self.admin_api_listen.is_some_and(|admin| admin == self.listen) {
      return Err(ServerError::Configuration("ADMIN_API_LISTEN must differ from LISTEN"));
    }
    Ok(())
  }
}

fn valid_address(address: &str) -> bool {
  let rest = address
    .strip_prefix("https://")
    .or_else(|| address.strip_prefix("http://"));
  rest
    .is_some_and(|host| !host.is_empty() && !host.contains(['/', '?', '#', '@']) && !host.contains(char::is_whitespace))
}

/// The admin app is reached over plain HTTP on a private address: `http://host:port`, nothing more.
fn valid_admin_url(url: &str) -> bool {
  url
    .strip_prefix("http://")
    .and_then(|authority| authority.parse::<hyper::http::uri::Authority>().ok())
    .is_some_and(|authority| authority.port_u16().is_some() && !authority.as_str().contains('@'))
}

fn list(value: &str) -> impl Iterator<Item = &str> {
  value.split(',').map(str::trim).filter(|item| !item.is_empty())
}

fn environment(key: &str) -> Result<Option<String>, ServerError> {
  match std::env::var(format!("ORCHESTER_SERVER_{key}")) {
    Ok(value) => Ok(Some(value)),
    Err(std::env::VarError::NotPresent) => Ok(None),
    Err(std::env::VarError::NotUnicode(_)) => Err(ServerError::Configuration("environment must be UTF-8")),
  }
}
