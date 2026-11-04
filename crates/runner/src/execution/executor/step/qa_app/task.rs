//! The QA app tools a run's invocation gets: its QA folder, `.tmp/qa/run-<number>-<step>/` in its worktree, and the
//! runner note that its start waits for a QA slot.

use std::{path::Path, sync::Arc};

use orchester_core::{
  connection::stream::{AgentEvent, QaSlotHolder, RunnerProblem},
  id::{InvocationId, RunId, StepExecutionId},
  orchester_tools::qa_name,
  store::ActivityStore,
  time::Timestamp,
};
use orchester_ipc::Event;
use orchester_store::{ActivityRecord, ProjectDb, StoreError};
use parking_lot::Mutex;

use super::StepQa;
use crate::execution::{
  Notifier,
  executor::{run::RunTask, sink::Sink, step::start::problem},
};

impl RunTask {
  /// The QA app tools of `invocation`, whose agent works on `execution` in `worktree`.
  pub(in crate::execution::executor::step) fn step_qa(
    &self,
    execution: StepExecutionId,
    invocation: InvocationId,
    worktree: &Path,
  ) -> Result<StepQa, RunnerProblem> {
    let step = self
      .run
      .execution(execution)
      .map(|execution| execution.step().clone())
      .ok_or_else(|| problem("the step execution is not the run's".to_owned()))?;
    let (root, run, notifier) = (self.root.clone(), self.run.id(), self.notifier.clone());
    let sink = Mutex::new(self.sink.another());
    Ok(StepQa {
      apps: Arc::clone(&self.qa),
      state: self.qa.register(invocation),
      name: qa_name(self.run.number(), &step),
      worktree: worktree.to_path_buf(),
      // A link that cannot be had is a setting that cannot be read: each start then takes the fewest slots, as before.
      link: Mutex::new(self.link.another()),
      run_created: self.run.created_at(),
      task: self.task.key().clone(),
      run: self.run.number(),
      waiting: Box::new(move |holders| {
        say_waiting((&root, &mut **sink.lock()), (run, invocation), holders, &notifier);
      }),
    })
  }
}

/// Notes in the run's transcript, through `sink`, that `invocation`'s `start_qa_app` waits for a QA slot, which
/// `holders` hold. A note that cannot be written is logged: the start waits all the same.
fn say_waiting(
  (root, sink): (&Path, &mut dyn Sink),
  (run, invocation): (RunId, InvocationId),
  holders: Vec<QaSlotHolder>,
  notifier: &Notifier,
) {
  let noted = || -> Result<Option<u64>, StoreError> {
    let db = ProjectDb::open(root)?;
    let Some(invocation) = db.invocations(run)?.into_iter().find(|found| found.id == invocation) else {
      return Ok(None);
    };
    let note = AgentEvent::RunnerProblem {
      problem: RunnerProblem::WaitingForQaSlot { holders },
    };
    sink
      .activity(ActivityRecord::new(&invocation, vec![(Timestamp::now(), note)]))
      .map(Some)
  };
  match noted() {
    Ok(Some(sequence)) => notifier.send(Event::AgentActivity { run, sequence }),
    Ok(None) => tracing::warn!(%run, %invocation, "the invocation waiting for a QA slot is not recorded"),
    Err(error) => tracing::warn!(%run, %error, "the wait for a QA slot was not noted in the transcript"),
  }
}
