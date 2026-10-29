use std::fmt;

use super::ServiceError;
use crate::resource::Slug;

/// What a service is called and started by, in kebab-case, e.g. `team-server`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ServiceName(Slug);

impl ServiceName {
  pub const MAX_CHARS: usize = Slug::MAX_CHARS;

  pub fn new(value: &str) -> Result<Self, ServiceError> {
    Slug::new(value)
      .map(Self)
      .map_err(|_| ServiceError::Name(value.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    self.0.as_str()
  }
}

impl fmt::Display for ServiceName {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.as_str())
  }
}

/// What a stack is called and started by, in kebab-case as a service, e.g. `full`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StackName(Slug);

impl StackName {
  pub fn new(value: &str) -> Result<Self, ServiceError> {
    Slug::new(value)
      .map(Self)
      .map_err(|_| ServiceError::Name(value.to_owned()))
  }

  pub fn as_str(&self) -> &str {
    self.0.as_str()
  }
}

impl fmt::Display for StackName {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(self.as_str())
  }
}
