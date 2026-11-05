//! Starting a service or a stack with what it needs (`docs/architecture/agent-testing.md`, "Service" and "Stack"):
//! the core orders the needs into steps ([`StartPlan`]); the services of one step start together, each on a thread of
//! its own, and a step starts only once every service of the steps before it is ready, so each service starts after
//! its needs are ready. A service the invocation runs already is reused, not started again. Each start's
//! `{{services.<name>.url|port}}` are filled from the services the invocation runs by then. When a service fails,
//! the services of its step that became ready are kept, nothing after it starts, and the error names it.
//!
//! A start's request names every service it runs ([`StepServices::service_approvals`]), and the start runs only those
//! definitions: a service published again since, with another fingerprint, is refused until it is asked about.

use std::thread;

use orchester_core::{
  orchester_tools::ToolResult,
  resource::Slug,
  service::{Service, ServiceApproval, ServiceName, Stack, StackName, StartPlan},
};
use orchester_ipc::services::ServiceStarter;
use serde_json::{Map, json};

use super::{
  Origin, Running, StepServices,
  launch::{published, published_stacks},
};

/// Where a service the invocation runs listens: its name, port and URL.
pub(super) type Address = (ServiceName, u16, String);

/// What a start brings up: a service with what it needs, or a stack's services with theirs.
#[derive(Clone, Copy)]
pub(in crate::execution::step) enum Started<'a> {
  Service(&'a ServiceName),
  Stack(&'a StackName),
}

impl StepServices {
  /// The services a start of `started` runs, in the order it starts them, as its request names them; why it cannot
  /// start, when the project's services do not let it.
  pub(in crate::execution::step) fn service_approvals(
    &self,
    started: Started<'_>,
  ) -> Result<Vec<ServiceApproval>, String> {
    let services = published(&self.root).map_err(|error| format!("the project's services cannot be read: {error}"))?;
    let plan = match started {
      Started::Service(name) => StartPlan::for_service(&services, name)
        .map_err(|error| format!("the service `{name}` cannot be started: {error}"))?,
      Started::Stack(name) => StartPlan::for_stack(&services, &self.stack(name)?)
        .map_err(|error| format!("the stack `{name}` cannot be started: {error}"))?,
    };
    Ok(
      plan
        .steps()
        .iter()
        .flatten()
        .filter_map(|name| services.iter().find(|service| service.name() == name))
        .map(ServiceApproval::of)
        .collect(),
    )
  }

  /// Starts the published service `name` with what it needs, in order, and waits until it is ready; a service the
  /// invocation runs already is reused. Runs only the definitions `approved` names. Blocks while an earlier start of
  /// the invocation runs.
  pub(in crate::execution::step) fn start(&self, name: &ServiceName, approved: &[ServiceApproval]) -> ToolResult {
    let _one_at_a_time = self.state.starting.lock();
    let started = self.services_of((Some(approved), &Origin::agent(None)), |services| {
      StartPlan::for_service(services, name).map_err(|error| format!("the service `{name}` cannot be started: {error}"))
    });
    if let Err(refusal) = started {
      return ToolResult::unavailable(&refusal);
    }
    match self.state.running.lock().get(name) {
      Some(running) => running.answer(),
      None => ToolResult::unavailable(&format!("the service `{name}` was started but no longer runs")),
    }
  }

  /// A person's Restart or Start in the Services tab: stops the service `name` when it runs, then starts it as
  /// published with what it needs, the person's press approving it, and waits until it is ready. A start of the
  /// agent's may be under way meanwhile, waiting for another service: the person's does not wait for it, only for an
  /// earlier one of the person's. A service both bring up at once is kept once ([`StepServices::keep`]).
  pub(super) fn restart_for_person(&self, name: &ServiceName) -> Result<(), String> {
    let _one_at_a_time = self.state.restarting.lock();
    let stopped = self.stop(name);
    if stopped.is_error() {
      return Err(stopped.text().to_owned());
    }
    let person = Origin {
      stack: None,
      by: ServiceStarter::Person,
    };
    self
      .services_of((None, &person), |services| {
        StartPlan::for_service(services, name).map_err(|error| format!("the service `{name}` cannot be started: {error}"))
      })
      .map(|_| ())
  }

  /// Starts every service of the published stack `name` with what they need, in order, and answers each one's URL
  /// and port; a service the invocation runs already is reused. Runs only the definitions `approved` names. Blocks
  /// while an earlier start of the invocation runs.
  pub(in crate::execution::step) fn start_stack(&self, name: &StackName, approved: &[ServiceApproval]) -> ToolResult {
    let _one_at_a_time = self.state.starting.lock();
    let stack = match self.stack(name) {
      Ok(stack) => stack,
      Err(refusal) => return ToolResult::unavailable(&refusal),
    };
    let started = self.services_of((Some(approved), &Origin::agent(Some(name))), |services| {
      StartPlan::for_stack(services, &stack).map_err(|error| error.to_string())
    });
    let plan = match started {
      Ok(plan) => plan,
      Err(refusal) => return ToolResult::unavailable(&format!("the stack `{name}` was not started fully: {refusal}")),
    };
    let running = self.state.running.lock();
    let mut answered = Map::new();
    for service in plan.steps().iter().flatten() {
      if let Some(started) = running.get(service) {
        answered.insert(service.to_string(), started.address());
      }
    }
    ToolResult::shown(json!({ "services": answered }).to_string())
  }

  /// The published stack `name`.
  fn stack(&self, name: &StackName) -> Result<Stack, String> {
    let stacks =
      published_stacks(&self.root).map_err(|error| format!("the project's stacks cannot be read: {error}"))?;
    stacks
      .into_iter()
      .find(|stack| stack.name() == name)
      .ok_or_else(|| format!("the project publishes no stack named `{name}`"))
  }

  /// Starts what a test suite needs, `needs`, in order: each a published stack, whose services start as
  /// [`Self::start_stack`] starts them, or a published service with what it needs. The services it started, which
  /// the invocation did not run before, so [`Self::stop_started`] stops them after the suite; on an error it stops
  /// them itself. Blocks while an earlier start of the invocation runs.
  pub(in crate::execution::step) fn start_needs(&self, needs: &[Slug]) -> Result<Vec<ServiceName>, String> {
    let _one_at_a_time = self.state.starting.lock();
    let before: Vec<ServiceName> = self.state.running.lock().keys().cloned().collect();
    let stacks =
      published_stacks(&self.root).map_err(|error| format!("the project's stacks cannot be read: {error}"))?;
    let mut started = Ok(());
    for need in needs.iter().map(Slug::as_str) {
      let stack = stacks.iter().find(|stack| stack.name().as_str() == need);
      // A suite's run asks at every call, naming the suite: what it needs starts as published.
      started = match stack {
        Some(stack) => self
          .services_of((None, &Origin::agent(Some(stack.name()))), |services| {
            StartPlan::for_stack(services, stack).map_err(|error| error.to_string())
          })
          .map_err(|why| format!("the stack `{need}` the suite needs was not started fully: {why}")),
        None => ServiceName::new(need)
          .map_err(|error| error.to_string())
          .and_then(|name| {
            self.services_of((None, &Origin::agent(None)), |services| {
              StartPlan::for_service(services, &name).map_err(|error| {
                format!("the suite needs `{need}`, which cannot be started as a service or a stack: {error}")
              })
            })
          }),
      }
      .map(|_| ());
      if started.is_err() {
        break;
      }
    }
    let new: Vec<ServiceName> = self
      .state
      .running
      .lock()
      .keys()
      .filter(|name| !before.contains(name))
      .cloned()
      .collect();
    match started {
      Ok(()) => Ok(new),
      Err(why) => {
        self.stop_started(&new);
        Err(why)
      }
    }
  }

  /// Stops the services `started` named that still run, as [`Self::stop`] stops one.
  pub(in crate::execution::step) fn stop_started(&self, started: &[ServiceName]) {
    for name in started {
      let stopped = self.stop(name);
      if stopped.is_error() {
        tracing::warn!(service = %name, why = stopped.text(), "a service a test suite needed did not stop");
      }
    }
  }

  /// Reads the published services, orders them with `plan` and starts that plan, each service noted with `origin`; the
  /// plan once it is up. With `approved`, a service whose definition it does not name is refused before anything
  /// starts.
  fn services_of(
    &self,
    (approved, origin): (Option<&[ServiceApproval]>, &Origin),
    plan: impl FnOnce(&[Service]) -> Result<StartPlan, String>,
  ) -> Result<StartPlan, String> {
    self.may_start()?;
    let services = published(&self.root).map_err(|error| format!("the project's services cannot be read: {error}"))?;
    let plan = plan(&services)?;
    let mut steps = Vec::with_capacity(plan.steps().len());
    for step in plan.steps() {
      let mut definitions = Vec::with_capacity(step.len());
      for name in step {
        let service = services
          .iter()
          .find(|service| service.name() == name)
          .ok_or_else(|| format!("the project publishes no service named `{name}`"))?;
        if let Some(approved) = approved
          && !approved.contains(&ServiceApproval::of(service))
        {
          return Err(format!(
            "the service `{name}` is not the definition this start's approval named: it was published again since; \
             start it again to be asked about it"
          ));
        }
        definitions.push(service);
      }
      steps.push(definitions);
    }
    self.bring_up(&steps, origin)?;
    Ok(plan)
  }

  /// Starts the services of `steps` the invocation does not run yet, step by step, those of one step together, each
  /// noted with `origin`.
  fn bring_up(&self, steps: &[Vec<&Service>], origin: &Origin) -> Result<(), String> {
    for (at, step) in steps.iter().enumerate() {
      self.may_start()?;
      let (wired, missing) = {
        let running = self.state.running.lock();
        let wired: Vec<Address> = running
          .iter()
          .map(|(name, running)| (name.clone(), running.lease.port(), running.url.clone()))
          .collect();
        let missing: Vec<&Service> = step
          .iter()
          .copied()
          .filter(|service| !running.contains_key(service.name()))
          .collect();
        (wired, missing)
      };
      let started: Vec<(&ServiceName, Result<Running, String>)> = thread::scope(|scope| {
        let starts: Vec<_> = missing
          .iter()
          .map(|service| {
            let wired = &wired;
            (
              service.name(),
              scope.spawn(move || self.launch(service, (wired, origin), &|| self.state.ended())),
            )
          })
          .collect();
        starts
          .into_iter()
          .map(|(name, start)| {
            let outcome = start
              .join()
              .unwrap_or_else(|_| Err(format!("the start of the service `{name}` ended without an answer")));
            (name, outcome)
          })
          .collect()
      });
      let mut failed = None;
      for (name, outcome) in started {
        let kept = outcome.and_then(|running| self.keep(name, running));
        if let Err(why) = kept {
          failed.get_or_insert((name, why));
        }
      }
      if let Some((name, why)) = failed {
        let running = self.state.running.lock();
        let skipped: Vec<&ServiceName> = steps[at + 1..]
          .iter()
          .flatten()
          .map(|service| service.name())
          .filter(|later| !running.contains_key(*later))
          .collect();
        return Err(stopped_chain(name, &why, &skipped));
      }
    }
    Ok(())
  }
}

/// The error of a start that stopped at the service `failed`, which failed as `why` says: it names the services after
/// it that were not started, `skipped`.
fn stopped_chain(failed: &ServiceName, why: &str, skipped: &[&ServiceName]) -> String {
  if skipped.is_empty() {
    return why.to_owned();
  }
  let skipped: Vec<String> = skipped.iter().map(|name| format!("`{name}`")).collect();
  format!(
    "the service `{failed}` did not start, so nothing after it was started ({}): {why}",
    skipped.join(", ")
  )
}
