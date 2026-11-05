//! Starting a service or a stack with what it needs (`docs/architecture/agent-testing.md`, "Service" and "Stack"):
//! the core orders the needs into steps ([`StartPlan`]); the services of one step start together, each on a thread of
//! its own, and a step starts only once every service of the steps before it is ready, so each service starts after
//! its needs are ready. A service the invocation runs already is reused, not started again. Each start's
//! `{{services.<name>.url|port}}` are filled from the services the invocation runs by then. When a service fails,
//! the services of its step that became ready are kept, nothing after it starts, and the error names it.

use std::thread;

use orchester_core::{
  orchester_tools::ToolResult,
  service::{RunData, Seed, Service, ServiceName, StackName, StartPlan},
};
use serde_json::{Map, json};

use super::{
  Running, StepServices,
  launch::{published, published_stacks},
};

/// Where a service the invocation runs listens: its name, port and URL.
pub(super) type Address = (ServiceName, u16, String);

impl StepServices {
  /// Starts the published service `name` with what it needs, in order, and waits until it is ready; a service the
  /// invocation runs already is reused. Blocks while an earlier start of the invocation runs.
  pub(in crate::execution::step) fn start(&self, name: &ServiceName) -> ToolResult {
    let _one_at_a_time = self.state.starting.lock();
    let started = self.services_of(|services| {
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

  /// Starts every service of the published stack `name` with what they need, in order, and answers each one's URL
  /// and port; a service the invocation runs already is reused. Blocks while an earlier start of the invocation runs.
  pub(in crate::execution::step) fn start_stack(&self, name: &StackName) -> ToolResult {
    let _one_at_a_time = self.state.starting.lock();
    let stacks = match published_stacks(&self.root) {
      Ok(stacks) => stacks,
      Err(error) => return ToolResult::unavailable(&format!("the project's stacks cannot be read: {error}")),
    };
    let Some(stack) = stacks.into_iter().find(|stack| stack.name() == name) else {
      return ToolResult::unavailable(&format!("the project publishes no stack named `{name}`"));
    };
    let started =
      self.services_of(|services| StartPlan::for_stack(services, &stack).map_err(|error| error.to_string()));
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

  /// Reads the published services, orders them with `plan` and starts that plan; the plan once it is up.
  fn services_of(&self, plan: impl FnOnce(&[Service]) -> Result<StartPlan, String>) -> Result<StartPlan, String> {
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
        startable(service)?;
        definitions.push(service);
      }
      steps.push(definitions);
    }
    self.bring_up(&steps)?;
    Ok(plan)
  }

  /// Starts the services of `steps` the invocation does not run yet, step by step, those of one step together.
  fn bring_up(&self, steps: &[Vec<&Service>]) -> Result<(), String> {
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
              scope.spawn(move || self.launch(service, wired, &|| self.state.ended())),
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

/// Whether this step can start `service`: one without seed or per-run data.
fn startable(service: &Service) -> Result<(), String> {
  let unsupported = if !matches!(service.seed(), Seed::None) {
    Some("has a seed")
  } else if !matches!(service.run_data(), RunData::None) {
    Some("keeps data of its own per run")
  } else {
    None
  };
  match unsupported {
    Some(what) => Err(format!(
      "the service `{}` {what}, which Orchester cannot start for a run yet",
      service.name()
    )),
    None => Ok(()),
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
