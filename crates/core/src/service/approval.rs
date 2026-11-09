//! What a person approves of a service start (`docs/architecture/agent-testing.md`, "Service › approval"): one
//! request for a service, or a stack, with everything it starts, each service with its program, arguments and the
//! names of its variables, never their values, and a fingerprint of its program, arguments, variables and seed.
//! *Approve for every run* keeps one rule of the project per service, keyed by that fingerprint, so a change to any of
//! them asks again ([`crate::run::ProjectScope`]).

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{Service, stored};

/// The characters of a fingerprint: a SHA-256 in lowercase hex.
const FINGERPRINT_CHARS: usize = 64;
/// How many characters of a fingerprint a person reads.
const SHORT_FINGERPRINT: usize = 7;

/// One service a start runs, as its approval names it.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ServiceApproval {
  pub name: String,
  pub program: String,
  #[serde(default)]
  pub arguments: Vec<String>,
  /// The names of its variables; their values, plain or a secret's, are only in the fingerprint.
  #[serde(default)]
  pub variables: Vec<String>,
  /// The lowercase hex SHA-256 of its program, arguments, variables with their values (a secret by its name) and seed.
  pub fingerprint: String,
}

impl ServiceApproval {
  /// What a start of `service` is approved as.
  pub fn of(service: &Service) -> Self {
    let hex: String = Sha256::digest(stored::approved_bytes(service))
      .iter()
      .map(|byte| format!("{byte:02x}"))
      .collect();
    Self {
      name: service.name().as_str().to_owned(),
      program: service.program().as_str().to_owned(),
      arguments: service
        .arguments()
        .iter()
        .map(|argument| argument.as_str().to_owned())
        .collect(),
      variables: service
        .variables()
        .iter()
        .map(|(name, _)| name.as_str().to_owned())
        .collect(),
      fingerprint: hex,
    }
  }

  /// The fingerprint abbreviated, for a person to read.
  pub fn short_fingerprint(&self) -> &str {
    self.fingerprint.get(..SHORT_FINGERPRINT).unwrap_or(&self.fingerprint)
  }

  /// Its program and arguments as one line, for a person to read.
  pub fn command_line(&self) -> String {
    std::iter::once(self.program.as_str())
      .chain(self.arguments.iter().map(String::as_str))
      .collect::<Vec<_>>()
      .join(" ")
  }

  /// Whether it names a service and a SHA-256 fingerprint, as one read back must.
  pub fn is_valid(&self) -> bool {
    !self.name.is_empty()
      && self.fingerprint.len() == FINGERPRINT_CHARS
      && self
        .fingerprint
        .chars()
        .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
  }
}

/// The target of a `start_service` or `start_stack` request: every service the start runs, in the order it starts
/// them, as JSON `{"services": [...]}`.
#[derive(Serialize, Deserialize)]
struct StartTarget {
  services: Vec<ServiceApproval>,
}

/// The target a request for a start of `services` names.
pub fn start_target(services: &[ServiceApproval]) -> String {
  serde_json::to_string(&StartTarget {
    services: services.to_vec(),
  })
  .expect("a list of texts is always JSON")
}

/// The services a [`start_target`] names; `None` for any other text, and for one that names none or a fingerprint
/// that is not a SHA-256.
pub fn started_by(target: &str) -> Option<Vec<ServiceApproval>> {
  let StartTarget { services } = serde_json::from_str(target).ok()?;
  (!services.is_empty() && services.iter().all(ServiceApproval::is_valid)).then_some(services)
}
