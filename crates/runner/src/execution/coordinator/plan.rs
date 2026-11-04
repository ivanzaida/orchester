//! A person's decision on a plan (`crate::execution::executor::plan`), as the scheduler takes it: the task executing
//! the run records it, or, while none does, it is recorded here and the run's task is started again.

use orchester_core::{
  id::{RunId, StepExecutionId},
  run::{Answerer, PlanDecision, PlanVersion},
};
use orchester_ipc::Rejection;

use super::{Scheduler, idle, prepare::Prepared, sink::LocalSink};
use crate::execution::executor::{
  control::Control,
  handoff::Keeper,
  plan::{PlanRequest, record},
};

impl Scheduler {
  /// Records `by`'s `decision` on plan `version` of `execution` in `run` and lets the run go on.
  /// Returns once the decision is recorded and a task executes the run: the task at work took it, or
  /// the run's task, which ended while the run waited, is started again. A decision that is not taken
  /// comes back as the rejection. Needs no free agent slot: the continued run waits for one like
  /// any other.
  pub async fn decide_plan(
    &self,
    run: RunId,
    execution: StepExecutionId,
    version: PlanVersion,
    decision: PlanDecision,
    by: Answerer,
  ) -> Result<(), Rejection> {
    let request = PlanRequest {
      execution,
      version,
      decision,
      by,
    };
    loop {
      let one_at_a_time = self.inner.continuing.lock().await;
      if self.inner.board.is_stopping() {
        return Err(Rejection::ShuttingDown);
      }
      if let Some(answer) = self.ask(run, |ack| Control::DecidePlan(request.clone(), ack)) {
        drop(one_at_a_time);
        if let Ok(result) = answer.await {
          return result;
        }
        continue;
      }
      return self.decide_idle(run, request);
    }
  }

  fn decide_idle(&self, run: RunId, request: PlanRequest) -> Result<(), Rejection> {
    // No task executes the run (it waits for the decision), or its task ended without reading it.
    let mut db = self.open()?;
    // A run that cannot continue refuses the decision before it is recorded.
    let mut prepared = Prepared::continuing(&db, &self.inner.project_root, idle::load(&db, run)?)?;
    let mut sink = LocalSink::new(&self.inner.project_root);
    let keeper = Keeper {
      db: &mut db,
      sink: &mut sink,
      notifier: &self.inner.notifier,
      vault: self.inner.vault.as_ref(),
    };
    record(keeper, &mut prepared.run, request)?;
    self.spawn(db, prepared)
  }
}
