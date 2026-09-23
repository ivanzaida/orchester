use std::{collections::BTreeMap, fmt};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::id::SecretId;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
  Get,
  Post,
  Put,
  Patch,
  Delete,
}

/// How the request authenticates. The secret is resolved by the runner at call time; the
/// body holds only its reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpAuth {
  None,
  Bearer { secret: SecretId },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FieldNameError {
  #[error(
    "field name must be 1–{max} characters: a lowercase letter followed by lowercase letters, digits or \
     underscores, got `{0}`",
    max = FieldName::MAX_CHARS
  )]
  Invalid(String),
}

/// Name of a tool input the agent supplies or an output it receives, e.g. `revision`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct FieldName(String);

impl FieldName {
  pub const MAX_CHARS: usize = 64;

  pub fn new(value: &str) -> Result<Self, FieldNameError> {
    let mut chars = value.chars();
    let starts_with_letter = chars.next().is_some_and(|c| c.is_ascii_lowercase());
    let rest_valid = chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if starts_with_letter && rest_valid && value.len() <= Self::MAX_CHARS {
      Ok(Self(value.to_owned()))
    } else {
      Err(FieldNameError::Invalid(value.to_owned()))
    }
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Display for FieldName {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

impl TryFrom<String> for FieldName {
  type Error = FieldNameError;

  fn try_from(value: String) -> Result<Self, FieldNameError> {
    Self::new(&value)
  }
}

impl From<FieldName> for String {
  fn from(name: FieldName) -> Self {
    name.0
  }
}

/// Where an input value is placed in the request. Body paths are dot-separated JSON object
/// keys, e.g. `deployment.revision`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestTarget {
  Query(String),
  Header(String),
  Body(String),
}

impl RequestTarget {
  fn is_valid(&self) -> bool {
    match self {
      Self::Query(name) => is_token(name),
      Self::Header(name) => is_header_name(name),
      Self::Body(path) => is_body_path(path),
    }
  }
}

/// Where an output value is read from in the response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseSource {
  Status,
  Header(String),
  Body(String),
}

impl ResponseSource {
  fn is_valid(&self) -> bool {
    match self {
      Self::Status => true,
      Self::Header(name) => is_header_name(name),
      Self::Body(path) => is_body_path(path),
    }
  }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HttpError {
  #[error("`{0}` is not an absolute http(s) URL with a host and without embedded credentials")]
  InvalidUrl(String),
  #[error("the URL host `{host}` is not the allowed host `{allowed}`")]
  HostNotAllowed { host: String, allowed: String },
  #[error("bearer authentication requires an https URL")]
  InsecureBearer,
  #[error("the request or response location mapped to field `{0}` is not valid")]
  InvalidLocation(FieldName),
  #[error("input `{0}` targets the Authorization header, which is reserved for the tool's authentication")]
  ReservedHeader(FieldName),
}

/// A single HTTP request with a fixed URL. Agents supply mapped inputs; they never choose the
/// host or see the credential.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "HttpRequestFields")]
pub struct HttpRequest {
  method: HttpMethod,
  url: String,
  allowed_host: String,
  auth: HttpAuth,
  inputs: BTreeMap<FieldName, RequestTarget>,
  outputs: BTreeMap<FieldName, ResponseSource>,
}

#[derive(Deserialize)]
struct HttpRequestFields {
  method: HttpMethod,
  url: String,
  allowed_host: String,
  auth: HttpAuth,
  inputs: BTreeMap<FieldName, RequestTarget>,
  outputs: BTreeMap<FieldName, ResponseSource>,
}

impl TryFrom<HttpRequestFields> for HttpRequest {
  type Error = HttpError;

  fn try_from(fields: HttpRequestFields) -> Result<Self, HttpError> {
    Self::new(
      fields.method,
      fields.url,
      fields.allowed_host,
      fields.auth,
      fields.inputs,
      fields.outputs,
    )
  }
}

impl HttpRequest {
  pub fn new(
    method: HttpMethod,
    url: String,
    allowed_host: String,
    auth: HttpAuth,
    inputs: BTreeMap<FieldName, RequestTarget>,
    outputs: BTreeMap<FieldName, ResponseSource>,
  ) -> Result<Self, HttpError> {
    let (secure, host) = split_url(&url)?;
    if !host.eq_ignore_ascii_case(&allowed_host) {
      return Err(HttpError::HostNotAllowed {
        host: host.to_owned(),
        allowed: allowed_host,
      });
    }
    if matches!(auth, HttpAuth::Bearer { .. }) && !secure {
      return Err(HttpError::InsecureBearer);
    }
    check_inputs(&inputs)?;
    if let Some((field, _)) = outputs.iter().find(|(_, source)| !source.is_valid()) {
      return Err(HttpError::InvalidLocation(field.clone()));
    }
    Ok(Self {
      method,
      url,
      allowed_host,
      auth,
      inputs,
      outputs,
    })
  }

  pub const fn method(&self) -> HttpMethod {
    self.method
  }

  pub fn url(&self) -> &str {
    &self.url
  }

  pub fn allowed_host(&self) -> &str {
    &self.allowed_host
  }

  pub const fn auth(&self) -> HttpAuth {
    self.auth
  }

  pub fn inputs(&self) -> &BTreeMap<FieldName, RequestTarget> {
    &self.inputs
  }

  pub fn outputs(&self) -> &BTreeMap<FieldName, ResponseSource> {
    &self.outputs
  }
}

fn check_inputs(inputs: &BTreeMap<FieldName, RequestTarget>) -> Result<(), HttpError> {
  for (field, target) in inputs {
    if !target.is_valid() {
      return Err(HttpError::InvalidLocation(field.clone()));
    }
    if matches!(target, RequestTarget::Header(name) if name.eq_ignore_ascii_case("authorization")) {
      return Err(HttpError::ReservedHeader(field.clone()));
    }
  }
  Ok(())
}

/// Returns whether the URL is https, and its host without port.
fn split_url(url: &str) -> Result<(bool, &str), HttpError> {
  let invalid = || HttpError::InvalidUrl(url.to_owned());
  if url.chars().any(|c| c.is_whitespace() || c.is_control()) {
    return Err(invalid());
  }
  let (scheme, rest) = url.split_once("://").ok_or_else(invalid)?;
  let secure = match scheme.to_ascii_lowercase().as_str() {
    "https" => true,
    "http" => false,
    _ => return Err(invalid()),
  };
  let authority = rest.find(['/', '?', '#']).map_or(rest, |end| &rest[..end]);
  if authority.contains('@') {
    return Err(invalid());
  }
  let host = match authority.rsplit_once(':') {
    Some((host, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => host,
    _ => authority,
  };
  if host.is_empty() {
    return Err(invalid());
  }
  Ok((secure, host))
}

fn is_token(value: &str) -> bool {
  !value.is_empty() && !value.chars().any(|c| c.is_whitespace() || c.is_control())
}

fn is_header_name(value: &str) -> bool {
  !value.is_empty()
    && value
      .bytes()
      .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
}

fn is_body_path(value: &str) -> bool {
  value.split('.').all(is_token)
}
