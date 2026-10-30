//! Driving one run: every step execution whose dependencies have passed starts, as far as agent
//! slots allow, and runs beside the others until the run succeeds or fails, or settles: paused at
//! a checkpoint, stopped at a limit, cancelled or interrupted. The task owns the run's state: each
//! transition is appended to the run's log and announced here, while each agent reports its
//! usage and its end from a task of its own ([`super::step::worker`]). Requests for the run
//! (pause, cancel, the runner stopping) arrive as [`Control`]s.

use std::{
  collections::HashMap,
  path::PathBuf,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  time::Duration,
};

use orchester_core::{
  connection::stream::{AgentEvent, RunnerProblem},
  id::{MessageId, StepExecutionId},
  project::Project,
  run::{InterruptCause, Invocation, Run, RunError, RunEvent, RunLimit, RunStatus, active_time},
  secret::SecretVault,
  store::ActivityStore,
  task::Task,
  time::Timestamp,
};
use orchester_ipc::{Event, Rejection};
use orchester_store::{ProjectDb, StoreError};
use thiserror::Error;
use tokio::{sync::mpsc, time::Instant};

use super::{
  Notifier,
  board::Board,
  codebase_index::{self, IndexBuild, IndexState},
  control::{Ack, Control, answer},
  low_disk::DiskGuard,
  pool::PoolError,
  prepare::{Prepared, RunFolder},
  record::{RecordError, announce_ready, record},
  step::{
    Active, QaApps,
    worker::{Stop, StopCause, WorkerEvent},
  },
};
use crate::browser::ProjectBrowser;

/// How long a run waiting for a free agent slot waits before it looks again, in case the slot
/// was freed by another run in a way it was not woken for. More slots wake it at once.
const SLOT_RETRY: Duration = Duration::from_secs(1);
/// The time limit is checked at least this often while agents work.
const TIME_CHECK_FLOOR: Duration = Duration::from_millis(50);

#[derive(Debug, Error)]
pub(super) enum ExecutionError {
  #[error(transparent)]
  Record(#[from] RecordError),
  #[error(transparent)]
  Store(#[from] StoreError),
  #[error("step execution {0} is not an agent step of the run")]
  NotAnAgentStep(StepExecutionId),
  #[error("the run has steps left but none can run")]
  Stalled,
  #[error(transparent)]
  Conversation(orchester_core::conversation::MessageError),
  #[error(transparent)]
  Workspace(#[from] PoolError),
}

/// What the task of a run is given besides the run: where it announces changes, where its
/// requests arrive and where it tells the scheduler that it no longer executes the run.
pub(super) struct TaskContext {
  pub(super) root: PathBuf,
  pub(super) notifier: Notifier,
  pub(super) vault: Arc<dyn SecretVault>,
  pub(super) relay: Option<PathBuf>,
  pub(super) board: Arc<Board>,
  pub(super) qa: Arc<QaApps>,
  pub(super) browser: Arc<ProjectBrowser>,
  pub(super) disks: DiskGuard,
  pub(super) controls: mpsc::UnboundedReceiver<Control>,
  pub(super) settled: Arc<AtomicBool>,
}

/// Requests received and not carried out yet; they take effect once every agent has ended.
#[derive(Default)]
pub(super) struct Pending {
  pub(super) cancel: Option<Ack>,
  pub(super) interrupt: Option<InterruptCause>,
  /// A limit the run reached while agents worked.
  pub(super) limit: Option<RunLimit>,
  /// The disk ran low while agents worked: they were stopped, and the run pauses once they ended.
  pub(super) low_disk: bool,
}

pub(super) struct RunTask {
  /// A step a person asked to start on its account now ([`Prepared::try_at_plan_limit`]), taken when it starts.
  pub(super) try_at_plan_limit: Option<orchester_core::id::StepExecutionId>,
  pub(super) root: PathBuf,
  pub(super) db: ProjectDb,
  pub(super) run: Run,
  pub(super) task: Task,
  pub(super) project: Project,
  /// Where the run works: its own worktree from before workspaces, or a workspace it borrows.
  pub(super) folder: RunFolder,
  /// The folder on the run's branch while the run works: its own worktree, or the workspace it borrowed, which its
  /// first step takes ([`RunTask::hold_workspace`]).
  pub(super) worktree: Option<PathBuf>,
  pub(super) notifier: Notifier,
  pub(super) vault: Arc<dyn SecretVault>,
  /// The program Claude Code starts as its permission prompt ([`crate::approval_relay`]).
  pub(super) relay: Option<PathBuf>,
  pub(super) board: Arc<Board>,
  /// The QA apps the run's agents started ([`QaApps`]), shared with the runner's other runs for the QA slots.
  pub(super) qa: Arc<QaApps>,
  /// The project's browser, whose tabs the run's browser tools open and drive.
  pub(super) browser: Arc<ProjectBrowser>,
  /// The disk guard's reading of free space, and how often it looks while agents work.
  pub(super) disks: DiskGuard,
  /// When the disk guard looks at the free space next while agents work.
  pub(super) next_disk_check: Instant,
  /// The run waits for a free workspace; the desktop was told once.
  pub(super) waits_for_workspace: bool,
  pub(super) controls: mpsc::UnboundedReceiver<Control>,
  listening: bool,
  /// Set before the change that settles the run is recorded, or once the task stops executing
  /// the run otherwise; the scheduler no longer counts the run from then on.
  settled: Arc<AtomicBool>,
  pub(super) pending: Pending,
  /// Agents and command tools at work, by the execution they work on.
  pub(super) agents: HashMap<StepExecutionId, Active>,
  /// The code index of the run's base revision.
  pub(super) code_index: IndexState,
  /// Its build in the background, while one runs.
  pub(super) index_build: Option<IndexBuild>,
  pub(super) worker_events: mpsc::UnboundedSender<WorkerEvent>,
  events: mpsc::UnboundedReceiver<WorkerEvent>,
  /// The memory records were scanned for unreadable ones: once per run task is enough.
  pub(super) memory_scanned: bool,
  /// People's messages sent now that went to an agent at work, each once, by the execution whose
  /// agent has them until it ends: no other agent's prompt takes them meanwhile.
  pub(super) steered: HashMap<MessageId, StepExecutionId>,
}

impl RunTask {
  pub(super) fn new(db: ProjectDb, prepared: Prepared, context: TaskContext) -> Self {
    let project = db.project().clone();
    let (worker_events, events) = mpsc::unbounded_channel();
    Self {
      root: context.root,
      db,
      run: prepared.run,
      task: prepared.task,
      project,
      worktree: match &prepared.folder {
        RunFolder::Own(path) => Some(path.clone()),
        RunFolder::Borrowed => None,
      },
      folder: prepared.folder,
      notifier: context.notifier,
      vault: context.vault,
      relay: context.relay,
      board: context.board,
      qa: context.qa,
      browser: context.browser,
      disks: context.disks,
      next_disk_check: Instant::now(),
      waits_for_workspace: false,
      controls: context.controls,
      listening: true,
      settled: context.settled,
      pending: Pending::default(),
      try_at_plan_limit: prepared.try_at_plan_limit,
      agents: HashMap::new(),
      code_index: IndexState::NotBuilt,
      index_build: None,
      worker_events,
      events,
      memory_scanned: false,
      steered: HashMap::new(),
    }
  }

  /// Executes the run until it ends or settles; whether it ended (completed, cancelled or failed). A failure of the
  /// runner itself (the database, a rejected transition) ends the run's agents and interrupts the run where it was
  /// last recorded, with the error in its activity ([`super::broken`]); it is logged, and its steps are not retried.
  pub(super) async fn drive(mut self) -> bool {
    let run = self.run.id();
    tracing::info!(%run, "run executing");
    let result = self.steps().await;
    // A run that waits for a person, or whose task failed, is no longer executed either.
    self.settled.store(true, Ordering::Release);
    if let Err(error) = &result {
      self.end_agents(error).await;
      self.interrupt_broken(error);
      self.controls.close();
      while let Ok(control) = self.controls.try_recv() {
        control.reject(error.to_string());
      }
    }
    self.end_waiting_approvals();
    self.end_waiting_questions();
    self.give_back_workspaces();
    self.board.settled(&self.run);
    match result {
      Ok(()) => tracing::info!(%run, status = ?self.run.status(), "run settled"),
      Err(error) => {
        tracing::error!(%run, %error, "the run could not continue");
        if let Some(ack) = self.pending.cancel.take() {
          answer(
            ack,
            Err(Rejection::Failed {
              detail: error.to_string(),
            }),
          );
        }
      }
    }
    self.run.status().is_terminal()
  }

  async fn steps(&mut self) -> Result<(), ExecutionError> {
    self.begin_code_index()?;
    loop {
      while let Ok(control) = self.controls.try_recv() {
        self.control(control)?;
      }
      if self.agents.is_empty() && self.settle()? {
        return Ok(());
      }
      if self.index_holds_starts() {
        self.wait(false).await?;
        continue;
      }
      let waiting_for_slot = self.start_ready()?;
      if self.agents.is_empty() && !waiting_for_slot {
        // A step that could not start fails, which may open a retry attempt to start next.
        let startable = !self.run.executions_to_start(1).is_empty() || !self.run.running_executions().is_empty();
        if startable && self.may_start() {
          continue;
        }
        // A run that waits for a person ends its task here: it holds no agent slot meanwhile, and
        // the decision executes it again.
        if self.settle()? || self.run.waits_for_a_person() {
          return Ok(());
        }
        return Err(ExecutionError::Stalled);
      }
      self.wait(waiting_for_slot).await?;
    }
  }

  /// Waits for an agent's news, a request, the time limit, a free slot or the code index build.
  /// A task that stops executing the run (a cancel, a pause, the runner stopping) leaves a running
  /// build to finish in the background; the next start of the task reuses it.
  async fn wait(&mut self, waiting_for_slot: bool) -> Result<(), ExecutionError> {
    let working = !self.agents.is_empty();
    // A run without a time limit has no time to check; it still looks for a free slot again, and while agents work
    // the disk guard looks at the free space.
    let time_check = if working { self.time_check()? } else { None };
    let disk_check = working.then_some(self.next_disk_check);
    let slot_retry = waiting_for_slot.then(|| Instant::now() + SLOT_RETRY);
    let deadline = [time_check, disk_check, slot_retry].into_iter().flatten().min();
    tokio::select! {
      Some(event) = self.events.recv() => self.on_worker_event(event),
      control = self.controls.recv(), if self.listening => match control {
        Some(control) => self.control(control),
        None => {
          self.listening = false;
          Ok(())
        }
      },
      () = tokio::time::sleep_until(deadline.unwrap_or_else(Instant::now)), if deadline.is_some() => {
        if working {
          self.check_time()?;
          self.check_disk_while_working()?;
        }
        Ok(())
      }
      () = self.board.resized(), if waiting_for_slot => Ok(()),
      state = codebase_index::built(&mut self.index_build) => {
        self.code_index_built(state);
        Ok(())
      }
    }
  }

  fn on_worker_event(&mut self, event: WorkerEvent) -> Result<(), ExecutionError> {
    match event {
      WorkerEvent::Usage { execution, tokens } => self.on_usage(execution, tokens),
      WorkerEvent::Ended { execution, end } => self.on_ended(execution, end),
      WorkerEvent::CommandEnded { execution, end } => self.on_command_ended(execution, end),
    }
  }

  /// When the time limit is to be checked next: the time left is shared by the agents at work.
  /// `None` for a run without a time limit.
  fn time_check(&self) -> Result<Option<Instant>, ExecutionError> {
    let working = u32::try_from(self.agents.len()).unwrap_or(u32::MAX).max(1);
    Ok(
      self
        .time_left()?
        .map(|left| Instant::now() + (left / working).max(TIME_CHECK_FLOOR)),
    )
  }

  /// Ends every agent once the run's active agent time is used up.
  fn check_time(&mut self) -> Result<(), ExecutionError> {
    let used_up = self.time_left()?.is_some_and(|left| left.is_zero());
    if used_up && self.pending.limit.is_none_or(|limit| !limit.is_run_limit()) {
      self.pending.limit = Some(RunLimit::Time);
      self.stop_agents(Stop::Now(StopCause::Limit(RunLimit::Time)));
    }
    Ok(())
  }

  /// Active agent time left before the time limit; agents working at once each add theirs.
  /// `None` for a run without a time limit, which never stops at one.
  pub(super) fn time_left(&self) -> Result<Option<Duration>, ExecutionError> {
    let Some(limit) = self.run.max_duration() else {
      return Ok(None);
    };
    let invocations = self.db.invocations(self.run.id())?;
    let active = active_time(&invocations, Timestamp::now());
    Ok(Some(limit.saturating_sub(active)))
  }

  /// Tells every agent still counted for the run `stop`. A cancel, a pause or any other stop that ends the agents ends
  /// their calls waiting for the person too: a pause lets a running tool finish first, and such a call would act.
  pub(super) fn stop_agents(&mut self, stop: Stop) {
    if matches!(stop, Stop::Now(_) | Stop::AtApproval) {
      self.browser.stop_person_waits(self.run.id());
    }
    for active in self.agents.values_mut().filter(|active| !active.abandoned) {
      active.tell(stop);
    }
  }

  /// Ends every agent after the runner itself failed, and waits for them to end.
  async fn end_agents(&mut self, error: &ExecutionError) {
    tracing::warn!(run = %self.run.id(), %error, "ending workers after runner failure");
    self.stop_agents(Stop::Now(StopCause::Requested));
    while !self.agents.is_empty() {
      tokio::select! {
        Some(event) = self.events.recv() => {
          if let WorkerEvent::Ended { execution, .. } | WorkerEvent::CommandEnded { execution, .. } = event {
            drop(self.retire_worker(execution));
          }
        }
        control = self.controls.recv(), if self.listening => match control {
          Some(control) => control.reject(error.to_string()),
          None => self.listening = false,
        },
      }
    }
  }

  /// An end event is sent only once, even when recording it failed. Retire its worker before
  /// propagating that error so cleanup never waits for a second event from the same worker.
  /// Its invocation is over, whatever ended it: its QA app stops and frees its QA slot.
  pub(super) fn retire_worker(&mut self, execution: StepExecutionId) -> Option<Active> {
    let active = self.agents.remove(&execution)?;
    self.board.release(execution);
    self.release_steered(execution);
    self.qa.end(active.invocation.id);
    Some(active)
  }

  /// Applies a transition, stores its events with the new state and announces the change. After
  /// a transition that settles the run (it no longer runs) the task executes nothing more: it is
  /// marked settled before the change can be read.
  pub(super) fn transition(
    &mut self,
    transition: impl FnOnce(&mut Run, Timestamp) -> Result<Vec<RunEvent>, RunError>,
  ) -> Result<(), ExecutionError> {
    let settled = &self.settled;
    let settling = |run: &mut Run, now| {
      let events = transition(run, now)?;
      if run.status() != &RunStatus::Running {
        settled.store(true, Ordering::Release);
      }
      Ok(events)
    };
    let was_ready = self.run.status() == &RunStatus::Succeeded;
    record(&mut self.db, &self.notifier, &mut self.run, settling)?;
    if !was_ready {
      announce_ready(&self.notifier, &self.run);
    }
    Ok(())
  }

  /// The task executes the run again after a transition settled it: a follow-up of a run that
  /// just succeeded.
  pub(super) fn executes_again(&self) {
    self.settled.store(false, Ordering::Release);
  }

  /// Reports a problem around `invocation`'s agent in the run's transcript.
  pub(super) fn report(&mut self, invocation: &Invocation, problem: RunnerProblem) -> Result<(), ExecutionError> {
    tracing::warn!(run = %self.run.id(), ?problem, "a step ran into a problem");
    let events = [(Timestamp::now(), AgentEvent::RunnerProblem { problem })];
    let sequence = self.db.append_activity(invocation, &events)?;
    self.notifier.send(Event::AgentActivity {
      run: self.run.id(),
      sequence,
    });
    Ok(())
  }
}
