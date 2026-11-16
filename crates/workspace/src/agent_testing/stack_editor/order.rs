//! What the stack editor works out from the project's services as they are now (each its draft if it has one,
//! otherwise its latest version), as a start of the stack resolves them ([`StartPlan::for_stack`]): every service it
//! starts in start order, with its step, whether the person chose it or a need brought it, and what it waits for
//! (`Services` `x4S3LY`); and the same services as a tree of needs (`Dependencies` `z1aSH`).

use std::sync::Arc;

use orchester_core::service::{NeedsError, RunData, Service, ServiceName, Stack, StartPlan, UiDriver};

use crate::{agent_testing::editor::order::order_error, messages::Message};

/// A service a start of the stack brings up.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
  pub name: Arc<str>,
  /// The step it starts on, from 1; the services of one step start together.
  pub step: usize,
  /// The person chose it; otherwise a chosen service needs it.
  pub chosen: bool,
  /// The services of the stack that need it, in start order.
  pub needed_by: Vec<Arc<str>>,
  /// The other services that start on its step.
  pub together: Vec<Arc<str>>,
  pub driver: UiDriver,
}

/// A line of the tree of needs: a service as deep as the step it starts on, under the need it waits for longest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Branch {
  pub name: Arc<str>,
  /// 0 for a service that needs nothing.
  pub depth: usize,
  /// A fresh database container, which the tree marks as one.
  pub database: bool,
}

/// The services `stack` starts, from the project's `services`, in start order.
pub fn members(stack: &Stack, services: &[Service]) -> Result<Vec<Member>, Message> {
  let plan = StartPlan::for_stack(services, stack).map_err(|error| stack_order_error(&error))?;
  let started: Vec<(&ServiceName, usize)> = plan
    .steps()
    .iter()
    .enumerate()
    .flat_map(|(step, names)| names.iter().map(move |name| (name, step)))
    .collect();
  let body = |name: &ServiceName| {
    services
      .iter()
      .find(|service| service.name() == name)
      .expect("a start plan names only the services it was resolved from")
  };
  Ok(
    started
      .iter()
      .map(|&(name, step)| Member {
        name: name.as_str().into(),
        step: step + 1,
        chosen: stack.services().contains(name),
        needed_by: started
          .iter()
          .filter(|(other, _)| body(other).needs().contains(name))
          .map(|(other, _)| other.as_str().into())
          .collect(),
        together: plan.steps()[step]
          .iter()
          .filter(|other| *other != name)
          .map(|other| other.as_str().into())
          .collect(),
        driver: body(name).ui_driver(),
      })
      .collect(),
  )
}

/// The tree of needs of the services `stack` starts: each service that needs nothing at the root, each other one
/// under the need on its latest step (the first such need listed), so a service is as deep as its step and shows once.
pub fn tree(stack: &Stack, services: &[Service]) -> Result<Vec<Branch>, Message> {
  let members = members(stack, services)?;
  let parent = |member: &Member| -> Option<Arc<str>> {
    let service = services
      .iter()
      .find(|service| service.name().as_str() == &*member.name)?;
    let step_of = |need: &ServiceName| members.iter().find(|m| &*m.name == need.as_str()).map(|m| m.step);
    let mut best: Option<(&ServiceName, usize)> = None;
    for need in service.needs() {
      let step = step_of(need)?;
      if best.is_none_or(|(_, at)| step > at) {
        best = Some((need, step));
      }
    }
    best.map(|(need, _)| need.as_str().into())
  };
  let parents: Vec<Option<Arc<str>>> = members.iter().map(parent).collect();
  let mut branches = Vec::with_capacity(members.len());
  for (at, parent) in parents.iter().enumerate() {
    if parent.is_none() {
      grow(&members, &parents, at, 0, &mut branches, services);
    }
  }
  Ok(branches)
}

/// Adds member `at` at `depth` and, after it, the members under it in start order.
fn grow(
  members: &[Member],
  parents: &[Option<Arc<str>>],
  at: usize,
  depth: usize,
  branches: &mut Vec<Branch>,
  services: &[Service],
) {
  let member = &members[at];
  let database = services
    .iter()
    .find(|service| service.name().as_str() == &*member.name)
    .is_some_and(|service| matches!(service.run_data(), RunData::DatabaseContainer));
  branches.push(Branch {
    name: member.name.clone(),
    depth,
    database,
  });
  for (child, parent) in parents.iter().enumerate() {
    if parent.as_deref() == Some(&*member.name) {
      grow(members, parents, child, depth + 1, branches, services);
    }
  }
}

/// Why a stack's services do not resolve: it lists a service the project does not have, a need names one, or the needs
/// form a cycle.
fn stack_order_error(error: &NeedsError) -> Message {
  match error {
    NeedsError::Missing { name, needed_by: None } => {
      Message::new("services.stack.errors.missing").with("service", name.to_string())
    }
    other => order_error(other),
  }
}
