//! Executing runs: creating a run for a queued task, its worktree, and driving its agent steps on
//! their agent CLIs (Codex or Claude Code); pausing, resuming, cancelling and continuing a run
//! stopped at a limit. Every state change is recorded in the project database first; the desktop is only
//! told *that* something changed ([`Event::RunChanged`], [`Event::AgentActivity`]) and reads the
//! database.
//!
//! This version executes pipeline graphs of agent steps and of tool steps that run command tools
//! (`tool`): every step whose dependencies have passed runs, beside the others (see `workspace` for
//! where each works), with failure routes and retries, step budgets, approval gates on agent steps
//! (`plan`) and the outputs of earlier steps in later prompts. Other tools and gates on tool steps
//! are refused by the preflight. Every agent at work takes one of the desktop's agent slots, which
//! all runs share, and every command one of the runner's command slots; the project's daily token
//! budget holds new and continued work back once
//! today's runs used it.

mod again;
mod attach;
mod blocked_runs;
mod board;
mod broken;
mod chat;
mod checkpoint;
mod codebase_index;
mod connection_check;
mod control;
mod conversation;
mod environment;
mod estimate;
mod halt;
mod handoff;
mod held;
mod idle;
mod low_disk;
#[cfg(feature = "test-agent")]
mod own_worktrees;
mod permissions;
mod plan;
mod plan_limit;
mod pool;
mod prepare;
mod question;
mod record;
mod recovery;
mod returned;
mod run;
mod step;
mod stuck;
mod task_groups;
mod tool;
mod tool_approval;
mod turn_stop;
mod work;
mod workspace;

use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  time::Duration,
};

use orchester_core::{
  id::{ConnectionId, IdempotencyKey, RunId, TaskId},
  run::{InterruptCause, RaisedLimits, RunStatus},
  secret::SecretVault,
  store::RunStore,
  tokens::TokenBudget,
  workspaces::DiskSpace,
};
use orchester_ipc::{Event, Rejection};
use orchester_store::{ProjectDb, RunStoreError};
use parking_lot::Mutex;
use tokio::{
  sync::{mpsc, oneshot},
  task::JoinHandle,
};

pub use self::{attach::Attached, step::QaSlots};
use self::{
  board::Board,
  control::Control,
  prepare::Prepared,
  run::{RunTask, TaskContext},
  step::QaApps,
};
pub use crate::notify::Notifier;

/// How long stopping waits for each run to end its agent and record the interruption.
const STOP_TIMEOUT: Duration = Duration::from_secs(10);

struct ActiveRun {
  control: mpsc::UnboundedSender<Control>,
  task: JoinHandle<()>,
  /// Set by the task once it no longer executes the run, before the change that shows it can be
  /// read: the run settled (paused, stopped at a limit, cancelled, interrupted or ended), waits
  /// for a person with nothing else to do, or the task failed. The task only ends after.
  settled: Arc<AtomicBool>,
}

impl ActiveRun {
  /// Whether the task has ended. It lets go of its requests as it drops its state, once its last
  /// change is recorded and its agents have ended; its [`JoinHandle`] reports it finished only a
  /// moment later.
  fn ended(&self) -> bool {
    self.control.is_closed()
  }

  /// Whether the task still executes the run: it counts among the active runs.
  fn executes(&self) -> bool {
    !self.ended() && !self.settled.load(Ordering::Acquire)
  }
}

/// The runs this runner executes.
#[derive(Clone)]
pub struct Scheduler {
  inner: Arc<Inner>,
}

struct Inner {
  project_root: PathBuf,
  notifier: Notifier,
  /// Where the values of the secrets MCP servers use are read from when an agent starts.
  vault: Arc<dyn SecretVault>,
  /// The runner's own program, which Claude Code starts as its permission prompt; `None` when it
  /// cannot be found, which keeps steps that ask a person on Claude Code from starting.
  relay: Option<PathBuf>,
  board: Arc<Board>,
  /// The QA apps the runs' agents started, and the computer's QA slots they take.
  qa: Arc<QaApps>,
  active: Mutex<HashMap<RunId, ActiveRun>>,
  /// Serializes requests that start a task or change a run while its task may end. A fallback
  /// must not change a stored run after another request has started its next task.
  continuing: tokio::sync::Mutex<()>,
  group_retries: task_groups::GroupRetries,
  /// Read when the scheduler is made, as the runner starts, before the desktop connects.
  earlier_groups: task_groups::EarlierGroups,
  blocked_runs: blocked_runs::BlockedRuns,
  /// Runs stopped at their account's plan limit that a timer continues at its reset.
  plan_waits: plan_limit::PlanWaits,
  /// The turns of the Orchestrator chat under way.
  chats: chat::Chats,
  /// The disk guard's reading of free space (ORC-114), and its watch over runs paused for low disk space.
  disks: Arc<dyn DiskSpace>,
  disk_watch: low_disk::DiskWatch,
  /// Runs get a worktree of their own, as before workspaces: set only by tests (`own_worktrees`).
  own_worktrees: AtomicBool,
}

impl Scheduler {
  /// The scheduler of the project at `project_root`; its agents' QA apps take `qa_slots` ([`QaSlots::machine`]
  /// outside tests). The disk guard reads this computer's disks unless [`Scheduler::with_disks`] says otherwise.
  pub fn new(
    project_root: &Path,
    notifier: Notifier,
    vault: Arc<dyn SecretVault>,
    (relay, qa_slots): (Option<PathBuf>, QaSlots),
  ) -> Self {
    // Every statement of the process that waits for a database lock pauses off the runtime's async workers.
    orchester_store::pause_lock_waits_with(crate::lock_pause::pause_off_async_workers);
    Self {
      inner: Arc::new(Inner {
        project_root: project_root.to_path_buf(),
        vault,
        relay,
        board: Arc::new(Board::new(notifier.clone())),
        qa: Arc::new(QaApps::new(qa_slots)),
        notifier,
        active: Mutex::new(HashMap::new()),
        continuing: tokio::sync::Mutex::new(()),
        group_retries: task_groups::GroupRetries::default(),
        earlier_groups: task_groups::EarlierGroups::read(project_root),
        blocked_runs: blocked_runs::BlockedRuns::default(),
        plan_waits: plan_limit::PlanWaits::default(),
        chats: chat::Chats::default(),
        disks: Arc::new(orchester_exec::disk_space::LocalDisks),
        disk_watch: low_disk::DiskWatch::default(),
        own_worktrees: AtomicBool::new(false),
      }),
    }
  }

  /// This scheduler reading free space through `disks`, before it is shared: what tests give the disk guard.
  #[must_use]
  pub fn with_disks(mut self, disks: Arc<dyn DiskSpace>) -> Self {
    if let Some(inner) = Arc::get_mut(&mut self.inner) {
      inner.disks = disks;
    }
    self
  }

  fn plan_waits(&self) -> &plan_limit::PlanWaits {
    &self.inner.plan_waits
  }

  /// Runs being executed right now. A run its task settled is not, even while the task still
  /// ends: the desktop reads the change at once and may start other work in its place.
  pub fn active_runs(&self) -> Vec<RunId> {
    let mut active = self.inner.active.lock();
    active.retain(|_, run| !run.ended());
    active
      .iter()
      .filter(|(_, run)| run.executes())
      .map(|(&run, _)| run)
      .collect()
  }

  /// Waits for the run tasks registered when called to end, without stopping them. Closing each
  /// task's control receiver signals that its final state and agent work have settled.
  pub async fn wait_for_current_tasks(&self) {
    let controls: Vec<_> = self
      .inner
      .active
      .lock()
      .values()
      .filter(|run| !run.ended())
      .map(|run| run.control.clone())
      .collect();
    for control in controls {
      control.closed().await;
    }
  }

  /// Whether no run executes (including a step waiting for a slot or saving a checkpoint) and no
  /// request is changing a run under the scheduler lock. The server separately tracks requests
  /// awaiting a task's answer.
  pub fn is_idle(&self) -> bool {
    if !self.active_runs().is_empty() || self.inner.continuing.try_lock().is_err() || !self.inner.chats.idle() {
      return false;
    }
    match self
      .has_pending_task_groups()
      .and_then(|pending| Ok(pending || self.blocked_runs_wait_for_work()?))
    {
      Ok(pending) => !pending,
      Err(error) => {
        tracing::error!(?error, "cannot establish whether queued groups are idle");
        false
      }
    }
  }

  /// Creates and starts the run of queued task `task`, or finds the one an earlier request with
  /// `key` created. Must be called within a Tokio runtime; returns once the run is recorded as
  /// started and its first step is scheduled.
  pub async fn start_run(&self, key: IdempotencyKey, task: TaskId) -> Result<RunId, Rejection> {
    let _one_at_a_time = self.inner.continuing.lock().await;
    let root = &self.inner.project_root;
    let mut db = self.open()?;
    if let Some(recorded) = db
      .run_with_key(key)
      .map_err(|error| read_failed("cannot read runs", error))?
    {
      if recorded.status() == &RunStatus::Pending {
        self.check_group_start(&db, task, Some(&recorded))?;
        self.admit(&db)?;
      }
      let prepared = prepare::resume_recorded(&mut db, root, task, recorded);
      return self.begin(db, task, prepared);
    }
    self.check_group_start(&db, task, None)?;
    self.admit(&db)?;
    let prepared = prepare::prepare_first(&mut db, root, key, task);
    self.begin(db, task, prepared)
  }

  /// Tells the desktop about the run `prepared` recorded and executes it, unless an earlier
  /// request already does; or about the run of `task` that failed as it could not start.
  fn begin(&self, db: ProjectDb, task: TaskId, prepared: Result<Prepared, Rejection>) -> Result<RunId, Rejection> {
    let prepared = prepared.inspect_err(|rejection| {
      if let Rejection::RunNotStarted { run, .. } = rejection {
        self.inner.notifier.send(Event::RunChanged { run: *run, task });
      }
    })?;
    let run = prepared.run.id();
    self.inner.notifier.send(Event::RunChanged {
      run,
      task: prepared.run.task(),
    });
    if prepared.execute {
      self.spawn(db, prepared)?;
    }
    Ok(run)
  }

  /// Pauses `run` at its next checkpoint. Returns once the request is recorded; the run is
  /// paused when its active agent invocation has ended and its work is committed.
  pub async fn pause_run(&self, run: RunId) -> Result<(), Rejection> {
    loop {
      let one_at_a_time = self.inner.continuing.lock().await;
      if self.inner.board.is_stopping() {
        return Err(Rejection::ShuttingDown);
      }
      if let Some(answer) = self.ask(run, Control::Pause) {
        drop(one_at_a_time);
        if let Ok(result) = answer.await {
          return result;
        }
        continue;
      }
      if let Some(paused) = idle::pause(&mut self.open()?, run)? {
        self.inner.notifier.send(Event::RunChanged {
          run,
          task: paused.task(),
        });
      }
      return Ok(());
    }
  }

  /// Cancels `run`: ends its agent, keeps its worktree and branch. Returns once the run is
  /// recorded as cancelled.
  pub async fn cancel_run(&self, run: RunId) -> Result<(), Rejection> {
    self.plan_waits().clear(run);
    let cancelled = self.cancel_now(run).await;
    self.wait_again_if_refused(&cancelled);
    cancelled
  }

  async fn cancel_now(&self, run: RunId) -> Result<(), Rejection> {
    loop {
      let one_at_a_time = self.inner.continuing.lock().await;
      if self.inner.board.is_stopping() {
        return Err(Rejection::ShuttingDown);
      }
      if let Some(answer) = self.ask(run, Control::Cancel) {
        drop(one_at_a_time);
        if let Ok(result) = answer.await {
          return result;
        }
        continue;
      }
      // Without an answer the run's task ended before it read the request: the run is idle now.
      let cancelled = idle::cancel(&mut self.open()?, run)?;
      self.inner.notifier.send(Event::RunChanged {
        run,
        task: cancelled.task(),
      });
      return Ok(());
    }
  }

  /// Continues paused run `run`; a step it stopped in continues the agent's CLI session.
  pub async fn resume_run(&self, run: RunId) -> Result<(), Rejection> {
    self.plan_waits().clear(run);
    let resumed = self.continue_settled(run, |db, root| idle::resume(db, root, run)).await;
    self.wait_again_if_refused(&resumed);
    resumed
  }

  /// A run whose timer a person's refused request dropped gets it back.
  fn wait_again_if_refused(&self, result: &Result<(), Rejection>) {
    if result.is_err() {
      self.wait_for_plan_resets();
    }
  }

  /// Continues `run`, stopped at its account's plan limit, now on `connection`, another account's connection of
  /// the same CLI, as a new session for the stopped step and this run only.
  pub async fn continue_on_connection(&self, run: RunId, connection: ConnectionId) -> Result<(), Rejection> {
    self.plan_waits().clear(run);
    let continued = self
      .continue_settled(run, |db, root| plan_limit::continue_on(db, root, run, connection))
      .await;
    self.wait_again_if_refused(&continued);
    continued
  }

  /// Continues `run`, stopped at its account's plan limit, on the same account now, as a person asked; it stops there
  /// again if the CLI still refuses.
  pub async fn try_at_plan_limit(&self, run: RunId) -> Result<(), Rejection> {
    self.plan_waits().clear(run);
    let tried = self
      .continue_settled(run, |db, root| plan_limit::try_now(db, root, run))
      .await;
    self.wait_again_if_refused(&tried);
    tried
  }

  /// Raises the limits of `run`, stopped at a limit, and continues it.
  pub async fn raise_limit(&self, run: RunId, limits: RaisedLimits) -> Result<(), Rejection> {
    self
      .continue_settled(run, |db, root| idle::raise(db, root, run, limits))
      .await
  }

  /// Raises, for `run` only, the budget of the step it stopped at, and continues the run: the
  /// stopped agent's CLI session continues.
  pub async fn raise_step_budget(&self, run: RunId, budget: Option<TokenBudget>) -> Result<(), Rejection> {
    self
      .continue_settled(run, |db, root| idle::raise_step_budget(db, root, run, budget))
      .await
  }

  /// "Wait for agents to save": starts no new work, asks every active run to pause at its next
  /// checkpoint and returns once each has settled. The work summary shows each run saving, then
  /// saved. Pending tool decisions checkpoint without granting the requested action.
  pub async fn finish_at_checkpoints(&self) {
    let settling = {
      let _one_at_a_time = self.inner.continuing.lock().await;
      self.inner.board.stop_at_checkpoints();
      let mut settling = Vec::new();
      for run in self.active_runs() {
        match self.ask(run, Control::Pause) {
          Some(answer) => settling.push((run, answer)),
          None => tracing::debug!(%run, "the run settled before it was asked to pause"),
        }
      }
      settling
    };
    for (run, answer) in settling {
      match answer.await {
        Ok(Ok(())) => {}
        Ok(Err(rejection)) => tracing::warn!(%run, ?rejection, "the run could not be asked to pause"),
        Err(_) => tracing::debug!(%run, "the run settled before it read the pause"),
      }
    }
    // Keep the tasks visible to an immediate shutdown while their agents finish the checkpoint.
    self.wait_for_current_tasks().await;
    let tasks: Vec<(RunId, ActiveRun)> = {
      let _one_at_a_time = self.inner.continuing.lock().await;
      self.inner.active.lock().drain().collect()
    };
    for (run, active) in tasks {
      if let Err(error) = active.task.await {
        tracing::warn!(%run, %error, "the run's task ended abnormally");
      }
    }
  }

  /// Ends the agents of every active run and records the runs as interrupted by `cause`,
  /// waiting at most [`STOP_TIMEOUT`] for each.
  pub async fn stop_all(&self, cause: InterruptCause) {
    let _one_at_a_time = self.inner.continuing.lock().await;
    self.inner.board.stop_immediately();
    let active: Vec<(RunId, ActiveRun)> = self.inner.active.lock().drain().collect();
    for (run, active) in active {
      // A run that already ended has dropped its receiver; there is nothing to stop.
      if active.control.send(Control::Interrupt(cause)).is_err() {
        tracing::debug!(%run, "the run had already ended");
      }
      let mut task = active.task;
      if tokio::time::timeout(STOP_TIMEOUT, &mut task).await.is_err() {
        tracing::warn!(%run, "the run did not stop within {STOP_TIMEOUT:?}; ending it");
        task.abort();
      }
    }
  }

  fn open(&self) -> Result<ProjectDb, Rejection> {
    ProjectDb::open(&self.inner.project_root).map_err(|error| failed("cannot open the project database", &error))
  }

  /// Hands `control` to the task executing `run`, if one does; `None` when the run is idle. A task
  /// that ends before it reads the request drops it unanswered: once the answer fails, the task
  /// has ended.
  fn ask<T>(&self, run: RunId, control: impl FnOnce(oneshot::Sender<T>) -> Control) -> Option<oneshot::Receiver<T>> {
    let active = self.inner.active.lock();
    let active = active.get(&run).filter(|active| !active.ended())?;
    let (answer, answered) = oneshot::channel();
    active.control.send(control(answer)).ok()?;
    Some(answered)
  }

  /// Whether new or continued work may start: the runner is not stopping, an agent slot is
  /// free, the disk has the free space the project keeps and today's token budget has room.
  fn admit(&self, db: &ProjectDb) -> Result<(), Rejection> {
    if self.inner.board.is_stopping() {
      return Err(Rejection::ShuttingDown);
    }
    // Each executing run has at least one agent at work or about to be, and its parallel steps
    // take further slots while they are free.
    if let Some(slots) = self.inner.board.slots()
      && (self.active_runs().len() >= slots.get() as usize || !self.inner.board.has_free_slot())
    {
      return Err(Rejection::NoAgentSlot { slots });
    }
    self.check_disk(db)?;
    idle::check_daily_budget(db)
  }

  /// Applies `transition` to idle run `run` and executes it again.
  fn continue_idle(
    &self,
    run: RunId,
    transition: impl FnOnce(&mut ProjectDb, &Path) -> Result<Prepared, Rejection>,
  ) -> Result<(), Rejection> {
    if self.is_active(run) {
      return Err(Rejection::RunNotPaused);
    }
    let mut db = self.open()?;
    self.admit(&db)?;
    let prepared = transition(&mut db, &self.inner.project_root)?;
    self.inner.notifier.send(Event::RunChanged {
      run,
      task: prepared.run.task(),
    });
    self.spawn(db, prepared)
  }

  /// [`Scheduler::continue_idle`] for a run its task may have settled a moment ago. The task
  /// records the run paused or stopped at a limit as its last change and ends right after, so the
  /// desktop can read the settled run and ask to continue it before the task has ended: the
  /// request then waits for it, and the run is never executed by two tasks.
  async fn continue_settled(
    &self,
    run: RunId,
    transition: impl FnOnce(&mut ProjectDb, &Path) -> Result<Prepared, Rejection>,
  ) -> Result<(), Rejection> {
    self.settled(run).await?;
    let _one_at_a_time = self.inner.continuing.lock().await;
    self.continue_idle(run, transition)
  }

  /// Waits for the task of `run` to end once it has settled the run; a run its task still
  /// executes is not paused.
  async fn settled(&self, run: RunId) -> Result<(), Rejection> {
    let ending = {
      let active = self.inner.active.lock();
      match active.get(&run) {
        Some(active) if active.executes() => return Err(Rejection::RunNotPaused),
        Some(active) => active.control.clone(),
        None => return Ok(()),
      }
    };
    ending.closed().await;
    Ok(())
  }

  fn is_active(&self, run: RunId) -> bool {
    self.inner.active.lock().get(&run).is_some_and(|active| !active.ended())
  }

  /// Starts the task that executes `prepared`. A run another task still executes is refused rather
  /// than left to that task, which does not know the change just recorded.
  fn spawn(&self, db: ProjectDb, prepared: Prepared) -> Result<(), Rejection> {
    #[cfg(feature = "test-agent")]
    let prepared = self.own_worktree_for_tests(prepared)?;
    let mut active = self.inner.active.lock();
    let run = prepared.run.id();
    if active.get(&run).is_some_and(|active| !active.ended()) {
      tracing::error!(%run, "another task still executes the run");
      return Err(Rejection::Failed {
        detail: format!("another task still executes run {run}"),
      });
    }
    let (control, controls) = mpsc::unbounded_channel();
    let settled = Arc::new(AtomicBool::new(false));
    let context = TaskContext {
      root: self.inner.project_root.clone(),
      notifier: self.inner.notifier.clone(),
      vault: self.inner.vault.clone(),
      relay: self.inner.relay.clone(),
      board: self.inner.board.clone(),
      qa: self.inner.qa.clone(),
      disks: self.inner.disks.clone(),
      controls,
      settled: settled.clone(),
    };
    let scheduler = self.clone();
    let task = tokio::spawn(async move {
      RunTask::new(db, prepared, context).drive().await;
      // A run its task stopped at a plan limit continues at the reset, and one it paused for low disk space once
      // the disk has room.
      scheduler.wait_for_plan_resets();
      scheduler.watch_disk();
    });
    active.insert(run, ActiveRun { control, task, settled });
    Ok(())
  }
}

fn failed(what: &str, error: &dyn std::error::Error) -> Rejection {
  tracing::error!("{what}: {error}");
  Rejection::Failed {
    detail: format!("{what}: {error}"),
  }
}

/// A run that could not be read: its record cannot be read back, which rejects the request for it
/// and leaves the record as it is, or reading failed.
fn read_failed(what: &str, error: RunStoreError) -> Rejection {
  match error {
    RunStoreError::Unreadable { run, source } => {
      tracing::warn!(%run, error = %source, "a request names a run that cannot be read");
      Rejection::RunUnreadable {
        detail: source.to_string(),
      }
    }
    other => failed(what, &other),
  }
}
