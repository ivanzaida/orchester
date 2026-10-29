//! Environment variables a test suite's program gets: plain values, stored in the body, or a project
//! secret named by its name. A secret is named rather than referenced by id because its value is kept
//! in each computer's keychain, and a shared project's suites travel to computers whose secrets have
//! other ids; the value is read when the suite runs and is never stored, shown or logged.

use std::collections::{BTreeMap, BTreeSet};

use super::TestSuiteError;
use crate::{
  resource::{CommandError, VariableName},
  secret::SecretName,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SuiteVariable {
  Plain(String),
  Secret(SecretName),
}

/// The variables of one suite, distinct by name without regard to case, as Windows compares them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SuiteVariables(BTreeMap<VariableName, SuiteVariable>);

impl SuiteVariables {
  pub fn new(variables: BTreeMap<VariableName, SuiteVariable>) -> Result<Self, TestSuiteError> {
    let mut seen = BTreeSet::new();
    for (name, value) in &variables {
      if !seen.insert(name.as_str().to_ascii_uppercase()) {
        return Err(CommandError::DuplicateVariable(name.as_str().to_owned()).into());
      }
      if matches!(value, SuiteVariable::Plain(plain) if plain.contains('\0')) {
        return Err(CommandError::VariableValue(name.as_str().to_owned()).into());
      }
    }
    Ok(Self(variables))
  }

  pub fn iter(&self) -> impl Iterator<Item = (&VariableName, &SuiteVariable)> {
    self.0.iter()
  }

  pub fn is_empty(&self) -> bool {
    self.0.is_empty()
  }
}
