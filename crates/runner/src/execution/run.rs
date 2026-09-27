//! Driving one run: every step execution whose dependencies have passed starts, as far as agent
//! slots allow, and runs beside the others until the run succeeds or fails, or settles: paused at
//! a checkpoint, stopped at a limit, cancelled or interrupted. The task owns the run's state: each
//! transition is appended to the run's log and announced here, while each agent reports its
//! usage and its end from a task of its own ([`super::step::worker`]). Requests for the run
//! (pause, cancel, the runner stopping) arrive as [`Control`]s.

use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

use orchester_core::{
  connection::stream::{AgentEvent, RunnerProblem},
  id::StepExecutionId,
  project::Project,
  run::{InterruptCause, Invocation, Run, RunError, RunEvent, RunLimit, active_time},
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
  control::{Ack, Control, answer},
  prepare::Prepared,
  record::{RecordError, record},
  step::{
    Active,
    worker::{Stop, StopCause, WorkerEvent},
  },
};

/// How long a run waiting for a free agent slot waits before it looks again, in case the slot
/// was freed by another run in a way it was not woken for.
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
  #[error("cannot write the step result schema: {0}")]
  Schema(std::io::Error),
  #[error("the run has steps left but none can run")]
  Stalled,
}

/// What the task of a run is given besides the run: where it announces changes and where its
/// requests arrive.
pub(super) struct TaskContext {
  pub(super) root: PathBuf,
  pub(super) notifier: Notifier,
  pub(super) board: Arc<Board>,
  pub(super) controls: mpsc::UnboundedReceiver<Control>,
}

/// Requests received and not carried out yet; they take effect once every agent has ended.
#[derive(Default)]
pub(super) struct Pending {
  pub(super) cancel: Option<Ack>,
  pub(super) interrupt: Option<InterruptCause>,
  /// A limit the run reached while agents worked.
  pub(super) limit: Option<RunLimit>,
}

pub(super) struct RunTask {
  pub(super) root: PathBuf,
  pub(super) db: ProjectDb,
  pub(super) run: Run,
  pub(super) task: Task,
  pub(super) project: Project,
  /// The run's worktree, on the run's branch.
  pub(super) worktree: PathBuf,
  pub(super) notifier: Notifier,
  pub(super) board: Arc<Board>,
  pub(super) controls: mpsc::UnboundedReceiver<Control>,
  listening: bool,
  pub(super) pending: Pending,
  /// Agents at work, by the execution they work on.
  pub(super) agents: HashMap<StepExecutionId, Active>,
  pub(super) worker_events: mpsc::UnboundedSender<WorkerEvent>,
  events: mpsc::UnboundedReceiver<WorkerEvent>,
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
      worktree: prepared.worktree,
      notifier: context.notifier,
      board: context.board,
      controls: context.controls,
      listening: true,
      pending: Pending::default(),
      agents: HashMap::new(),
      worker_events,
      events,
    }
  }

  /// Executes the run until it ends or settles. A failure of the runner itself (the database, a
  /// rejected transition) ends the run's agents and leaves the run as last recorded, and is
  /// logged; its steps are not retried.
  pub(super) async fn drive(mut self) {
    let run = self.run.id();
    tracing::info!(%run, "run executing");
    let result = self.steps().await;
    if result.is_err() {
      self.end_agents().await;
    }
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
  }

  async fn steps(&mut self) -> Result<(), ExecutionError> {
    self.write_result_schema()?;
    loop {
      while let Ok(control) = self.controls.try_recv() {
        self.control(control)?;
      }
      if self.agents.is_empty() && self.settle()? {
        return Ok(());
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

  /// Waits for an agent's news, a request, the time limit or a free slot.
  async fn wait(&mut self, waiting_for_slot: bool) -> Result<(), ExecutionError> {
    let working = !self.agents.is_empty();
    let deadline = if working {
      self.time_check()?
    } else {
      Instant::now() + SLOT_RETRY
    };
    tokio::select! {
      Some(event) = self.events.recv() => self.on_worker_event(event),
      control = self.controls.recv(), if self.listening => match control {
        Some(control) => self.control(control),
        None => {
          self.listening = false;
          Ok(())
        }
      },
      () = tokio::time::sleep_until(deadline), if working || waiting_for_slot => {
        if working {
          self.check_time()?;
        }
        Ok(())
      }
    }
  }

  fn on_worker_event(&mut self, event: WorkerEvent) -> Result<(), ExecutionError> {
    match event {
      WorkerEvent::Usage { execution, tokens } => self.on_usage(execution, tokens),
      WorkerEvent::Ended { execution, end } => self.on_ended(execution, &end?),
    }
  }

  /// When the time limit is to be checked next: the time left is shared by the agents at work.
  fn time_check(&self) -> Result<Instant, ExecutionError> {
    let working = u32::try_from(self.agents.len()).unwrap_or(u32::MAX).max(1);
    let share = (self.time_left()? / working).max(TIME_CHECK_FLOOR);
    Ok(Instant::now() + share)
  }

  /// Ends every agent once the run's active agent time is used up.
  fn check_time(&mut self) -> Result<(), ExecutionError> {
    if self.time_left()?.is_zero() && self.pending.limit.is_none_or(|limit| !limit.is_run_limit()) {
      self.pending.limit = Some(RunLimit::Time);
      self.stop_agents(Stop::Now(StopCause::Limit(RunLimit::Time)));
    }
    Ok(())
  }

  /// Active agent time left before the time limit; agents working at once each add theirs.
  pub(super) fn time_left(&self) -> Result<Duration, ExecutionError> {
    let invocations = self.db.invocations(self.run.id())?;
    let active = active_time(&invocations, Timestamp::now());
    Ok(self.run.max_duration().saturating_sub(active))
  }

  /// Tells every agent still counted for the run `stop`.
  pub(super) fn stop_agents(&mut self, stop: Stop) {
    for active in self.agents.values_mut().filter(|active| !active.abandoned) {
      active.tell(stop);
    }
  }

  /// Ends every agent after the runner itself failed, and waits for them to end.
  async fn end_agents(&mut self) {
    self.stop_agents(Stop::Now(StopCause::Requested));
    while !self.agents.is_empty() {
      let Some(event) = self.events.recv().await else {
        return;
      };
      if let WorkerEvent::Ended { execution, .. } = event
        && self.agents.remove(&execution).is_some()
      {
        self.board.release(execution);
      }
    }
  }

  /// Writes the schema new agent sessions report their result in, where `--output-schema` finds
  /// it: next to the project database, outside every worktree.
  fn write_result_schema(&self) -> Result<(), ExecutionError> {
    let path = self.schema_path();
    // A missing or unreadable schema file is simply written again.
    let current = std::fs::read_to_string(&path).ok();
    if current.as_deref() != Some(orchester_core::run::STEP_RESULT_SCHEMA) {
      std::fs::write(&path, orchester_core::run::STEP_RESULT_SCHEMA).map_err(ExecutionError::Schema)?;
    }
    Ok(())
  }

  pub(super) fn schema_path(&self) -> PathBuf {
    self.root.join(".orchester").join("step-result.schema.json")
  }

  /// Applies a transition, stores its events with the new state and announces the change.
  pub(super) fn transition(
    &mut self,
    transition: impl FnOnce(&mut Run, Timestamp) -> Result<Vec<RunEvent>, RunError>,
  ) -> Result<(), ExecutionError> {
    Ok(record(&mut self.db, &self.notifier, &mut self.run, transition)?)
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
