//! Approval gates (plan review, 02.11): a gated step's agent that passes proposes its output as the
//! step's next plan, and the step waits for a person ([`orchester_core::run::WaitReason::Plan`]).
//! While nothing else of the run works or can start, its task ends: a waiting run holds no agent
//! slot and no active time. A decision comes as [`Command::DecidePlan`](orchester_ipc::Command):
//! approval continues the graph, requested changes send the step back to its agent, whose next
//! invocation resumes the CLI session with the reviewer's note (see `step::command`). The scheduler's
//! side of the request is `crate::execution::coordinator::plan`'s.

use orchester_core::{
  id::StepExecutionId,
  memory::StepFate,
  run::{Answerer, Plan, PlanDecision, PlanVersion, Run, RunError},
  store::RunStore,
  time::Timestamp,
};
use orchester_ipc::{Event, Rejection};
use orchester_store::{RunLogRecord, RunStoreError};

use super::{
  control::{Ack, answer},
  failed,
  handoff::Keeper,
  record::{RecordError, announce_ready},
  run::{ExecutionError, RunTask},
  sink::Sink,
};
use crate::execution::coordinator::idle;

/// A decision on one plan of a run, and who made it.
#[derive(Clone, Debug)]
pub(in crate::execution) struct PlanRequest {
  pub(in crate::execution) execution: StepExecutionId,
  pub(in crate::execution) version: PlanVersion,
  pub(in crate::execution) decision: PlanDecision,
  pub(in crate::execution) by: Answerer,
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
    RunLogRecord::with_plan(&self.run, &events, &plan)
      .and_then(|record| self.sink.run_log(record))
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
    let keeper = Keeper {
      db: &mut self.db,
      notifier: &self.notifier,
      vault: self.vault.as_ref(),
    };
    let result = record(keeper, self.sink.as_mut(), &mut self.run, request);
    answer(ack, result);
    Ok(())
  }
}

/// Applies `request` to `run`: the plan must be the latest of its execution and still undecided,
/// and the run running with the step waiting for it. Sends the decision with its events to `sink`
/// and announces the change; an approved plan's handoff is then kept as task memory.
pub(in crate::execution) fn record(
  keeper: Keeper<'_>,
  sink: &mut dyn Sink,
  run: &mut Run,
  request: PlanRequest,
) -> Result<(), Rejection> {
  let Keeper { db, notifier, vault } = keeper;
  let plans = db
    .plans(run.id())
    .map_err(|error| failed("cannot read the run's plans", &error))?;
  let mut plan = Plan::latest_of(&plans, request.execution)
    .filter(|plan| plan.version == request.version && plan.review.is_none())
    .cloned()
    .ok_or(Rejection::PlanNotAwaited)?;
  let approved = request.decision == PlanDecision::Approved;
  let events = run
    .decide_plan(&mut plan, request.decision, request.by, Timestamp::now())
    .map_err(rejection)?;
  RunLogRecord::with_plan(run, &events, &plan)
    .and_then(|record| sink.run_log(record))
    .map_err(|error: RunStoreError| failed("cannot record the plan decision", &error))?;
  tracing::info!(
    run = %run.id(),
    execution = %request.execution,
    version = %plan.version,
    by = ?request.by,
    "plan decided"
  );
  if approved {
    // Kept before the decision is announced, so whoever sees it finds the handoff. The decision is
    // recorded and the run goes on; a handoff the database could not take is reported rather than
    // turning the recorded decision into a rejection.
    Keeper { db, notifier, vault }.keep_or_report(run, request.execution, StepFate::Passed);
  }
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
