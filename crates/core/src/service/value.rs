use std::{
  collections::{BTreeMap, BTreeSet},
  fmt,
};

use super::ServiceError;
use crate::{resource::VariableName, secret::SecretName};

/// The value of a variable or a request header: text as written, or a project secret by name, whose value the
/// runner reads from the keychain when it starts the service.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum ServiceValue {
  Plain(String),
  Secret(SecretName),
}

/// A person may type a token as a plain value, so `Debug` shows only how long it is.
impl fmt::Debug for ServiceValue {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Plain(value) => write!(f, "Plain(<{} characters>)", value.chars().count()),
      Self::Secret(name) => f.debug_tuple("Secret").field(name).finish(),
    }
  }
}

/// The variables a service's program gets on top of the cleaned environment, each named once without regard to
/// case.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServiceVariables(BTreeMap<VariableName, ServiceValue>);

impl ServiceVariables {
  pub fn new(variables: BTreeMap<VariableName, ServiceValue>) -> Result<Self, ServiceError> {
    let mut seen = BTreeSet::new();
    for (name, value) in &variables {
      if !seen.insert(name.as_str().to_ascii_uppercase()) {
        return Err(ServiceError::DuplicateVariable(name.as_str().to_owned()));
      }
      if matches!(value, ServiceValue::Plain(text) if text.contains('\0')) {
        return Err(ServiceError::VariableValue(name.as_str().to_owned()));
      }
    }
    Ok(Self(variables))
  }

  pub fn iter(&self) -> impl Iterator<Item = (&VariableName, &ServiceValue)> {
    self.0.iter()
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }
}
