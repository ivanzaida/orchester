//! Input and discovery identity of an embedded app call. Discovery credentials never leave the executor.

use std::{num::NonZeroU32, sync::Arc};

use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;

use crate::{command::SecretVariable, secret::SecretValue};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppCall {
  pid: NonZeroU32,
  tool: String,
  arguments: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AppInput {
  pid: NonZeroU32,
  tool: String,
  arguments: Value,
}

impl AppCall {
  pub(super) fn parse(arguments: Value) -> Result<Self, String> {
    let input: AppInput =
      serde_json::from_value(arguments).map_err(|_| "expected pid, tool and arguments".to_owned())?;
    if input.tool.is_empty()
      || input.tool.len() > 200
      || input.tool.chars().any(|c| c.is_control() || c.is_whitespace())
    {
      return Err("tool must be a name of at most 200 bytes without whitespace".to_owned());
    }
    if !input.arguments.is_object() {
      return Err("arguments must be a JSON object".to_owned());
    }
    Ok(Self {
      pid: input.pid,
      tool: input.tool,
      arguments: input.arguments,
    })
  }

  pub const fn pid(&self) -> NonZeroU32 {
    self.pid
  }

  pub fn tool(&self) -> &str {
    &self.tool
  }

  pub const fn arguments(&self) -> &Value {
    &self.arguments
  }
}

/// Validated discovery of exactly the requested process and app. Debug redacts its credential.
#[derive(Debug)]
pub struct AppDiscovery {
  port: u16,
  token: SecretVariable,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AppDiscoveryError {
  #[error("refused: MCP discovery is invalid")]
  Invalid,
  #[error("refused: MCP discovery does not name the requested pid and app")]
  Identity,
  #[error("refused: MCP discovery is not the expected loopback endpoint")]
  Endpoint,
}

impl AppDiscovery {
  /// Parses discovery without ever putting its contents or parser errors into diagnostics.
  pub fn parse(bytes: &[u8], pid: NonZeroU32, app: &str) -> Result<Self, AppDiscoveryError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| AppDiscoveryError::Invalid)?;
    if value["pid"].as_u64() != Some(u64::from(pid.get())) || value["app"].as_str() != Some(app) {
      return Err(AppDiscoveryError::Identity);
    }
    let port = value["port"]
      .as_u64()
      .and_then(|port| u16::try_from(port).ok())
      .filter(|port| *port != 0)
      .ok_or(AppDiscoveryError::Invalid)?;
    if value["url"].as_str() != Some(format!("http://127.0.0.1:{port}/mcp").as_str()) {
      return Err(AppDiscoveryError::Endpoint);
    }
    let token = value["token"]
      .as_str()
      .filter(|token| !token.is_empty() && token.len() <= 4096)
      .ok_or(AppDiscoveryError::Invalid)?;
    let token = SecretValue::new(token.to_owned()).map_err(|_| AppDiscoveryError::Invalid)?;
    Ok(Self {
      port,
      token: SecretVariable::new("app discovery", Arc::new(token)),
    })
  }

  pub fn url(&self) -> String {
    format!("http://127.0.0.1:{}/mcp", self.port)
  }

  /// Only the HTTP authorization header and answer redaction may use this value.
  pub const fn token(&self) -> &SecretVariable {
    &self.token
  }
}
