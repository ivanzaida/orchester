//! The `{{…}}` placeholders of a service (`docs/architecture/agent-testing.md`, "Service"): `{{port}}`, `{{url}}`
//! and `{{run}}` of the service's own start, `{{services.<name>.url}}` and `{{services.<name>.port}}` of a service
//! it needs, and `{{seed.<name>}}`, a value an earlier seed request captured. Placeholders are data references,
//! never expressions, and they are filled only into plain text: a secret stays a name, which the runner resolves.

mod check;
mod error;
mod fill;
mod place;

use std::fmt;

pub use self::{
  check::check_placeholders,
  error::PlaceholderError,
  fill::{FilledSeedRequest, RunValues},
  place::PlaceholderPlace,
};
use super::ServiceName;
use crate::resource::FieldName;

/// What one placeholder stands for.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Placeholder {
  /// `{{port}}`: the port the run gave this service.
  Port,
  /// `{{url}}`: this service's URL in the run.
  Url,
  /// `{{run}}`: the run's id.
  Run,
  /// `{{services.<name>.url}}`
  ServiceUrl(ServiceName),
  /// `{{services.<name>.port}}`
  ServicePort(ServiceName),
  /// `{{seed.<name>}}`: what an earlier seed request captured as `name`.
  Seed(FieldName),
}

impl Placeholder {
  /// The placeholders of `text` in order; an error naming `place` for a brace that does not make one.
  pub fn find_all(text: &str, place: &PlaceholderPlace) -> Result<Vec<Self>, PlaceholderError> {
    let parts = parts(text, place)?;
    let placeholders = parts.into_iter().filter_map(|part| match part {
      Part::Text(_) => None,
      Part::Placeholder(placeholder) => Some(placeholder),
    });
    Ok(placeholders.collect())
  }

  /// The placeholder `{{inner}}` stands for.
  fn parse(inner: &str, place: &PlaceholderPlace) -> Result<Self, PlaceholderError> {
    let parts: Vec<_> = inner.split('.').collect();
    match parts[..] {
      ["port"] => Ok(Self::Port),
      ["url"] => Ok(Self::Url),
      ["run"] => Ok(Self::Run),
      ["services", name, field @ ("url" | "port")] => {
        let name = ServiceName::new(name).map_err(|_| PlaceholderError::UnknownService {
          place: place.clone(),
          service: name.to_owned(),
        })?;
        Ok(if field == "url" {
          Self::ServiceUrl(name)
        } else {
          Self::ServicePort(name)
        })
      }
      ["seed", name] => FieldName::new(name)
        .map(Self::Seed)
        .map_err(|_| PlaceholderError::UnknownCapture {
          place: place.clone(),
          capture: name.to_owned(),
        }),
      _ => Err(PlaceholderError::UnknownField {
        place: place.clone(),
        placeholder: format!("{{{{{inner}}}}}"),
      }),
    }
  }
}

impl fmt::Display for Placeholder {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Port => f.write_str("{{port}}"),
      Self::Url => f.write_str("{{url}}"),
      Self::Run => f.write_str("{{run}}"),
      Self::ServiceUrl(name) => write!(f, "{{{{services.{name}.url}}}}"),
      Self::ServicePort(name) => write!(f, "{{{{services.{name}.port}}}}"),
      Self::Seed(name) => write!(f, "{{{{seed.{name}}}}}"),
    }
  }
}

/// A piece of a text: written as is, or a placeholder to fill.
enum Part<'a> {
  Text(&'a str),
  Placeholder(Placeholder),
}

/// `text` split at its placeholders. A `}}` outside a placeholder is text, as a JSON body ends objects with it;
/// a `{{` must close with `}}` around a name without braces.
fn parts<'a>(text: &'a str, place: &PlaceholderPlace) -> Result<Vec<Part<'a>>, PlaceholderError> {
  let mut parts = Vec::new();
  let mut rest = text;
  while let Some(start) = rest.find("{{") {
    parts.push(Part::Text(&rest[..start]));
    let after = &rest[start + 2..];
    let end = after
      .find("}}")
      .ok_or_else(|| PlaceholderError::Unclosed { place: place.clone() })?;
    let inner = &after[..end];
    if inner.is_empty() || inner.contains(['{', '}']) {
      return Err(PlaceholderError::Malformed { place: place.clone() });
    }
    parts.push(Part::Placeholder(Placeholder::parse(inner, place)?));
    rest = &after[end + 2..];
  }
  parts.push(Part::Text(rest));
  Ok(parts)
}
