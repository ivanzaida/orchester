//! `start_service`, `start_stack`, `stop_service`, `service_status` and `service_logs`
//! (`docs/architecture/agent-testing.md`, "Service", "Stack" and "Agent tools"): the runner starts one of the project's
//! published services, or a stack's, with what they need in order ([`order`]), for an invocation, in the run's worktree
//! and the runner's own process tree, without a shell ([`launch`]), on a free loopback port no other service of the
//! runner holds ([`port`]), and waits until each is ready as the service defines ([`readiness`]). Its output goes into
//! a bounded log with its secret values redacted ([`orchester_core::service::ServiceLog`]); a start that fails says its
//! last lines, and the step reads the log and the service's status while the invocation lasts, also after it stopped.
//!
//! A service lives for the step that started it, as a QA app does ([`super::qa_app`]): when the invocation ends
//! ([`Services::end`]) — its turn is over, its run stopped, paused or failed — and while it waits for a person inside
//! its turn ([`Services::release`]), each of its services is stopped with everything it started, off the async
//! workers, and its port freed. A start that is still waiting for readiness when its invocation ends gives up and
//! stops what it started.
//!
//! Once a service is ready, its seed runs once before its start answers ([`seed`]); what the seed's requests captured
//! comes back with its URL and port.
//!
//! A service's per-run data is prepared before it starts ([`data`]): its run folder in the run's storage, kept until
//! the run ends (`crate::execution::run_data`), or a fresh database container that lives as long as the service.
//!
//! A ready service that exits on its own crashed ([`crash`]): the services that need it stop with it, the agent reads a
//! notice with its next Orchester tool answer, and the run's conversation records it.
//!
//! A start asks a person once for everything it runs, unless the project's rules approve every service of it
//! ([`order`], `orchester_core::run::ProjectScope`).
//!
//! Each running service holds one of the computer's service slots within its project's share ([`slots`]), freed when
//! it stops, also while its invocation waits for a person; a start takes the slots of every service it starts first,
//! waiting while too few are free, and a test suite's own process takes one while it runs
//! ([`StepServices::suite_slot`]).

mod crash;
mod data;
mod launch;
mod order;
mod port;
mod readiness;
mod seed;
mod slots;
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
  time::Instant,
};

use orchester_core::{
  id::{InvocationId, ProjectId, RunId},
  orchester_tools::ToolResult,
  resource::CommandEnvironment,
  secret::SecretVault,
  service::{ServiceName, ServicePhase, ServiceState, ServiceStatus},
};
use orchester_exec::process_tree::ProcessTree;
use parking_lot::Mutex;
use serde_json::json;

pub use self::slots::ServiceSlots;
pub(in crate::execution::step) use self::{
  launch::{project_secret, published as published_services, published_stacks},
  order::Started,
  slots::{ServiceSlot, Waited},
};
use self::{
  port::{PortLease, Ports},
  readiness::SharedLog,
  slots::{Asker, Waiting},
};

/// The services of the runner's invocations that have not ended, the ports they hold, and the computer's service
/// slots they take.
pub(in crate::execution) struct Services {
  ports: Arc<Ports>,
  slots: ServiceSlots,
  invocations: Mutex<HashMap<InvocationId, Arc<InvocationServices>>>,
}

/// One invocation's services: its starts one at a time, the services it keeps, what it knows of each service it
/// started, whether it ended or waits for a person, and the crash notices its agent has not read yet. Its tools hold
/// it as long as they live; the runner forgets it once the invocation ended. Whoever takes both locks takes `running`
/// first.
#[derive(Default)]
pub(in crate::execution::step) struct InvocationServices {
  ended: AtomicBool,
  waiting: AtomicBool,
  /// Set once the watcher of its crashes started ([`crash`]).
  watching: AtomicBool,
  starting: Mutex<()>,
  running: Mutex<BTreeMap<ServiceName, Running>>,
  watched: Mutex<BTreeMap<ServiceName, Watch>>,
  notices: Mutex<Vec<String>>,
}

impl InvocationServices {
  fn ended(&self) -> bool {
    self.ended.load(Ordering::Acquire)
  }

  fn waiting(&self) -> bool {
    self.waiting.load(Ordering::Acquire)
  }

  /// Notes that the service `name` reached `phase`, and how its program exited when `exit` says it was seen to.
  fn note(&self, name: &ServiceName, phase: ServicePhase, exit: Option<Option<i32>>) {
    if let Some(watch) = self.watched.lock().get_mut(name) {
      watch.phase = phase;
      if exit.is_some() {
        watch.exit = exit;
      }
    }
  }
}

/// A service at work. Its tree is stopped, then its container removed, before its port and then its service slot are
/// freed (fields drop in order).
struct Running {
  tree: ProcessTree,
  /// The container it is, when its per-run data is a database container: held to be removed when it drops.
  _container: Option<data::Container>,
  url: String,
  lease: PortLease,
  /// What its seed's requests captured, their secret values replaced.
  captured: seed::Captured,
  /// The services it needs, which a crash of one of them stops it with ([`crash`]).
  needs: Vec<ServiceName>,
  /// The readers of its output, which a crash waits for so that its log holds the last lines.
  output: readiness::Output,
  /// Its slot of the computer's *Services at once*.
  _slot: ServiceSlot,
}

impl Running {
  /// Its URL and port, as a start answers them, and what its seed captured when it captured anything.
  fn address(&self) -> serde_json::Value {
    let mut address = json!({"url": self.url, "port": self.lease.port()});
    if !self.captured.is_empty() {
      address["seed"] = json!(self.captured);
    }
    address
  }

  /// The exit code of its program, once it exited on its own.
  fn exit(&mut self) -> Result<Option<Option<i32>>, String> {
    match self.tree.exited() {
      Ok(exited) => Ok(exited.map(|status| status.code())),
      Err(error) => Err(format!("cannot be watched: {error}")),
    }
  }
}

/// What the invocation knows of a service it started, from its start on: kept after it stopped, until the invocation
/// ends or starts it again.
struct Watch {
  log: SharedLog,
  port: u16,
  url: String,
  started: Instant,
  phase: ServicePhase,
  /// The exit code of its program, once it was seen to exit on its own.
  exit: Option<Option<i32>>,
}

impl Watch {
  fn status(&self) -> ServiceStatus {
    ServiceStatus {
      state: ServiceState::of(self.phase, self.exit),
      port: self.port,
      url: self.url.clone(),
      uptime: self.started.elapsed(),
    }
  }
}

impl Services {
  /// No services yet; they take `slots`.
  pub(in crate::execution) fn new(slots: ServiceSlots) -> Self {
    Self {
      ports: Arc::default(),
      slots,
      invocations: Mutex::default(),
    }
  }

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
      let stopped = std::mem::take(&mut *running);
      for name in stopped.keys() {
        state.note(name, ServicePhase::Stopped, None);
      }
      stop_aside(stopped);
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
  /// The project's id, which its containers' labels name.
  project_id: ProjectId,
  worktree: PathBuf,
  run: RunId,
  vault: Arc<dyn SecretVault>,
  /// The environment the runner runs in, whose agent part every service gets, and what the project adds.
  inherited: Vec<(OsString, OsString)>,
  project: CommandEnvironment,
  /// Records a crash in the run's conversation.
  crashed: crash::CrashReport,
  /// Says in the run's conversation that a start waits for service slots.
  waiting: Arc<dyn Fn(Waiting) + Send + Sync>,
}

impl StepServices {
  /// Stops the service `name` the invocation started, with everything it started, and frees its port and its service
  /// slot; says so when none runs.
  pub(in crate::execution::step) fn stop(&self, name: &ServiceName) -> ToolResult {
    let removed = self.state.running.lock().remove(name);
    let Some(mut running) = removed else {
      return ToolResult::shown(format!("the service `{name}` is not running in this step"));
    };
    // A program that cannot be watched any more is stopped all the same; its status then says stopped.
    let exit = running.exit().unwrap_or(None);
    self.state.note(name, ServicePhase::Stopped, exit);
    let port = running.lease.port();
    // Its port and its slot are freed as the rest of it drops, when this answers.
    match running.tree.stop() {
      Ok(()) => ToolResult::shown(format!(
        "the service `{name}` was stopped with everything it started; port {port} and its service slot are free"
      )),
      Err(error) => ToolResult::unavailable(&format!(
        "the service `{name}` was asked to stop, but it may not have ended: {error}"
      )),
    }
  }

  /// `count` of the computer's service slots for this invocation's start, waiting in order while too few are free;
  /// refused when the invocation ends or begins to wait for a person meanwhile, or when it could get them only once
  /// the services it runs, which hold a slot each, stopped.
  fn take_slots(&self, count: u32) -> Result<(Vec<ServiceSlot>, Option<Waited>), String> {
    let holding = u32::try_from(self.state.running.lock().len()).unwrap_or(u32::MAX);
    let asker = Asker {
      project: self.project_id,
      root: &self.root,
      holding,
      gate: &|| self.may_start(),
      waiting: &*self.waiting,
    };
    self.services.slots.take(count, &asker)
  }

  /// The service slot a test suite's own process holds while it runs, taken as a service's is.
  pub(in crate::execution::step) fn suite_slot(&self) -> Result<ServiceSlot, String> {
    let (mut slots, _) = self.take_slots(1)?;
    slots
      .pop()
      .ok_or_else(|| "no service slot was taken for the suite".to_owned())
  }

  /// The status of the service `name` the invocation started, also while it starts and after it stopped.
  pub(in crate::execution::step) fn status(&self, name: &ServiceName) -> ToolResult {
    let mut running = self.state.running.lock();
    let mut watched = self.state.watched.lock();
    let Some(watch) = watched.get_mut(name) else {
      return not_started(name);
    };
    if watch.exit.is_none()
      && let Some(service) = running.get_mut(name)
    {
      match service.exit() {
        Ok(exit) => watch.exit = exit,
        Err(why) => return ToolResult::unavailable(&format!("the service `{name}` {why}")),
      }
    }
    ToolResult::shown(watch.status().answer())
  }

  /// The last `tail` kept lines after line `since` of the service `name` the invocation started.
  pub(in crate::execution::step) fn logs(&self, name: &ServiceName, tail: u32, since: u64) -> ToolResult {
    let log = self.state.watched.lock().get(name).map(|watch| Arc::clone(&watch.log));
    match log {
      Some(log) => ToolResult::shown(log.lock().excerpt(tail, since)),
      None => not_started(name),
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
  fn keep(&self, name: &ServiceName, running: Running) -> Result<(), String> {
    let mut kept = self.state.running.lock();
    // Checked under the services' lock, which `Services::release` takes too: a wait that began during the start sees
    // the service stored, or the start sees the wait.
    if let Err(refusal) = self.may_start() {
      self.state.note(name, ServicePhase::Stopped, None);
      drop(kept);
      drop(running);
      return Err(format!("the service `{name}` was ready but stopped again: {refusal}"));
    }
    self.state.note(name, ServicePhase::Ready, None);
    kept.insert(name.clone(), running);
    drop(kept);
    self.watch_crashes();
    Ok(())
  }

  /// The crash notices the agent has not read yet, which the answer of its next Orchester tool call carries.
  pub(in crate::execution::step) fn take_notices(&self) -> Vec<String> {
    std::mem::take(&mut *self.state.notices.lock())
  }
}

fn not_started(name: &ServiceName) -> ToolResult {
  ToolResult::shown(format!("the service `{name}` was not started in this step"))
}
