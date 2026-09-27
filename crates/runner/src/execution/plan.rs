//! Approval gates (plan review, 02.11): a gated step's agent that passes proposes its output as the
//! step's next plan, and the step waits for a person ([`orchester_core::run::WaitReason::Plan`]).
//! While nothing else of the run works or can start, its task ends: a waiting run holds no agent
//! slot and no active time. A decision comes as [`Command::DecidePlan`](orchester_ipc::Command):
//! approval continues the graph, requested changes send the step back to its agent, whose next
//! invocation resumes the CLI session with the reviewer's note (see `step::command`).

use orchester_core::{
  id::{RunId, StepExecutionId},
  run::{Plan, PlanDecision, PlanVersion, Run, RunError},
  store::RunStore,
  time::Timestamp,
};
use orchester_ipc::{Event, Rejection};
use orchester_store::{ProjectDb, RunStoreError};

use super::{
  Notifier, Scheduler,
  control::{Ack, Control, answer},
  failed, idle,
  prepare::Prepared,
  record::{RecordError, announce_ready},
  run::{ExecutionError, RunTask},
};

/// A person's decision on one plan of a run.
#[derive(Clone, Debug)]
pub(super) struct PlanRequest {
  pub(super) execution: StepExecutionId,
  pub(super) version: PlanVersion,
  pub(super) decision: PlanDecision,
}

impl Scheduler {
  /// Records `decision` on plan `version` of `execution` in `run` and lets the run go on. Returns
  /// once the decision is recorded and a task executes the run: the task at work took it, or the
  /// run's task, which ended while the run waited, is started again. A decision that is not taken
  /// comes back as the rejection. Needs no free agent slot: the continued run waits for one like
  /// any other.
  pub async fn decide_plan(
    &self,
    run: RunId,
    execution: StepExecutionId,
    version: PlanVersion,
    decision: PlanDecision,
  ) -> Result<(), Rejection> {
    let request = PlanRequest {
      execution,
      version,
      decision,
    };
    let _one_at_a_time = self.inner.continuing.lock().await;
    if let Some(answer) = self.ask(run, |ack| Control::DecidePlan(request.clone(), ack))
      && let Ok(result) = answer.await
    {
      return result;
    }
    // No task executes the run (it waits for the decision), or its task ended without reading it.
    if self.inner.board.is_stopping() {
      return Err(Rejection::ShuttingDown);
    }
    let mut db = self.open()?;
    // A run that cannot continue refuses the decision before it is recorded.
    let mut prepared = Prepared::continuing(&db, &self.inner.project_root, idle::load(&db, run)?)?;
    record(&mut db, &self.inner.notifier, &mut prepared.run, request)?;
    self.spawn(db, prepared)
  }
}

impl RunTask {
  /// A gated step's agent passed with `text` as its result: it becomes the step's next plan and the
  /// step waits for a person's decision.
  pub(super) fn propose(&mut self, execution: StepExecutionId, text: String) -> Result<(), ExecutionError> {
    let earlier = self.db.plans(self.run.id()).map_err(RecordError::from)?;
    let (events, plan) = self
      .run
      .propose_plan(execution, text, &earlier, Timestamp::now())
      .map_err(RecordError::from)?;
    self
      .db
      .append_events_with_plan(&self.run, &events, &plan)
      .map_err(RecordError::from)?;
    tracing::info!(run = %self.run.id(), %execution, version = %plan.version, "plan proposed");
    self.notifier.send(Event::RunChanged {
      run: self.run.id(),
      task: self.run.task(),
    });
    self.notifier.send(Event::PlanProposed {
      run: self.run.id(),
      task: self.run.task(),
      task_key: self.task.key().to_string(),
      version: plan.version,
    });
    Ok(())
  }

  /// Records a decision that came while other steps of the run work; the run goes on from it.
  pub(super) fn decide_plan(&mut self, request: PlanRequest, ack: Ack) -> Result<(), ExecutionError> {
    let result = record(&mut self.db, &self.notifier, &mut self.run, request);
    answer(ack, result);
    Ok(())
  }
}

/// Applies `request` to `run`: the plan must be the latest of its execution and still undecided,
/// and the run running with the step waiting for it. Stores the decision with its events and
/// announces the change.
fn record(db: &mut ProjectDb, notifier: &Notifier, run: &mut Run, request: PlanRequest) -> Result<(), Rejection> {
  let plans = db
    .plans(run.id())
    .map_err(|error| failed("cannot read the run's plans", &error))?;
  let mut plan = Plan::latest_of(&plans, request.execution)
    .filter(|plan| plan.version == request.version && plan.review.is_none())
    .cloned()
    .ok_or(Rejection::PlanNotAwaited)?;
  let events = run
    .decide_plan(&mut plan, request.decision, Timestamp::now())
    .map_err(rejection)?;
  db.append_events_with_plan(run, &events, &plan)
    .map_err(|error: RunStoreError| failed("cannot record the plan decision", &error))?;
  tracing::info!(run = %run.id(), execution = %request.execution, version = %plan.version, "plan decided");
  notifier.send(Event::RunChanged {
    run: run.id(),
    task: run.task(),
  });
  // Approving the plan of the last step finishes the run.
  announce_ready(notifier, run);
  Ok(())
}

/// What the desktop reads for a decision the run does not take now.
fn rejection(error: RunError) -> Rejection {
  match error {
    RunError::InvalidRunTransition { .. } => Rejection::RunNotRunning,
    RunError::InvalidStepTransition { .. } | RunError::PlanAlreadyDecided(_) => Rejection::PlanNotAwaited,
    other => idle::rejection(other),
  }
}
