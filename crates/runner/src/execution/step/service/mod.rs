//! `start_service` and `stop_service` (`docs/architecture/agent-testing.md`, "Service" and "Agent tools"): the runner
//! starts one of the project's published services for an invocation, in the run's worktree and the runner's own
//! process tree, without a shell ([`launch`]), on a free loopback port no other service of the runner holds
//! ([`port`]), and waits until it is ready as the service defines ([`readiness`]).
//!
//! A service lives for the step that started it, as a QA app does ([`super::qa_app`]): when the invocation ends
//! ([`Services::end`]) — its turn is over, its run stopped, paused or failed — and while it waits for a person inside
//! its turn ([`Services::release`]), each of its services is stopped with everything it started, off the async
//! workers, and its port freed. A start that is still waiting for readiness when its invocation ends gives up and
//! stops what it started.
//!
//! Not here yet: needs, seeds and per-run data (the start refuses a service that has them), its logs and crash
//! events, the computer's service slots and approval rules of their own; until then a start asks a person as any
//! program run does.

mod launch;
mod port;
mod readiness;
mod task;

use std::{
  collections::{BTreeMap, HashMap},
  ffi::OsString,
  path::PathBuf,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  thread,
};

use orchester_core::{
  id::{InvocationId, RunId},
  orchester_tools::ToolResult,
  resource::CommandEnvironment,
  secret::SecretVault,
  service::ServiceName,
};
use orchester_exec::process_tree::ProcessTree;
use parking_lot::Mutex;
use serde_json::json;

use self::port::{PortLease, Ports};

/// The services of the runner's invocations that have not ended, and the ports they hold.
#[derive(Default)]
pub(in crate::execution) struct Services {
  ports: Arc<Ports>,
  invocations: Mutex<HashMap<InvocationId, Arc<InvocationServices>>>,
}

/// One invocation's services: its starts one at a time, the services it keeps, and whether it ended or waits for a
/// person. Its tools hold it as long as they live; the runner forgets it once the invocation ended.
#[derive(Default)]
pub(in crate::execution::step) struct InvocationServices {
  ended: AtomicBool,
  waiting: AtomicBool,
  starting: Mutex<()>,
  running: Mutex<BTreeMap<ServiceName, Running>>,
}

impl InvocationServices {
  fn ended(&self) -> bool {
    self.ended.load(Ordering::Acquire)
  }

  fn waiting(&self) -> bool {
    self.waiting.load(Ordering::Acquire)
  }
}

/// A service at work. Its tree is stopped before its port is freed (fields drop in order).
struct Running {
  tree: ProcessTree,
  url: String,
  lease: PortLease,
}

impl Running {
  fn answer(&self) -> ToolResult {
    ToolResult::shown(json!({"url": self.url, "port": self.lease.port()}).to_string())
  }
}

impl Services {
  /// The services of `invocation`, which is starting.
  fn register(&self, invocation: InvocationId) -> Arc<InvocationServices> {
    Arc::clone(self.invocations.lock().entry(invocation).or_default())
  }

  /// `invocation` ended: its services stop and free their ports, and a start of its still under way gives up.
  pub(in crate::execution) fn end(&self, invocation: InvocationId) {
    let Some(state) = self.invocations.lock().remove(&invocation) else {
      return;
    };
    state.ended.store(true, Ordering::Release);
    stop_aside(std::mem::take(&mut *state.running.lock()));
  }

  /// `invocation` waits for a person inside its turn: its services stop meanwhile, also one a start of its still
  /// under way would keep.
  pub(in crate::execution) fn release(&self, invocation: InvocationId) {
    let state = self.invocations.lock().get(&invocation).cloned();
    if let Some(state) = state {
      let mut running = state.running.lock();
      state.waiting.store(true, Ordering::Release);
      stop_aside(std::mem::take(&mut *running));
    }
  }

  /// `invocation`'s wait for a person ended: its starts keep their services again.
  pub(in crate::execution) fn resume(&self, invocation: InvocationId) {
    let state = self.invocations.lock().get(&invocation).cloned();
    if let Some(state) = state {
      state.waiting.store(false, Ordering::Release);
    }
  }
}

/// Stops `running` on a thread of its own: ending a tree waits for its processes, which no async worker does.
fn stop_aside(running: BTreeMap<ServiceName, Running>) {
  if !running.is_empty() {
    thread::spawn(move || drop(running));
  }
}

/// The service tools of one invocation.
pub(in crate::execution::step) struct StepServices {
  services: Arc<Services>,
  state: Arc<InvocationServices>,
  /// The project's repository, whose database holds the published services and the secrets' names.
  root: PathBuf,
  worktree: PathBuf,
  run: RunId,
  vault: Arc<dyn SecretVault>,
  /// The environment the runner runs in, whose agent part every service gets, and what the project adds.
  inherited: Vec<(OsString, OsString)>,
  project: CommandEnvironment,
}

impl StepServices {
  /// Starts the published service `name` for the invocation and waits until it is ready; one it runs already is
  /// answered as it is. Blocks while an earlier start of the invocation runs and until the service is ready.
  pub(in crate::execution::step) fn start(&self, name: &ServiceName) -> ToolResult {
    let _one_at_a_time = self.state.starting.lock();
    if let Err(refusal) = self.may_start() {
      return ToolResult::unavailable(&refusal);
    }
    if let Some(running) = self.state.running.lock().get(name) {
      return running.answer();
    }
    let state = Arc::clone(&self.state);
    match self.launch(name, &move || state.ended()) {
      Ok(running) => self.keep(name, running),
      Err(refusal) => ToolResult::unavailable(&refusal),
    }
  }

  /// Stops the service `name` the invocation started, with everything it started; says so when none runs.
  pub(in crate::execution::step) fn stop(&self, name: &ServiceName) -> ToolResult {
    let Some(running) = self.state.running.lock().remove(name) else {
      return ToolResult::shown(format!("the service `{name}` is not running in this step"));
    };
    let port = running.lease.port();
    match running.tree.stop() {
      Ok(()) => ToolResult::shown(format!(
        "the service `{name}` was stopped with everything it started; port {port} is free"
      )),
      Err(error) => ToolResult::unavailable(&format!(
        "the service `{name}` was asked to stop, but it may not have ended: {error}"
      )),
    }
  }

  fn may_start(&self) -> Result<(), String> {
    if self.state.ended() {
      return Err("the turn ended, so no service is started".to_owned());
    }
    if self.state.waiting() {
      return Err(
        "the turn waits for a person, so no service is started meanwhile; call start_service again once you go on"
          .to_owned(),
      );
    }
    Ok(())
  }

  /// Keeps `running` for the invocation, or stops it when the invocation ended or began to wait for a person while it
  /// started.
  fn keep(&self, name: &ServiceName, running: Running) -> ToolResult {
    let mut kept = self.state.running.lock();
    // Checked under the services' lock, which `Services::release` takes too: a wait that began during the start sees
    // the service stored, or the start sees the wait.
    if let Err(refusal) = self.may_start() {
      drop(kept);
      drop(running);
      return ToolResult::unavailable(&format!("the service `{name}` was ready but stopped again: {refusal}"));
    }
    let answer = running.answer();
    kept.insert(name.clone(), running);
    answer
  }
}
