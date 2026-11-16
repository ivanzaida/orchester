//! An HTTP tool's request as typed (A32 `gLLEv`, L20 `Kb6Vk`): its method, URL, the one host it may call, whether it
//! sends a secret as its bearer token, and its inputs and outputs one per line ([`super::lines`]). Each field is
//! checked on its own, so every field that is wrong says why at once (L22 `Xp89k`): a URL that is no absolute http(s)
//! URL, or an http one with a bearer token; an allowed host that is not the URL's; the lines that are wrong. The
//! request is then the core type's, which checks it again.

use std::collections::BTreeMap;

use orchester_core::{
  resource::{
    FieldName, HttpAuth, HttpError, HttpMethod, HttpRequest, HttpUrl, RequestTarget, ResponseSource, TeamHttpRequest,
  },
  secret::SecretName,
};

use super::lines::{LineError, inputs_text, outputs_text, parse_inputs, parse_outputs};
use crate::messages::Message;

/// The request's fields as the user left them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpText {
  pub method: HttpMethod,
  pub url: String,
  pub allowed_host: String,
  /// It sends a secret's value as its bearer token.
  pub bearer: bool,
  /// `name → place KEY` per line.
  pub inputs: String,
  /// `name ← place NAME` per line.
  pub outputs: String,
}

impl Default for HttpText {
  /// A new request: a GET with nothing typed and no authorization.
  fn default() -> Self {
    Self {
      method: HttpMethod::Get,
      url: String::new(),
      allowed_host: String::new(),
      bearer: false,
      inputs: String::new(),
      outputs: String::new(),
    }
  }
}

/// Why the request cannot be made yet, per field. A field that holds nothing says so too: the form shows that only once
/// Create was pressed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HttpErrors {
  pub url: Option<Message>,
  pub allowed_host: Option<Message>,
  /// Every wrong line, in order.
  pub inputs: Vec<LineError>,
  pub outputs: Vec<LineError>,
}

impl HttpErrors {
  pub fn is_empty(&self) -> bool {
    *self == Self::default()
  }
}

/// The request with its secret left to the caller: local or the team's.
pub(super) struct Checked {
  pub method: HttpMethod,
  pub url: String,
  pub allowed_host: String,
  pub inputs: BTreeMap<FieldName, RequestTarget>,
  pub outputs: BTreeMap<FieldName, ResponseSource>,
}

impl HttpText {
  /// The fields of `request`.
  pub fn of(request: &HttpRequest) -> Self {
    Self {
      method: request.method(),
      url: request.url().to_owned(),
      allowed_host: request.allowed_host().to_owned(),
      bearer: matches!(request.auth(), HttpAuth::Bearer { .. }),
      inputs: inputs_text(request.inputs()),
      outputs: outputs_text(request.outputs()),
    }
  }

  /// The fields of the team's `request`.
  pub fn of_team(request: &TeamHttpRequest) -> Self {
    Self {
      method: request.method(),
      url: request.url().to_owned(),
      allowed_host: request.allowed_host().to_owned(),
      bearer: request.bearer().is_some(),
      inputs: inputs_text(request.inputs()),
      outputs: outputs_text(request.outputs()),
    }
  }

  /// Every field checked on its own; the URL and the host trimmed.
  pub(super) fn checked(&self) -> Result<Checked, Box<HttpErrors>> {
    let (url, allowed_host) = (self.url.trim(), self.allowed_host.trim());
    let parsed = if url.is_empty() {
      Err(Message::new("tools.errors.urlRequired"))
    } else {
      HttpUrl::parse(url).map_err(|error| http_message(&error))
    };
    let url_error = match &parsed {
      Err(message) => Some(message.clone()),
      Ok(parsed) if self.bearer && !parsed.https => Some(http_message(&HttpError::InsecureBearer)),
      Ok(_) => None,
    };
    let host_error = match (&parsed, allowed_host) {
      (_, "") => Some(Message::new("tools.errors.hostRequired")),
      (Ok(parsed), allowed) if !parsed.host.eq_ignore_ascii_case(allowed) => {
        Some(http_message(&HttpError::HostNotAllowed {
          host: parsed.host.to_owned(),
          allowed: allowed.to_owned(),
        }))
      }
      _ => None,
    };
    let (inputs, outputs) = (parse_inputs(&self.inputs), parse_outputs(&self.outputs));
    match (url_error, host_error, inputs, outputs) {
      (None, None, Ok(inputs), Ok(outputs)) => Ok(Checked {
        method: self.method,
        url: url.to_owned(),
        allowed_host: allowed_host.to_owned(),
        inputs,
        outputs,
      }),
      (url, allowed_host, inputs, outputs) => Err(Box::new(HttpErrors {
        url,
        allowed_host,
        inputs: inputs.err().unwrap_or_default(),
        outputs: outputs.err().unwrap_or_default(),
      })),
    }
  }
}

impl Checked {
  /// The local request, authenticating as `auth` says.
  pub(super) fn request(self, auth: HttpAuth) -> Result<HttpRequest, Box<HttpErrors>> {
    HttpRequest::new(
      self.method,
      self.url,
      self.allowed_host,
      auth,
      self.inputs,
      self.outputs,
    )
    .map_err(|error| refused(&error))
  }

  /// The team's request, sending the team secret `bearer` as its token, if any.
  pub(super) fn team_request(self, bearer: Option<SecretName>) -> Result<TeamHttpRequest, Box<HttpErrors>> {
    TeamHttpRequest::new(
      self.method,
      (self.url, self.allowed_host),
      bearer,
      self.inputs,
      self.outputs,
    )
    .map_err(|error| refused(&error))
  }
}

/// What the core type refused after the fields were checked, at the field it comes from.
fn refused(error: &HttpError) -> Box<HttpErrors> {
  let message = http_message(error);
  Box::new(match error {
    HttpError::InvalidUrl(_) | HttpError::InsecureBearer => HttpErrors {
      url: Some(message),
      ..HttpErrors::default()
    },
    HttpError::HostNotAllowed { .. } => HttpErrors {
      allowed_host: Some(message),
      ..HttpErrors::default()
    },
    // Each line was checked as the request checks it (`RequestTarget::is_valid`, the reserved header) before it.
    HttpError::InvalidLocation(_) | HttpError::ReservedHeader(_) => {
      unreachable!("the lines were checked as the request checks them")
    }
  })
}

/// Why a request's URL, host or a location cannot be used.
pub fn http_message(error: &HttpError) -> Message {
  match error {
    HttpError::InvalidUrl(_) => Message::new("tools.errors.invalidUrl"),
    HttpError::HostNotAllowed { host, allowed } => Message::new("tools.errors.hostNotAllowed")
      .with("host", host.clone())
      .with("allowed", allowed.clone()),
    HttpError::InsecureBearer => Message::new("tools.errors.insecureBearer"),
    HttpError::InvalidLocation(field) => Message::new("tools.errors.invalidLocation").with("name", field.to_string()),
    HttpError::ReservedHeader(field) => Message::new("tools.errors.reservedHeader").with("name", field.to_string()),
  }
}
