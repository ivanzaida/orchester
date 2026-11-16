//! A run's services as the task page's Services tab shows them (ORC-265, `cjvRr`): every service a step of the run at
//! work started, also one that stopped since, in the order they started; a service's latest log lines, their secret
//! values replaced as its log keeps them; and the person's Stop and Restart, carried out by the service tools of the
//! invocation that started it, as `stop_service` and `start_service` are. A run whose steps ended has none: its
//! services stopped with them.

use orchester_core::{
  id::{InvocationId, RunId},
  service::{LogStream, ServiceName, ServiceState},
};
use orchester_ipc::services::{RunService, RunServiceState, ServiceLogLine};

use super::{Services, StepServices, Watch};

/// Why a request about a run's service cannot be carried out.
#[derive(Debug, thiserror::Error)]
pub(in crate::execution) enum ShownServiceError {
  #[error("`{0}` is not a service name")]
  Name(String),
  #[error("no step of the run at work started the service `{0}`")]
  NotStarted(String),
  #[error("{0}")]
  Failed(String),
}

impl Services {
  /// The service tools of the run's invocations that have not ended, in the order the invocations started.
  fn tools_of(&self, run: RunId) -> Vec<StepServices> {
    let mut tools: Vec<(InvocationId, StepServices)> = self
      .tools
      .lock()
      .iter()
      .filter(|(_, tools)| tools.run == run)
      .map(|(invocation, tools)| (*invocation, tools.clone()))
      .collect();
    tools.sort_by_key(|(invocation, _)| *invocation);
    tools.into_iter().map(|(_, tools)| tools).collect()
  }

  /// Keeps `tools`, the service tools of `invocation`, for the Services tab until the invocation ends.
  pub(in crate::execution::executor::step) fn keep_tools(&self, invocation: InvocationId, tools: StepServices) {
    self.tools.lock().insert(invocation, tools);
  }

  /// The tools of the run's invocation that started the service `name`.
  fn tools_with(&self, run: RunId, name: &str) -> Result<(StepServices, ServiceName), ShownServiceError> {
    self.tools_found(run, name, |tools, service| {
      tools.state.watched.lock().contains_key(service)
    })
  }

  /// The tools of the run's invocation that started the service `name`, or whose start ran it and failed before its
  /// program started, as one whose Docker was not available: a person's Restart, or Retry, starts it again there.
  fn tools_that_ran(&self, run: RunId, name: &str) -> Result<(StepServices, ServiceName), ShownServiceError> {
    self.tools_found(run, name, |tools, service| {
      tools.state.watched.lock().contains_key(service)
        || tools
          .state
          .ran
          .lock()
          .iter()
          .any(|approval| approval.name == service.as_str())
    })
  }

  /// The tools of the run's first invocation that `found` says knows the service `name`.
  fn tools_found(
    &self,
    run: RunId,
    name: &str,
    found: impl Fn(&StepServices, &ServiceName) -> bool,
  ) -> Result<(StepServices, ServiceName), ShownServiceError> {
    let service = ServiceName::new(name).map_err(|_| ShownServiceError::Name(name.to_owned()))?;
    self
      .tools_of(run)
      .into_iter()
      .find(|tools| found(tools, &service))
      .map(|tools| (tools, service))
      .ok_or_else(|| ShownServiceError::NotStarted(name.to_owned()))
  }

  /// The services the steps of `run` at work started, in the order they started.
  pub(in crate::execution) fn of_run(&self, run: RunId) -> Vec<RunService> {
    let mut shown: Vec<RunService> = Vec::new();
    for tools in self.tools_of(run) {
      let watched = tools.state.watched.lock();
      shown.extend(watched.iter().map(|(name, watch)| shown_service(name, watch)));
    }
    shown.sort_by_key(|service| service.started_at);
    shown
  }

  /// The kept lines of the service `name` of `run` after line `since`, at most the last `tail`, and the number of the
  /// latest line it wrote.
  pub(in crate::execution) fn log_of(
    &self,
    run: RunId,
    name: &str,
    (since, tail): (u64, u32),
  ) -> Result<(Vec<ServiceLogLine>, u64), ShownServiceError> {
    let (tools, service) = self.tools_with(run, name)?;
    let log = tools
      .state
      .watched
      .lock()
      .get(&service)
      .map(|watch| std::sync::Arc::clone(&watch.log))
      .ok_or_else(|| ShownServiceError::NotStarted(name.to_owned()))?;
    let log = log.lock();
    let mut lines: Vec<ServiceLogLine> = log
      .lines()
      .filter(|line| line.number() > since)
      .map(|line| ServiceLogLine {
        number: line.number(),
        at: line.at(),
        stderr: line.stream() == LogStream::Stderr,
        text: line.text().to_owned(),
      })
      .collect();
    lines.drain(..lines.len().saturating_sub(tail as usize));
    Ok((lines, log.last_number()))
  }

  /// Stops the service `name` of `run` with everything it started, as `stop_service` does. Blocks until it ended.
  pub(in crate::execution) fn stop_of(&self, run: RunId, name: &str) -> Result<(), ShownServiceError> {
    let (tools, service) = self.tools_with(run, name)?;
    let stopped = tools.stop(&service);
    if stopped.is_error() {
      return Err(ShownServiceError::Failed(stopped.text().to_owned()));
    }
    Ok(())
  }

  /// Starts the service `name` of `run` again, stopping it first when it runs, also one whose start failed before its
  /// program started (the no-Docker card's Retry). Blocks until it is ready.
  pub(in crate::execution) fn restart_of(&self, run: RunId, name: &str) -> Result<(), ShownServiceError> {
    let (tools, service) = self.tools_that_ran(run, name)?;
    tools.restart_for_person(&service).map_err(ShownServiceError::Failed)
  }
}

/// The service `name` as the Services tab shows it; a crash shows once the crash watcher saw it ([`super::crash`]).
fn shown_service(name: &ServiceName, watch: &Watch) -> RunService {
  let state = match ServiceState::of(watch.phase, watch.exit) {
    ServiceState::Starting => RunServiceState::Starting,
    ServiceState::Ready => RunServiceState::Ready,
    ServiceState::Crashed { code } => RunServiceState::Crashed { code },
    ServiceState::Stopped => RunServiceState::Stopped,
  };
  RunService {
    name: name.as_str().to_owned(),
    state,
    port: watch.port,
    url: watch.url.clone(),
    started_at: watch.started_at,
    ended_at: watch.ended_at,
    stack: watch.origin.stack.as_ref().map(|stack| stack.as_str().to_owned()),
    started_by: watch.origin.by,
    stopped_for: watch.stopped_for.as_ref().map(|need| need.as_str().to_owned()),
  }
}
