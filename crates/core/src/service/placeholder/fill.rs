use std::{borrow::Cow, collections::BTreeMap, fmt};

use super::{Part, Placeholder, PlaceholderError, PlaceholderPlace, parts};
use crate::{
  id::RunId,
  resource::{FieldName, HttpMethod, VariableName},
  service::{ReadyCheck, Seed, SeedRequest, Service, ServiceName, ServiceValue},
};

/// Where a started service listens in a run.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Address {
  port: u16,
  url: String,
}

/// What a run fills a service's placeholders with: its own port, URL and run id, the started services' URLs and
/// ports, and what the seed requests captured so far. Secrets are not among them: a secret stays a name.
#[derive(Clone, PartialEq, Eq)]
pub struct RunValues {
  run: RunId,
  own: Address,
  services: BTreeMap<ServiceName, Address>,
  captured: BTreeMap<FieldName, String>,
}

impl RunValues {
  pub fn new(run: RunId, port: u16, url: &str) -> Self {
    Self {
      run,
      own: Address {
        port,
        url: url.to_owned(),
      },
      services: BTreeMap::new(),
      captured: BTreeMap::new(),
    }
  }

  /// A started service the placeholders may use.
  #[must_use]
  pub fn with_service(mut self, name: ServiceName, port: u16, url: &str) -> Self {
    self.services.insert(
      name,
      Address {
        port,
        url: url.to_owned(),
      },
    );
    self
  }

  /// What a seed request's answer gave `name`, for the requests after it and the place `ui_open` starts.
  pub fn capture(&mut self, name: FieldName, value: String) {
    self.captured.insert(name, value);
  }

  /// `text` with every placeholder replaced by its value; an error naming `place` for a placeholder the values
  /// have nothing for, never an empty string.
  pub fn fill(&self, text: &str, place: PlaceholderPlace) -> Result<String, PlaceholderError> {
    let mut filled = String::with_capacity(text.len());
    for part in parts(text, &place)? {
      match part {
        Part::Text(text) => filled.push_str(text),
        Part::Placeholder(placeholder) => match self.value(&placeholder) {
          Some(value) => filled.push_str(&value),
          None => return Err(PlaceholderError::NoValue { place, placeholder }),
        },
      }
    }
    Ok(filled)
  }

  /// The arguments the program gets.
  pub fn arguments(&self, service: &Service) -> Result<Vec<String>, PlaceholderError> {
    let arguments = service.arguments().iter().enumerate();
    arguments
      .map(|(index, argument)| self.fill(argument.as_str(), PlaceholderPlace::Argument(index)))
      .collect()
  }

  /// The variables the program gets: plain values filled, secrets left as their names.
  pub fn variables(&self, service: &Service) -> Result<Vec<(VariableName, ServiceValue)>, PlaceholderError> {
    let fill = |(name, value): (&VariableName, &ServiceValue)| {
      let value = match value {
        ServiceValue::Plain(text) => ServiceValue::Plain(self.fill(text, PlaceholderPlace::Variable(name.clone()))?),
        ServiceValue::Secret(secret) => ServiceValue::Secret(secret.clone()),
      };
      Ok((name.clone(), value))
    };
    service.variables().iter().map(fill).collect()
  }

  /// The path an HTTP readiness check asks; `None` for another check.
  pub fn readiness_path(&self, service: &Service) -> Result<Option<String>, PlaceholderError> {
    match service.readiness().check() {
      ReadyCheck::Http(path) => self.fill(path.as_str(), PlaceholderPlace::ReadinessPath).map(Some),
      ReadyCheck::PortOpen | ReadyCheck::LogLine(_) => Ok(None),
    }
  }

  /// Seed request `request` of `service`, filled once the requests before it captured their values; `None` when
  /// the service's seed has no such request.
  pub fn seed_request(&self, service: &Service, request: usize) -> Result<Option<FilledSeedRequest>, PlaceholderError> {
    let Seed::Requests(requests) = service.seed() else {
      return Ok(None);
    };
    requests
      .as_slice()
      .get(request)
      .map(|seed_request| self.fill_request(seed_request, request))
      .transpose()
  }

  /// The arguments of the seed command; none for another seed.
  pub fn seed_arguments(&self, service: &Service) -> Result<Vec<String>, PlaceholderError> {
    let Seed::Command { arguments, .. } = service.seed() else {
      return Ok(Vec::new());
    };
    arguments
      .iter()
      .enumerate()
      .map(|(index, argument)| self.fill(argument.as_str(), PlaceholderPlace::SeedArgument(index)))
      .collect()
  }

  /// Where `ui_open` starts, once the seed is done; `None` when the service does not say.
  pub fn ui_start(&self, service: &Service) -> Result<Option<String>, PlaceholderError> {
    service
      .ui_start()
      .map(|start| self.fill(start.as_str(), PlaceholderPlace::UiStart))
      .transpose()
  }

  fn fill_request(&self, seed_request: &SeedRequest, request: usize) -> Result<FilledSeedRequest, PlaceholderError> {
    let url = self.fill(seed_request.url(), PlaceholderPlace::SeedUrl { request })?;
    let body = seed_request
      .body()
      .map(|body| self.fill(body, PlaceholderPlace::SeedBody { request }))
      .transpose()?;
    let mut headers = Vec::with_capacity(seed_request.headers().len());
    for header in seed_request.headers() {
      let value = match header.value() {
        ServiceValue::Plain(text) => {
          let place = PlaceholderPlace::SeedHeader {
            request,
            header: header.name().to_owned(),
          };
          ServiceValue::Plain(self.fill(text, place)?)
        }
        ServiceValue::Secret(secret) => ServiceValue::Secret(secret.clone()),
      };
      headers.push((header.name().to_owned(), value));
    }
    Ok(FilledSeedRequest {
      method: seed_request.method(),
      url,
      body,
      headers,
    })
  }

  fn value(&self, placeholder: &Placeholder) -> Option<Cow<'_, str>> {
    let address = |name: &ServiceName| self.services.get(name);
    match placeholder {
      Placeholder::Port => Some(self.own.port.to_string().into()),
      Placeholder::Url => Some(self.own.url.as_str().into()),
      Placeholder::Run => Some(self.run.to_string().into()),
      Placeholder::ServiceUrl(name) => address(name).map(|address| address.url.as_str().into()),
      Placeholder::ServicePort(name) => address(name).map(|address| address.port.to_string().into()),
      Placeholder::Seed(name) => self.captured.get(name).map(|value| value.as_str().into()),
    }
  }
}

/// A captured value may be a token, so `Debug` shows only the captured names.
impl fmt::Debug for RunValues {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.debug_struct("RunValues")
      .field("run", &self.run)
      .field("own", &self.own)
      .field("services", &self.services)
      .field("captured", &self.captured.keys().collect::<Vec<_>>())
      .finish()
  }
}

/// A seed request with its placeholders filled; a header that names a secret still names it.
#[derive(Clone, PartialEq, Eq)]
pub struct FilledSeedRequest {
  method: HttpMethod,
  url: String,
  body: Option<String>,
  headers: Vec<(String, ServiceValue)>,
}

impl FilledSeedRequest {
  pub const fn method(&self) -> HttpMethod {
    self.method
  }

  pub fn url(&self) -> &str {
    &self.url
  }

  pub fn body(&self) -> Option<&str> {
    self.body.as_deref()
  }

  pub fn headers(&self) -> &[(String, ServiceValue)] {
    &self.headers
  }
}

/// A body may carry a password and a URL a captured token, so `Debug` shows only their lengths.
impl fmt::Debug for FilledSeedRequest {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let length = |text: &str| format!("<{} characters>", text.chars().count());
    f.debug_struct("FilledSeedRequest")
      .field("method", &self.method)
      .field("url", &length(&self.url))
      .field("body", &self.body.as_deref().map(length))
      .field("headers", &self.headers)
      .finish()
  }
}
