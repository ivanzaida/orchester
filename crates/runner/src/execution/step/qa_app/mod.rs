//! `start_qa_app` (`orchester_core::orchester_tools::QaAppStart`): the runner takes the computer's QA slot
//! ([`slots`]), waiting while another run's QA app holds it, runs the worktree's `tools/qa-app.ps1` ([`script`]) and
//! keeps the app it started for the invocation, so `call_run_app` without a pid reaches it through its exact
//! discovery file. Starting again in the same invocation replaces the app and keeps the slot. When the invocation
//! ends, its app is stopped with everything it started inside the worktree and the slot is freed; a start still
//! waiting for the slot gives up.

mod script;
mod slots;
mod task;

use std::{
  collections::{HashMap, HashSet},
  fmt::Write as _,
  num::NonZeroU32,
  path::{Path, PathBuf},
  sync::Arc,
  thread,
  time::{Duration, Instant},
};

use orchester_core::{
  id::InvocationId,
  orchester_tools::{ProcessCall, QaAppStart, StartedQaApp, ToolResult},
  time::Timestamp,
};
use orchester_exec::run_processes::RunProcesses;
use parking_lot::Mutex;

use self::slots::Lease;
pub use self::slots::QaSlots;

/// How often a start that waits for the QA slot tries again.
const SLOT_POLL: Duration = Duration::from_millis(500);

/// The QA apps the runner's steps started, by the invocation that started each, and the computer's QA slots.
pub(in crate::execution) struct QaApps {
  slots: QaSlots,
  apps: Mutex<HashMap<InvocationId, QaApp>>,
  /// Invocations that ended: a start of theirs still under way stops what it started and gives up.
  ended: Mutex<HashSet<InvocationId>>,
}

/// A started QA app with the slot it holds; the app is stopped before the slot is freed.
struct QaApp {
  running: Running,
  lease: Lease,
}

/// A QA app at work, stopped when dropped.
struct Running {
  pid: NonZeroU32,
  discovery: PathBuf,
  processes: RunProcesses,
}

impl Drop for Running {
  fn drop(&mut self) {
    let stopped = self.processes.run(ProcessCall::Stop { pid: self.pid });
    if stopped.is_error() {
      tracing::warn!(pid = self.pid, answer = ?stopped, "a QA app was not stopped");
    }
  }
}

/// The QA app `call_run_app` reaches: its pid and its discovery file.
pub(in crate::execution::step) struct QaTarget {
  pub(in crate::execution::step) pid: NonZeroU32,
  pub(in crate::execution::step) discovery: PathBuf,
}

impl QaApps {
  pub(in crate::execution) fn new(slots: QaSlots) -> Self {
    Self {
      slots,
      apps: Mutex::new(HashMap::new()),
      ended: Mutex::new(HashSet::new()),
    }
  }

  /// `invocation` ended: its QA app is stopped and its slot freed.
  pub(in crate::execution) fn end(&self, invocation: InvocationId) {
    self.ended.lock().insert(invocation);
    // Dropped outside the lock: stopping the app waits for it to close.
    let app = self.apps.lock().remove(&invocation);
    drop(app);
  }

  fn ended(&self, invocation: InvocationId) -> bool {
    self.ended.lock().contains(&invocation)
  }
}

/// The QA app tools of one invocation.
pub(in crate::execution::step) struct StepQa {
  pub(in crate::execution::step) apps: Arc<QaApps>,
  pub(in crate::execution::step) invocation: InvocationId,
  /// The QA folder's name, `.tmp/qa/<name>/` in the worktree.
  pub(in crate::execution::step) name: String,
  pub(in crate::execution::step) worktree: PathBuf,
  pub(in crate::execution::step) run_created: Timestamp,
  /// Says in the run's transcript that the start waits for the QA slot.
  pub(in crate::execution::step) waiting: Box<dyn Fn() + Send + Sync>,
}

impl StepQa {
  /// The app `call_run_app` with `pid` reaches through its known discovery file: the invocation's QA app when `pid`
  /// is left out or is its pid; `None` for another pid, whose discovery is searched for. Refused without a pid when
  /// the invocation started none.
  pub(in crate::execution::step) fn target(&self, pid: Option<NonZeroU32>) -> Result<Option<QaTarget>, String> {
    let apps = self.apps.apps.lock();
    let app = apps.get(&self.invocation).map(|app| QaTarget {
      pid: app.running.pid,
      discovery: app.running.discovery.clone(),
    });
    match pid {
      None => app
        .map(Some)
        .ok_or_else(|| "no QA app was started in this turn: call start_qa_app first, or give the app's pid".to_owned()),
      Some(pid) => Ok(app.filter(|app| app.pid == pid)),
    }
  }

  /// Starts a QA copy for `start` once a QA slot is free and keeps it for the invocation, replacing the one it
  /// started before. Blocks while it waits and while the script builds and starts the copy.
  pub(in crate::execution::step) fn start(&self, start: &QaAppStart) -> ToolResult {
    let waited = Instant::now();
    let earlier = self.apps.apps.lock().remove(&self.invocation);
    let lease = match earlier {
      Some(QaApp { running, lease }) => {
        drop(running);
        lease
      }
      None => match self.take_slot() {
        Ok(lease) => lease,
        Err(refusal) => return ToolResult::unavailable(&refusal),
      },
    };
    let waited = waited.elapsed();
    match self.run_script(start) {
      Ok(running) => self.keep(running, lease, start, waited),
      Err(refusal) => ToolResult::unavailable(&refusal),
    }
  }

  /// A free slot, waiting while every slot is held; refused when the invocation ends meanwhile.
  fn take_slot(&self) -> Result<Lease, String> {
    let mut said = false;
    loop {
      if self.apps.ended(self.invocation) {
        return Err("the turn ended before the QA slot was free".to_owned());
      }
      if let Some(lease) = self.apps.slots.try_take().map_err(|error| error.to_string())? {
        return Ok(lease);
      }
      if !said {
        (self.waiting)();
        said = true;
      }
      thread::sleep(SLOT_POLL);
    }
  }

  fn run_script(&self, start: &QaAppStart) -> Result<Running, String> {
    let folder = self.worktree.join(".tmp").join("qa").join(&self.name);
    let output = script::run(&self.worktree, &start.script_args(&self.name))?;
    let started = StartedQaApp::read(&output.stdout).ok_or_else(|| script::failure(&output))?;
    let processes = RunProcesses::new(&self.worktree, self.run_created).map_err(|error| error.to_string())?;
    let running = Running {
      pid: started.pid,
      discovery: started.discovery,
      processes,
    };
    // The app must be this run's, and its discovery file in its own QA folder: anything else is stopped as far as
    // the run's boundary lets it be, and refused.
    running
      .processes
      .app(running.pid)
      .map_err(|error| format!("the started QA app is refused: {error}"))?;
    if !inside(&running.discovery, &folder) {
      return Err(format!(
        "the QA app's discovery file is not in {}; it is refused",
        folder.display()
      ));
    }
    Ok(running)
  }

  /// Keeps `running` with `lease` for the invocation, or stops it when the invocation ended while it started.
  fn keep(&self, running: Running, lease: Lease, start: &QaAppStart, waited: Duration) -> ToolResult {
    let mut apps = self.apps.apps.lock();
    if self.apps.ended(self.invocation) {
      drop(apps);
      drop(running);
      return ToolResult::unavailable("the turn ended while the QA app started; it was stopped");
    }
    let mut text = format!(
      "QA app started on the {} scenario: pid {}. Call call_run_app without pid to drive it. Orchester stops it when \
       your turn ends; starting again replaces it.",
      start.scenario(),
      running.pid
    );
    if waited >= SLOT_POLL {
      // Writing to a String cannot fail.
      let _ = write!(text, " It waited {:.1} s for the QA slot.", waited.as_secs_f32());
    }
    apps.insert(self.invocation, QaApp { running, lease });
    ToolResult::shown(text)
  }
}

/// Whether `path` lies in `folder`, both with links resolved.
fn inside(path: &Path, folder: &Path) -> bool {
  match (std::fs::canonicalize(path), std::fs::canonicalize(folder)) {
    (Ok(path), Ok(folder)) => path.starts_with(folder),
    _ => false,
  }
}
