use std::{collections::BTreeSet, fmt};

use super::{ServiceError, ServiceValue};
use crate::resource::{CommandArg, CommandProgram, FieldName, HttpMethod};

/// Where in a JSON answer a captured value is read, e.g. `$.id` or `$.items[0].id`: `$` for the whole answer,
/// then `.key` and `[index]` parts.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CapturePath(String);

impl CapturePath {
  pub const MAX_CHARS: usize = 512;

  pub fn new(value: &str) -> Result<Self, ServiceError> {
    let invalid = || ServiceError::CapturePath(value.to_owned());
    if value.chars().count() > Self::MAX_CHARS {
      return Err(invalid());
    }
    let mut rest = value.strip_prefix('$').ok_or_else(invalid)?;
    while !rest.is_empty() {
      rest = match rest.as_bytes()[0] {
        b'.' => {
          let key_end = rest[1..].find(['.', '[']).map_or(rest.len(), |end| end + 1);
          let key = &rest[1..key_end];
          if key.is_empty() || key.chars().any(|c| c.is_whitespace() || c.is_control() || c == ']') {
            return Err(invalid());
          }
          &rest[key_end..]
        }
        b'[' => {
          let (index, after) = rest[1..].split_once(']').ok_or_else(invalid)?;
          if index.is_empty() || !index.bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalid());
          }
          after
        }
        _ => return Err(invalid()),
      };
    }
    Ok(Self(value.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// A value a seed request's answer gives later requests and agents, e.g. `team_id ← $.id`.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Capture {
  name: FieldName,
  path: CapturePath,
}

impl Capture {
  pub const fn new(name: FieldName, path: CapturePath) -> Self {
    Self { name, path }
  }

  pub const fn name(&self) -> &FieldName {
    &self.name
  }

  pub const fn path(&self) -> &CapturePath {
    &self.path
  }
}

/// A header a seed request sends; its value may be a project secret by name.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SeedHeader {
  name: String,
  value: ServiceValue,
}

impl SeedHeader {
  pub fn new(name: &str, value: ServiceValue) -> Result<Self, ServiceError> {
    let token = |b: u8| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b);
    if name.is_empty() || !name.bytes().all(token) {
      return Err(ServiceError::HeaderName(name.to_owned()));
    }
    if matches!(&value, ServiceValue::Plain(text) if text.chars().any(char::is_control)) {
      return Err(ServiceError::HeaderValue(name.to_owned()));
    }
    Ok(Self {
      name: name.to_owned(),
      value,
    })
  }

  pub fn name(&self) -> &str {
    &self.name
  }

  pub const fn value(&self) -> &ServiceValue {
    &self.value
  }
}

/// One HTTP request of a seed. The URL and the body keep their placeholders (`{{url}}`, `{{team_id}}`) as written.
#[derive(Clone, PartialEq, Eq)]
pub struct SeedRequest {
  method: HttpMethod,
  url: String,
  body: Option<String>,
  headers: Vec<SeedHeader>,
  captures: Vec<Capture>,
}

impl SeedRequest {
  pub const MAX_URL_CHARS: usize = 4096;

  pub fn new(
    method: HttpMethod,
    url: &str,
    body: Option<String>,
    headers: Vec<SeedHeader>,
    captures: Vec<Capture>,
  ) -> Result<Self, ServiceError> {
    let starts = ["http://", "https://", "{{"].iter().any(|start| url.starts_with(start));
    let plain = !url.chars().any(|c| c.is_whitespace() || c.is_control());
    if !starts || !plain || url.chars().count() > Self::MAX_URL_CHARS {
      return Err(ServiceError::SeedUrl);
    }
    let mut seen = BTreeSet::new();
    if let Some(header) = headers
      .iter()
      .find(|header| !seen.insert(header.name.to_ascii_lowercase()))
    {
      return Err(ServiceError::DuplicateHeader(header.name.clone()));
    }
    Ok(Self {
      method,
      url: url.to_owned(),
      body,
      headers,
      captures,
    })
  }

  pub const fn method(&self) -> HttpMethod {
    self.method
  }

  pub fn url(&self) -> &str {
    &self.url
  }

  pub fn body(&self) -> Option<&str> {
    self.body.as_deref()
  }

  pub fn headers(&self) -> &[SeedHeader] {
    &self.headers
  }

  pub fn captures(&self) -> &[Capture] {
    &self.captures
  }
}

/// A body may carry a password for the seed's own account, so `Debug` shows only how long it is.
impl fmt::Debug for SeedRequest {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("SeedRequest")
      .field("method", &self.method)
      .field("url", &self.url)
      .field(
        "body",
        &self
          .body
          .as_ref()
          .map(|body| format!("<{} characters>", body.chars().count())),
      )
      .field("headers", &self.headers)
      .field("captures", &self.captures)
      .finish()
  }
}

/// The HTTP requests of a seed, sent in order: at least one, and each captured name once across all of them, so a
/// later request and the agents read one value per name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeedRequests(Vec<SeedRequest>);

impl SeedRequests {
  pub fn new(requests: Vec<SeedRequest>) -> Result<Self, ServiceError> {
    if requests.is_empty() {
      return Err(ServiceError::NoSeedRequests);
    }
    let mut seen = BTreeSet::new();
    let captures = requests.iter().flat_map(SeedRequest::captures);
    for capture in captures {
      if !seen.insert(capture.name()) {
        return Err(ServiceError::DuplicateCapture(capture.name().clone()));
      }
    }
    Ok(Self(requests))
  }

  pub fn as_slice(&self) -> &[SeedRequest] {
    &self.0
  }
}

/// What runs once after the service is ready, before agents use it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Seed {
  None,
  Requests(SeedRequests),
  /// A program run without a shell in the service's working folder.
  Command {
    program: CommandProgram,
    arguments: Vec<CommandArg>,
  },
}
