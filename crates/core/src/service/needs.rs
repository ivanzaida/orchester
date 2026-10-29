use std::collections::BTreeMap;

use thiserror::Error;

use super::{Service, ServiceName, Stack};

/// Why a service or stack cannot be started from a set of services.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum NeedsError {
  #[error("the service `{0}` is defined twice")]
  DuplicateService(ServiceName),
  /// `needed_by` is `None` when the start or the stack names the service itself.
  #[error("{}", missing_message(name, needed_by.as_ref()))]
  Missing {
    name: ServiceName,
    needed_by: Option<ServiceName>,
  },
  /// The services of the cycle in the order they need each other, starting and ending with the same one.
  #[error("the needs form a cycle: {}", cycle_message(.0))]
  Cycle(Vec<ServiceName>),
}

fn missing_message(name: &ServiceName, needed_by: Option<&ServiceName>) -> String {
  match needed_by {
    Some(service) => format!("`{service}` needs `{name}`, which is not defined"),
    None => format!("there is no service `{name}`"),
  }
}

fn cycle_message(cycle: &[ServiceName]) -> String {
  cycle.iter().map(ServiceName::as_str).collect::<Vec<_>>().join(" → ")
}

/// The order a start brings services up in: step by step, each service after every service it needs. The
/// services on one step need none of each other and start together; within a step they keep the order the start
/// and the needs list them in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartPlan {
  steps: Vec<Vec<ServiceName>>,
}

impl StartPlan {
  /// The plan that starts `name` and everything it needs, from `services`.
  pub fn for_service(services: &[Service], name: &ServiceName) -> Result<Self, NeedsError> {
    Self::resolve(services, std::slice::from_ref(name))
  }

  /// The plan that starts every service of `stack` and everything they need, from `services`.
  pub fn for_stack(services: &[Service], stack: &Stack) -> Result<Self, NeedsError> {
    Self::resolve(services, stack.services())
  }

  pub fn steps(&self) -> &[Vec<ServiceName>] {
    &self.steps
  }

  fn resolve(services: &[Service], wanted: &[ServiceName]) -> Result<Self, NeedsError> {
    let mut by_name = BTreeMap::new();
    for service in services {
      if by_name.insert(service.name(), service).is_some() {
        return Err(NeedsError::DuplicateService(service.name().clone()));
      }
    }
    let mut resolver = Resolver {
      by_name,
      visits: BTreeMap::new(),
      path: Vec::new(),
      started: Vec::new(),
    };
    for name in wanted {
      resolver.visit(name, None)?;
    }
    let mut steps = Vec::<Vec<ServiceName>>::new();
    for (name, step) in resolver.started {
      if steps.len() <= step {
        steps.resize_with(step + 1, Vec::new);
      }
      steps[step].push(name.clone());
    }
    Ok(Self { steps })
  }
}

enum Visit {
  /// Its needs are being visited, so meeting it again closes a cycle.
  Open,
  Done(usize),
}

/// A depth-first walk of the needs: a service's step is one after the latest step of what it needs.
struct Resolver<'a> {
  by_name: BTreeMap<&'a ServiceName, &'a Service>,
  visits: BTreeMap<&'a ServiceName, Visit>,
  path: Vec<&'a ServiceName>,
  /// Each service with its step, after everything it needs.
  started: Vec<(&'a ServiceName, usize)>,
}

impl<'a> Resolver<'a> {
  fn visit(&mut self, name: &ServiceName, needed_by: Option<&ServiceName>) -> Result<usize, NeedsError> {
    let Some((&name, &service)) = self.by_name.get_key_value(name) else {
      return Err(NeedsError::Missing {
        name: name.clone(),
        needed_by: needed_by.cloned(),
      });
    };
    match self.visits.get(name) {
      Some(Visit::Done(step)) => return Ok(*step),
      Some(Visit::Open) => return Err(self.cycle_to(name)),
      None => {}
    }
    self.visits.insert(name, Visit::Open);
    self.path.push(name);
    let mut step = 0;
    for need in service.needs() {
      step = step.max(self.visit(need, Some(name))? + 1);
    }
    self.path.pop();
    self.visits.insert(name, Visit::Done(step));
    self.started.push((name, step));
    Ok(step)
  }

  fn cycle_to(&self, name: &'a ServiceName) -> NeedsError {
    let start = self
      .path
      .iter()
      .position(|open| *open == name)
      .expect("a service is open only while it is on the path");
    let cycle = self.path[start..]
      .iter()
      .chain([&name])
      .map(|service| (*service).clone());
    NeedsError::Cycle(cycle.collect())
  }
}
