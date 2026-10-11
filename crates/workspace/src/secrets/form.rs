//! Validation shared by the Secrets form and callers preparing secret metadata.

use orchester_core::secret::{self, Environment, NewSecret, Secret, SecretKind, SecretName, SecretNameError};

/// What an entered name permits in the chosen environment.
#[derive(Debug, PartialEq, Eq)]
pub enum NameCheck {
  Empty,
  Invalid(SecretNameError),
  Taken(Secret),
  Free(SecretName),
}

pub fn check_name(secrets: &[Secret], entered: &str, environment: Environment) -> NameCheck {
  if entered.trim().is_empty() {
    return NameCheck::Empty;
  }
  let name = match SecretName::new(entered) {
    Ok(name) => name,
    Err(error) => return NameCheck::Invalid(error),
  };
  match secret::existing(secrets, &name, environment) {
    Some(secret) => NameCheck::Taken(secret.clone()),
    None => NameCheck::Free(name),
  }
}

impl NameCheck {
  /// Metadata ready for saving; the lifecycle checks uniqueness again when it writes.
  pub fn new_secret(&self, kind: SecretKind, environment: Environment) -> Option<NewSecret> {
    match self {
      Self::Free(name) => Some(NewSecret {
        name: name.clone(),
        kind,
        environment,
      }),
      _ => None,
    }
  }
}
