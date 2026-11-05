//! Starting a service or a stack with what it needs (`docs/architecture/agent-testing.md`, "Service" and "Stack"):
//! the core orders the needs into steps ([`StartPlan`]); the services of one step start together, each on a thread of
//! its own, and a step starts only once every service of the steps before it is ready, so each service starts after
//! its needs are ready. A service the invocation runs already is reused, not started again. Each start's
//! `{{services.<name>.url|port}}` are filled from the services the invocation runs by then. When a service fails,
//! the services of its step that became ready are kept, nothing after it starts, and the error names it. Before
//! anything starts, the start takes a service slot for each service it will start ([`super::slots`]), waiting while
//! too few are free; its answer says how long it waited.
//!
//! A start's request names every service it runs ([`StepServices::service_approvals`]), and the start runs only those
//! definitions: a service published again since, with another fingerprint, is refused until it is asked about. A
//! person's Restart in the Services tab runs only definitions the invocation's starts ran before.

use std::thread;

use orchester_core::{
  orchester_tools::ToolResult,
  resource::Slug,
  service::{Service, ServiceApproval, ServiceName, ServiceStartFailure, Stack, StackName, StartPlan},
};
use orchester_ipc::services::ServiceStarter;
use serde_json::{Map, json};

use super::{
  Failed, Origin, Running, ServiceSlot, StepServices, Waited,
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
  /// the invocation runs, and while too few service slots are free; the answer says how long it waited for them.
  pub(in crate::execution::step) fn start(&self, name: &ServiceName, approved: &[ServiceApproval]) -> ToolResult {
    let _one_at_a_time = self.state.starting.lock();
    let started = self.services_of((Some(approved), &Origin::agent(None)), |services| {
      StartPlan::for_service(services, name).map_err(|error| format!("the service `{name}` cannot be started: {error}"))
    });
    let waited = match started {
      Ok((_, waited)) => waited,
      Err(refusal) => return ToolResult::unavailable(&refusal),
    };
    match self.state.running.lock().get(name) {
      Some(running) => {
        let mut address = running.address();
        if let Some(waited) = waited {
          address["waited_for_slots"] = waited.answer();
        }
        ToolResult::shown(address.to_string())
      }
      None => ToolResult::unavailable(&format!("the service `{name}` was started but no longer runs")),
    }
  }

  /// A person's Restart or Start in the Services tab: stops the service `name` when it runs, then starts it as
  /// published with what it needs, and waits until it is ready. It runs only the definitions the invocation's starts
  /// ran before, as their requests named them: a service published again since, or a need it did not have then, is
  /// refused before anything stops, as the agent's `start_service` refuses it, so a press never approves a definition
  /// nobody was shown. A start of the agent's may be under way meanwhile, waiting for another service: the person's
  /// does not wait for it, only for an earlier one of the person's. A service both bring up at once is kept once
  /// ([`StepServices::keep`]).
  pub(super) fn restart_for_person(&self, name: &ServiceName) -> Result<(), String> {
    let _one_at_a_time = self.state.restarting.lock();
    let ran = self.state.ran.lock().clone();
    // Checked before the stop too, so a refused Restart leaves the service running.
    if let Some(new) = self
      .service_approvals(Started::Service(name))?
      .iter()
      .find(|approval| !ran.contains(approval))
    {
      return Err(not_ran(&new.name));
    }
    let stopped = self.stop(name);
    if stopped.is_error() {
      return Err(stopped.text().to_owned());
    }
    let person = Origin {
      stack: None,
      by: ServiceStarter::Person,
    };
    self
      .services_of((Some(&ran), &person), |services| {
        StartPlan::for_service(services, name)
          .map_err(|error| format!("the service `{name}` cannot be started: {error}"))
      })
      .map(|_| ())
  }

  /// Starts every service of the published stack `name` with what they need, in order, and answers each one's URL
  /// and port; a service the invocation runs already is reused. Runs only the definitions `approved` names. Blocks
  /// while an earlier start of the invocation runs, and while too few service slots are free for all of them.
  pub(in crate::execution::step) fn start_stack(&self, name: &StackName, approved: &[ServiceApproval]) -> ToolResult {
    let _one_at_a_time = self.state.starting.lock();
    let stack = match self.stack(name) {
      Ok(stack) => stack,
      Err(refusal) => return ToolResult::unavailable(&refusal),
    };
    let started = self.services_of((Some(approved), &Origin::agent(Some(name))), |services| {
      StartPlan::for_stack(services, &stack).map_err(|error| error.to_string())
    });
    let (plan, waited) = match started {
      Ok(started) => started,
      Err(refusal) => return ToolResult::unavailable(&format!("the stack `{name}` was not started fully: {refusal}")),
    };
    let running = self.state.running.lock();
    let mut answered = Map::new();
    for service in plan.steps().iter().flatten() {
      if let Some(started) = running.get(service) {
        answered.insert(service.to_string(), started.address());
      }
    }
    let mut answer = json!({ "services": answered });
    if let Some(waited) = waited {
      answer["waited_for_slots"] = waited.answer();
    }
    ToolResult::shown(answer.to_string())
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
  /// plan once it is up, and what it waited for service slots when it did. With `approved`, a service whose definition
  /// it does not name is refused before anything starts.
  fn services_of(
    &self,
    (approved, origin): (Option<&[ServiceApproval]>, &Origin),
    plan: impl FnOnce(&[Service]) -> Result<StartPlan, String>,
  ) -> Result<(StartPlan, Option<Waited>), String> {
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
          return Err(match origin.by {
            ServiceStarter::Agent => format!(
              "the service `{name}` is not the definition this start's approval named: it was published again \
               since; start it again to be asked about it"
            ),
            ServiceStarter::Person => not_ran(name.as_str()),
          });
        }
        definitions.push(service);
      }
      steps.push(definitions);
    }
    self.note_ran(&steps);
    let waited = self.bring_up(&steps, origin)?;
    Ok((plan, waited))
  }

  /// Notes the definitions of `steps` as ones the invocation's starts ran, approved as their start's request named
  /// them, so a person's Restart may run them again.
  fn note_ran(&self, steps: &[Vec<&Service>]) {
    let mut ran = self.state.ran.lock();
    for approval in steps.iter().flatten().map(|service| ServiceApproval::of(service)) {
      if !ran.contains(&approval) {
        ran.push(approval);
      }
    }
  }

  /// Starts the services of `steps` the invocation does not run yet, step by step, those of one step together, each
  /// noted with `origin`, once it holds a service slot for each of them; what it waited for the slots, when it did. The
  /// slots of services that did not start are freed. A failure the person can act on is noted in the run's
  /// conversation, with the services not started because of it and those the step still runs.
  fn bring_up(&self, steps: &[Vec<&Service>], origin: &Origin) -> Result<Option<Waited>, String> {
    let needed = {
      let running = self.state.running.lock();
      steps
        .iter()
        .flatten()
        .filter(|service| !running.contains_key(service.name()))
        .count()
    };
    let needed = u32::try_from(needed).map_err(|_| "a start cannot start that many services".to_owned())?;
    let (mut slots, waited) = self.take_slots(needed)?;
    for (at, step) in steps.iter().enumerate() {
      self.may_start()?;
      if let Err((name, failed)) = self.bring_up_step(step, (&mut slots, origin)) {
        let running = self.state.running.lock();
        let skipped: Vec<&ServiceName> = steps[at + 1..]
          .iter()
          .flatten()
          .map(|service| service.name())
          .filter(|later| !running.contains_key(*later))
          .collect();
        if let Some((cause, lines)) = failed.shown {
          (self.start_failed)(ServiceStartFailure {
            service: name.to_string(),
            cause,
            lines,
            not_started: skipped.iter().map(ToString::to_string).collect(),
            running: running.keys().map(ToString::to_string).collect(),
          });
        }
        return Err(stopped_chain(name, &failed.why, &skipped));
      }
    }
    Ok(waited)
  }

  /// Starts the services of `step` the invocation does not run yet, together, each with one of `slots` and noted with
  /// `origin`; the first that failed and why.
  fn bring_up_step<'a>(
    &self,
    step: &[&'a Service],
    (slots, origin): (&mut Vec<ServiceSlot>, &Origin),
  ) -> Result<(), (&'a ServiceName, Failed)> {
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
    let mut given = slots.split_off(slots.len().saturating_sub(missing.len()));
    if let Some(first) = missing.get(given.len()) {
      // A service counted as running when the slots were taken stopped since, as one a crash stopped: it takes a
      // slot of its own now.
      let short = u32::try_from(missing.len() - given.len()).unwrap_or(u32::MAX);
      let (more, _) = self.take_slots(short).map_err(|why| (first.name(), Failed::from(why)))?;
      given.extend(more);
    }
    let started: Vec<(&ServiceName, Result<Running, Failed>)> = thread::scope(|scope| {
      let starts: Vec<_> = missing
        .iter()
        .zip(given)
        .map(|(service, slot)| {
          let wired = &wired;
          (
            service.name(),
            scope.spawn(move || self.launch(service, (wired, origin, slot), &|| self.state.ended())),
          )
        })
        .collect();
      starts
        .into_iter()
        .map(|(name, start)| {
          let outcome = start.join().unwrap_or_else(|_| {
            Err(format!("the start of the service `{name}` ended without an answer").into())
          });
          (name, outcome)
        })
        .collect()
    });
    let mut failed = None;
    for (name, outcome) in started {
      let kept = outcome.and_then(|running| self.keep(name, running).map_err(Failed::from));
      if let Err(why) = kept {
        failed.get_or_insert((name, why));
      }
    }
    failed.map_or(Ok(()), Err)
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

/// Why a person's Restart does not start the service `name`: this step never ran it as it is published now.
fn not_ran(name: &str) -> String {
  format!(
    "`{name}` is published differently from when this step's agent started it, or this step never ran it: a Restart \
     here starts only what an approved start ran. The agent starts it again with start_service, which asks about it"
  )
}
