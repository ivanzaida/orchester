use std::{
  net::SocketAddr,
  path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::{addresses::TrustedProxy, error::ServerError};

#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
  pub listen: SocketAddr,
  pub data_dir: PathBuf,
  pub name: String,
  pub tls_cert: Option<PathBuf>,
  pub tls_key: Option<PathBuf>,
  pub trusted_proxies: Vec<TrustedProxy>,
}

impl Default for Config {
  fn default() -> Self {
    Self {
      listen: SocketAddr::from(([127, 0, 0, 1], 8080)),
      data_dir: PathBuf::from("data"),
      name: "Orchester".into(),
      tls_cert: None,
      tls_key: None,
      trusted_proxies: Vec::new(),
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
    if let Some(value) = environment("TLS_CERT")? {
      self.tls_cert = Some(value.into());
    }
    if let Some(value) = environment("TLS_KEY")? {
      self.tls_key = Some(value.into());
    }
    if let Some(value) = environment("TRUSTED_PROXIES")? {
      self.trusted_proxies = if value.trim().is_empty() {
        Vec::new()
      } else {
        value
          .split(',')
          .map(|ip| ip.trim().parse())
          .collect::<Result<_, _>>()
          .map_err(|_| ServerError::Configuration("invalid TRUSTED_PROXIES"))?
      };
    }
    Ok(())
  }

  pub fn validate(&self) -> Result<(), ServerError> {
    if self.tls_cert.is_some() != self.tls_key.is_some() {
      return Err(ServerError::Configuration("TLS_CERT and TLS_KEY must both be supplied"));
    }
    if self.name.trim().is_empty() || self.data_dir.as_os_str().is_empty() {
      return Err(ServerError::Configuration("NAME and DATA_DIR must not be empty"));
    }
    Ok(())
  }
}

fn environment(key: &str) -> Result<Option<String>, ServerError> {
  match std::env::var(format!("ORCHESTER_SERVER_{key}")) {
    Ok(value) => Ok(Some(value)),
    Err(std::env::VarError::NotPresent) => Ok(None),
    Err(std::env::VarError::NotUnicode(_)) => Err(ServerError::Configuration("environment must be UTF-8")),
  }
}
