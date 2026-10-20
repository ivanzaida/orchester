//! `start_qa_app` (`orchester_core::orchester_tools::QaAppStart`): the runner takes one of the computer's QA slots its
//! project allows ([`slots`]), waiting while other runs' QA apps hold every one, stops what a holder that died left
//! running ([`leftover`]), runs the worktree's `tools/qa-app.ps1` ([`script`]) and keeps the app it started for the
//! invocation, so `call_run_app` without a pid reaches it through its exact discovery file.
//!
//! Several QA apps open at once do not clash: each run's step has its own QA folder, `.tmp/qa/run-<number>-<step>/`
//! in its own worktree (run numbers are unique in a project and worktrees are per run), which holds the app's copied
//! programs, its data, its seeded repository and its MCP discovery folder; the script gives each app a free port and a
//! new token, and the app's own runner serves its own seeded project.
//!
//! Each invocation has its state ([`InvocationQa`]), shared by its tools and the runner: its starts run one at a time
//! (a second waits for the first, then replaces its app and keeps the slot), and it ends once. When the invocation
//! ends ([`QaApps::end`]) — its turn is over, its run stopped, paused or failed — its app is stopped with everything it
//! started inside the worktree and the slot freed, off the async workers; a start still waiting for a slot, or
//! whose script still runs, gives up and ends the script's tree. While the invocation waits for a person inside its
//! turn (a tool approval, a question; [`QaApps::release`]) its app is stopped and the slot freed as well, so a person's
//! wait never holds one of the computer's slots: the agent starts the app again once it goes on.

mod leftover;
mod script;
mod slots;
mod task;
mod tree;

use std::{
  collections::HashMap,
  fmt::Write as _,
  num::NonZeroU32,
  path::{Path, PathBuf},
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  thread,
  time::{Duration, Instant},
};

use orchester_core::{
  connection::stream::QaSlotHolder,
  id::InvocationId,
  limits::QaSlots as Allowed,
  orchester_tools::{ProcessCall, QA_APP_SCRIPT, QaAppStart, StartedQaApp, ToolResult},
  project::TaskKey,
  run::RunNumber,
  store::SettingsStore,
  time::Timestamp,
};
use orchester_exec::run_processes::RunProcesses;
use orchester_store::ProjectDb;
use parking_lot::Mutex;

pub use self::slots::QaSlots;
use self::slots::{Holder, Lease, Take};

/// How often a start that waits for a QA slot tries again.
const SLOT_POLL: Duration = Duration::from_millis(500);

/// The QA state of the runner's invocations that have not ended, and the computer's QA slots.
pub(in crate::execution) struct QaApps {
  slots: QaSlots,
  invocations: Mutex<HashMap<InvocationId, Arc<InvocationQa>>>,
}

/// One invocation's QA app: its starts one at a time, the app it keeps, and whether it ended. Its tools hold it as long
/// as they live, so a start after the end still sees the end; the runner forgets it once the invocation ended.
#[derive(Default)]
pub(in crate::execution::step) struct InvocationQa {
  ended: AtomicBool,
  /// It waits for a person inside its turn: an app a start of its finishes now is stopped at once.
  waiting: AtomicBool,
  starting: Mutex<()>,
  app: Mutex<Option<QaApp>>,
}

impl InvocationQa {
  fn ended(&self) -> bool {
    self.ended.load(Ordering::Acquire)
  }

  /// Refused while the invocation waits for a person: a start then neither takes the slot nor builds, since the app
  /// would be stopped once it started.
  fn not_waiting(&self) -> Result<(), String> {
    if self.waiting.load(Ordering::Acquire) {
      return Err(
        "the turn waits for a person, so no QA app is started meanwhile; call start_qa_app again once you go on"
          .to_owned(),
      );
    }
    Ok(())
  }
}

/// A started QA app with the slot it holds; the app is stopped before the slot is freed.
struct QaApp {
  running: Running,
  lease: Lease,
}

/// A QA app at work, stopped when dropped.
struct Running {
  pid: NonZeroU32,
  /// Its discovery file, with links resolved when it was started.
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
      invocations: Mutex::new(HashMap::new()),
    }
  }

  /// The QA state of `invocation`, which is starting.
  fn register(&self, invocation: InvocationId) -> Arc<InvocationQa> {
    Arc::clone(self.invocations.lock().entry(invocation).or_default())
  }

  /// `invocation` ended: its QA app stops and its slot is freed, and a start of its still under way gives up.
  pub(in crate::execution) fn end(&self, invocation: InvocationId) {
    let Some(state) = self.invocations.lock().remove(&invocation) else {
      return;
    };
    state.ended.store(true, Ordering::Release);
    stop_aside(state.app.lock().take());
  }

  /// `invocation` waits for a person inside its turn: its QA app stops and its slot is freed meanwhile, also the app
  /// a start of its still under way would keep.
  pub(in crate::execution) fn release(&self, invocation: InvocationId) {
    let state = self.invocations.lock().get(&invocation).cloned();
    if let Some(state) = state {
      let mut app = state.app.lock();
      state.waiting.store(true, Ordering::Release);
      stop_aside(app.take());
    }
  }

  /// `invocation`'s wait for a person ended: its starts keep their apps again.
  pub(in crate::execution) fn resume(&self, invocation: InvocationId) {
    let state = self.invocations.lock().get(&invocation).cloned();
    if let Some(state) = state {
      state.waiting.store(false, Ordering::Release);
    }
  }
}

/// Stops `app` and frees its slot on a thread of its own: closing an app takes up to a few seconds, which no async
/// worker waits for.
fn stop_aside(app: Option<QaApp>) {
  if let Some(app) = app {
    thread::spawn(move || drop(app));
  }
}

/// The QA app tools of one invocation.
pub(in crate::execution::step) struct StepQa {
  pub(in crate::execution::step) apps: Arc<QaApps>,
  pub(in crate::execution::step) state: Arc<InvocationQa>,
  /// The QA folder's name, `.tmp/qa/<name>/` in the worktree.
  pub(in crate::execution::step) name: String,
  pub(in crate::execution::step) worktree: PathBuf,
  /// The project's repository, whose database holds its *QA apps at once*.
  pub(in crate::execution::step) root: PathBuf,
  pub(in crate::execution::step) run_created: Timestamp,
  /// The run's task's key and the run's number, which its slot's record names.
  pub(in crate::execution::step) task: TaskKey,
  pub(in crate::execution::step) run: RunNumber,
  /// Says in the run's transcript that the start waits for a QA slot, and who holds them when that is known.
  pub(in crate::execution::step) waiting: Box<dyn Fn(Vec<QaSlotHolder>) + Send + Sync>,
}

impl StepQa {
  /// The app `call_run_app` with `pid` reaches through its known discovery file: the invocation's QA app when `pid`
  /// is left out or is its pid; `None` for another pid, whose discovery is searched for. Refused without a pid when
  /// the invocation runs no QA app.
  pub(in crate::execution::step) fn target(&self, pid: Option<NonZeroU32>) -> Result<Option<QaTarget>, String> {
    let app = self.state.app.lock().as_ref().map(|app| QaTarget {
      pid: app.running.pid,
      discovery: app.running.discovery.clone(),
    });
    match pid {
      None => app.map(Some).ok_or_else(|| {
        "no QA app runs in this turn (none was started, or it was stopped while the turn waited for a person): call \
         start_qa_app, or give the app's pid"
          .to_owned()
      }),
      Some(pid) => Ok(app.filter(|app| app.pid == pid)),
    }
  }

  /// Starts a QA copy for `start` once a QA slot is free and keeps it for the invocation, replacing the one it
  /// started before. Blocks while an earlier start of the invocation runs, while it waits for the slot and while the
  /// script builds and starts the copy.
  /// `approved` is the QA script's blob its approval named: the script runs only while it is still that one.
  pub(in crate::execution::step) fn start(&self, start: &QaAppStart, approved: Option<&str>) -> ToolResult {
    let _one_at_a_time = self.state.starting.lock();
    let waited = Instant::now();
    let earlier = self.state.app.lock().take();
    let mut lease = match earlier {
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
    let folder = match self.qa_folder() {
      Ok(folder) => folder,
      Err(refusal) => return ToolResult::unavailable(&refusal),
    };
    let holder = Holder {
      folder: folder.clone(),
      since: self.run_created,
      task: self.task.clone(),
      run: self.run,
    };
    leftover::stop(&mut lease, &holder);
    if let Err(refusal) = self.state.not_waiting() {
      return ToolResult::unavailable(&refusal);
    }
    let now = script_blob(&self.worktree);
    if approved.is_none_or(str::is_empty) || now.as_deref() != approved {
      return ToolResult::unavailable(&format!(
        "{QA_APP_SCRIPT} is not the script this start's approval named (it changed since, or could not be read); \
         call start_qa_app again to be asked about it as it is now"
      ));
    }
    match self.run_script(start, &folder) {
      Ok(running) => self.keep(running, lease, start, waited),
      Err(refusal) => ToolResult::unavailable(&refusal),
    }
  }

  /// This step's QA folder, `.tmp/qa/<name>/` in the worktree, made if it is missing, with links resolved; refused
  /// when a link lies on the way to it below the worktree, where the script and the cleanup would follow it.
  fn qa_folder(&self) -> Result<PathBuf, String> {
    let spelled = self.worktree.join(".tmp").join("qa").join(&self.name);
    std::fs::create_dir_all(&spelled).map_err(|error| format!("the QA folder cannot be made: {error}"))?;
    let worktree =
      std::fs::canonicalize(&self.worktree).map_err(|error| format!("the worktree cannot be read: {error}"))?;
    let expected = worktree.join(".tmp").join("qa").join(&self.name);
    match leftover::link_free(&expected) {
      Some(folder) => Ok(folder),
      None => Err(format!(
        "{} is reached through a link; no QA app is started from it",
        spelled.display()
      )),
    }
  }

  /// A free slot of those the project allows, read at each try, waiting while every one is held; refused when the
  /// invocation ends meanwhile.
  fn take_slot(&self) -> Result<Lease, String> {
    let mut said = false;
    loop {
      if self.state.ended() {
        return Err("the turn ended before a QA slot was free".to_owned());
      }
      self.state.not_waiting()?;
      match self
        .apps
        .slots
        .try_take(self.allowed())
        .map_err(|error| error.to_string())?
      {
        Take::Taken(lease) => return Ok(lease),
        Take::AllHeld(holders) if !said => {
          (self.waiting)(named(holders));
          said = true;
        }
        Take::AllHeld(_) => {}
      }
      thread::sleep(SLOT_POLL);
    }
  }

  /// The project's *QA apps at once* as stored now; the fewest, logged, when it cannot be read.
  fn allowed(&self) -> Allowed {
    match ProjectDb::open_read_only(&self.root).and_then(|db| db.project_limits()) {
      Ok(limits) => limits.qa_slots(),
      Err(error) => {
        tracing::warn!(%error, "the project's QA apps at once cannot be read; this start uses one slot");
        Allowed::new(Allowed::MIN.into()).expect("the fewest QA slots are in range")
      }
    }
  }

  /// Runs the script and checks the app it names: this run's, its discovery file in its own QA folder `folder`. A
  /// script given up, failed or naming no app that passes ends with everything it started.
  fn run_script(&self, start: &QaAppStart, folder: &Path) -> Result<Running, String> {
    let state = Arc::clone(&self.state);
    let ran = script::run(&self.worktree, &start.script_args(&self.name), &move || state.ended())?;
    let started = StartedQaApp::read(&ran.output.stdout).ok_or_else(|| script::failure(&ran.output))?;
    let processes = RunProcesses::new(&self.worktree, self.run_created).map_err(|error| error.to_string())?;
    processes
      .app(started.pid)
      .map_err(|error| format!("the started QA app is refused, and what the script started was ended: {error}"))?;
    let discovery = inside(&started.discovery, folder).ok_or_else(|| {
      format!(
        "the QA app's discovery file is not in {}; what the script started was ended",
        folder.display()
      )
    })?;
    ran.keep()?;
    Ok(Running {
      pid: started.pid,
      discovery,
      processes,
    })
  }

  /// Keeps `running` with `lease` for the invocation, or stops it when the invocation ended while it started.
  fn keep(&self, running: Running, lease: Lease, start: &QaAppStart, waited: Duration) -> ToolResult {
    let mut app = self.state.app.lock();
    if self.state.ended() {
      drop(app);
      drop(running);
      return ToolResult::unavailable("the turn ended while the QA app started; it was stopped");
    }
    // Checked under the app's lock, which `QaApps::release` takes too: a wait that began during the start sees the
    // app stored, or the start sees the wait.
    if self.state.waiting.load(Ordering::Acquire) {
      drop(app);
      drop((running, lease));
      return ToolResult::unavailable(
        "the turn waits for a person, so the QA app that just started was stopped and its slot freed; call \
         start_qa_app again once you go on",
      );
    }
    let mut text = format!(
      "QA app started on the {} scenario: pid {}. Call call_run_app without pid to drive it. Orchester stops it when \
       your turn ends or waits for a person; starting again replaces it.",
      start.scenario(),
      running.pid
    );
    if waited >= SLOT_POLL {
      // Writing to a String cannot fail.
      let _ = write!(text, " It waited {:.1} s for a QA slot.", waited.as_secs_f32());
    }
    *app = Some(QaApp { running, lease });
    ToolResult::shown(text)
  }
}

/// The runs `holders` names for the transcript; none when a slot named no holder that reads.
fn named(holders: Option<Vec<Holder>>) -> Vec<QaSlotHolder> {
  holders
    .unwrap_or_default()
    .into_iter()
    .map(|holder| QaSlotHolder {
      task: holder.task.to_string(),
      run: holder.run.get(),
    })
    .collect()
}

/// The git blob of the worktree's QA script as it is now, which a start's approval names; `None`, logged, when git
/// cannot tell.
pub(in crate::execution::step) fn script_blob(worktree: &Path) -> Option<String> {
  crate::git::Git::new(worktree)
    // Of the file's exact bytes, which are what runs: no line-ending or other attribute filter applied first.
    .stdout(&["hash-object", "--no-filters", "--", QA_APP_SCRIPT])
    .map(|blob| blob.trim().to_owned())
    .inspect_err(|error| tracing::warn!(%error, "the QA script's blob cannot be read"))
    .ok()
    .filter(|blob| !blob.is_empty())
}

/// `path` with links resolved, when it lies in `folder`.
fn inside(path: &Path, folder: &Path) -> Option<PathBuf> {
  match (std::fs::canonicalize(path), std::fs::canonicalize(folder)) {
    (Ok(path), Ok(folder)) if path.starts_with(&folder) => Some(path),
    _ => None,
  }
}
