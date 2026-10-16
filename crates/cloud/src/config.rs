use std::{
  net::SocketAddr,
  path::{Path, PathBuf},
};

use orchester_api::Credential;
use serde::Deserialize;

use crate::{addresses::TrustedProxy, error::CloudError};

/// The cloud's configuration (`docs/architecture/server.md`, row 59): a TOML file may give values, and
/// `ORCHESTER_CLOUD_<KEY>` environment variables win. Nothing about where it runs is in the code.
#[derive(Debug)]
pub struct Config {
  pub listen: SocketAddr,
  /// Where people reach the cloud and its web front, without a trailing `/`: the links in emails start with it, and
  /// identity tokens name it as their issuer.
  pub public_url: String,
  /// `postgres://…` or key-value form; it holds the database password, so it is a credential.
  pub database_url: Credential,
  /// The relay as lettre reads it: `smtps://user:pass@host:465`, `smtp://user:pass@host:587?tls=required`, or plain
  /// `smtp://host:25` for a relay on the same host or private network. It holds the relay's password.
  pub smtp_url: Credential,
  /// The sender, e.g. `Orchester <no-reply@example.com>`.
  pub smtp_from: String,
  /// 64 hex characters (32 bytes): seals the root's and the signing keys' private halves in the database. The root
  /// is made at the first start; a different key later stops the start rather than make another root.
  pub key_encryption_key: Credential,
  pub trusted_proxies: Vec<TrustedProxy>,
}

#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct File {
  listen: Option<SocketAddr>,
  public_url: Option<String>,
  database_url: Option<Credential>,
  smtp_url: Option<Credential>,
  smtp_from: Option<String>,
  key_encryption_key: Option<Credential>,
  trusted_proxies: Option<Vec<TrustedProxy>>,
}

impl Config {
  pub fn load(path: Option<&Path>) -> Result<Self, CloudError> {
    let path = path
      .map(Path::to_path_buf)
      .or_else(|| Path::new("cloud.toml").exists().then(|| PathBuf::from("cloud.toml")));
    let mut file = match path {
      Some(path) => toml::from_str(&std::fs::read_to_string(path)?)
        .map_err(|_| CloudError::Configuration("cannot parse the cloud's TOML file"))?,
      None => File::default(),
    };
    file.apply_environment()?;
    file.into_config()
  }

  pub fn validate(&self) -> Result<(), CloudError> {
    let url = &self.public_url;
    let scheme = url.starts_with("https://") || url.starts_with("http://");
    if !scheme || url.ends_with('/') || url.contains(['?', '#', ' ']) {
      return Err(CloudError::Configuration(
        "ORCHESTER_CLOUD_PUBLIC_URL must be an http(s) address without a trailing slash, query or fragment",
      ));
    }
    if self.smtp_from.trim().is_empty() {
      return Err(CloudError::Configuration("ORCHESTER_CLOUD_SMTP_FROM must not be empty"));
    }
    Ok(())
  }
}

impl File {
  fn apply_environment(&mut self) -> Result<(), CloudError> {
    if let Some(value) = environment("LISTEN")? {
      self.listen = Some(
        value
          .parse()
          .map_err(|_| CloudError::Configuration("ORCHESTER_CLOUD_LISTEN is not an address and port"))?,
      );
    }
    set(&mut self.public_url, "PUBLIC_URL")?;
    set_secret(&mut self.database_url, "DATABASE_URL")?;
    set_secret(&mut self.smtp_url, "SMTP_URL")?;
    set(&mut self.smtp_from, "SMTP_FROM")?;
    set_secret(&mut self.key_encryption_key, "KEY_ENCRYPTION_KEY")?;
    if let Some(value) = environment("TRUSTED_PROXIES")? {
      self.trusted_proxies = Some(
        value
          .split(',')
          .map(str::trim)
          .filter(|proxy| !proxy.is_empty())
          .map(str::parse)
          .collect::<Result<_, _>>()?,
      );
    }
    Ok(())
  }

  fn into_config(self) -> Result<Config, CloudError> {
    let missing = CloudError::Configuration;
    let config = Config {
      listen: self.listen.unwrap_or(SocketAddr::from(([127, 0, 0, 1], 8090))),
      public_url: self
        .public_url
        .ok_or(missing("ORCHESTER_CLOUD_PUBLIC_URL is required"))?,
      database_url: self
        .database_url
        .ok_or(missing("ORCHESTER_CLOUD_DATABASE_URL is required"))?,
      smtp_url: self.smtp_url.ok_or(missing("ORCHESTER_CLOUD_SMTP_URL is required"))?,
      smtp_from: self.smtp_from.ok_or(missing("ORCHESTER_CLOUD_SMTP_FROM is required"))?,
      key_encryption_key: self
        .key_encryption_key
        .ok_or(missing("ORCHESTER_CLOUD_KEY_ENCRYPTION_KEY is required"))?,
      trusted_proxies: self.trusted_proxies.unwrap_or_default(),
    };
    config.validate()?;
    Ok(config)
  }
}

fn set(field: &mut Option<String>, key: &str) -> Result<(), CloudError> {
  if let Some(value) = environment(key)? {
    *field = Some(value);
  }
  Ok(())
}

fn set_secret(field: &mut Option<Credential>, key: &str) -> Result<(), CloudError> {
  if let Some(value) = environment(key)? {
    *field = Some(Credential::new(value));
  }
  Ok(())
}

fn environment(key: &str) -> Result<Option<String>, CloudError> {
  match std::env::var(format!("ORCHESTER_CLOUD_{key}")) {
    Ok(value) => Ok(Some(value)),
    Err(std::env::VarError::NotPresent) => Ok(None),
    Err(std::env::VarError::NotUnicode(_)) => Err(CloudError::Configuration("environment must be UTF-8")),
  }
}
