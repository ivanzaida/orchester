use std::fmt;

use crate::resource::VariableName;

/// Where in a service a text with placeholders is written. Indexes count from 0; messages count from 1.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PlaceholderPlace {
  Argument(usize),
  /// A plain value; a secret has no placeholders.
  Variable(VariableName),
  /// The path of an HTTP readiness check.
  ReadinessPath,
  SeedUrl {
    request: usize,
  },
  SeedBody {
    request: usize,
  },
  /// A plain header value; a secret has no placeholders.
  SeedHeader {
    request: usize,
    header: String,
  },
  /// An argument of a seed command.
  SeedArgument(usize),
  /// Where `ui_open` starts.
  UiStart,
}

impl fmt::Display for PlaceholderPlace {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Argument(index) => write!(f, "argument {}", index + 1),
      Self::Variable(name) => write!(f, "the variable `{}`", name.as_str()),
      Self::ReadinessPath => f.write_str("the readiness path"),
      Self::SeedUrl { request } => write!(f, "the URL of seed request {}", request + 1),
      Self::SeedBody { request } => write!(f, "the body of seed request {}", request + 1),
      Self::SeedHeader { request, header } => write!(f, "the header `{header}` of seed request {}", request + 1),
      Self::SeedArgument(index) => write!(f, "argument {} of the seed command", index + 1),
      Self::UiStart => f.write_str("the place `ui_open` starts"),
    }
  }
}
