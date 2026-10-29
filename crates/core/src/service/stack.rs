use std::collections::BTreeSet;

use super::{ServiceError, ServiceName, StackName};

/// A named set of services started together, e.g. `full`; their needs are started with them
/// ([`super::StartPlan::for_stack`]).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Stack {
  name: StackName,
  services: Vec<ServiceName>,
}

impl Stack {
  /// At least one service, each listed once.
  pub fn new(name: StackName, services: Vec<ServiceName>) -> Result<Self, ServiceError> {
    if services.is_empty() {
      return Err(ServiceError::EmptyStack(name));
    }
    let mut seen = BTreeSet::new();
    if let Some(service) = services.iter().find(|service| !seen.insert(*service)) {
      return Err(ServiceError::DuplicateStackService {
        service: service.clone(),
        stack: name,
      });
    }
    Ok(Self { name, services })
  }

  pub const fn name(&self) -> &StackName {
    &self.name
  }

  /// The services as the person listed them.
  pub fn services(&self) -> &[ServiceName] {
    &self.services
  }
}
