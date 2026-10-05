//! What happens when an agent ends. Its work is committed in its worktree; a step that passed and
//! worked beside others is merged back into the run's branch (for command tools as well, see
//! `tool::end`). Its final message becomes its output for the steps after it; the step then passes
//! — a gated one proposes the output as its next plan and waits for a decision — or fails for a
//! reason its failure route handles; once its fate is known, its handoff is kept as task memory
//! (`super::memory`). A failed step ends the attempt, so agents of the same attempt
//! still at work are ended. An agent whose turn ends reads what people wrote first (see
//! `conversation`): its question makes the step wait for the answer, and messages queued for it
//! keep the step going.

use orchester_core::{
  connection::stream::RunnerProblem,
  id::StepExecutionId,
  memory::StepFate,
  revision::Revision,
  run::{Checkpoint, RunLimit, RunStatus, StepFailure, StepOutcome, StepOutput},
  store::ActivityStore,
  time::Timestamp,
};
use orchester_ipc::Event;

use super::{
  Active,
  worker::{Finished, StopCause},
};
use crate::{
  execution::{
    checkpoint::{self, Merge},
    record::RecordError,
    run::{ExecutionError, RunTask},
    workspace::Workspace,
  },
  worktree::{WorktreeError, Worktrees},
};

impl RunTask {
  /// `execution`'s agent ended as `finished` says.
  pub(in super::super) fn on_ended(
    &mut self,
    execution: StepExecutionId,
    finished: &Finished,
  ) -> Result<(), ExecutionError> {
    let Some(active) = self.agents.remove(&execution) else {
      return Ok(());
    };
    self.board.release(execution);
    self.release_steered(execution);
    if active.abandoned {
      return Ok(());
    }
    match finished.stopped {
      // Carried out once every agent has ended; the work stays uncommitted in the worktree.
      Some(StopCause::Requested | StopCause::Abandoned) => Ok(()),
      // No tool decision was sent: save the session's work without completing its step.
      Some(StopCause::Paused) => self.save(execution, &active, "saved while awaiting a tool decision"),
      // The execution stays open and continues with the run's new permissions.
      Some(StopCause::Permissions) => {
        self.restart_for_permissions(execution, &active)?;
        if finished.approval_interrupted {
          self.report(&active.invocation, RunnerProblem::CouldNotStart {
            detail: "A tool decision ended without a confirmed result. Inspect its side effects before explicitly resuming the paused run.".to_owned(),
          })?;
          self.transition(|run, now| run.request_pause(now))?;
        }
        Ok(())
      }
      // The execution stays open: it continues where it stopped once the run continues.
      Some(StopCause::Limit(limit)) => {
        let label = match limit {
          RunLimit::Tokens => "the token limit",
          RunLimit::Time => "the time limit",
          RunLimit::DailyBudget => "the daily budget limit",
          RunLimit::GroupBudget => "the task group budget limit",
          RunLimit::StepBudget { .. } => {
            let budget = active.budget().map_or(0, |budget| budget.get());
            self.report(&active.invocation, RunnerProblem::StepBudgetReached { budget })?;
            "the step budget"
          }
        };
        self.save(execution, &active, &format!("work in progress at {label}"))
      }
      None => self.finish(execution, &active, finished),
    }
  }

  /// Commits the work of an agent stopped to take changed permissions as a checkpoint and records
  /// that it restarts with them; its step then continues as a new invocation, resuming its session.
  fn restart_for_permissions(&mut self, execution: StepExecutionId, active: &Active) -> Result<(), ExecutionError> {
    let before = self.run.last_checkpoint().map(Checkpoint::id);
    self.save(execution, active, "saved to apply permissions")?;
    let checkpoint = self
      .run
      .last_checkpoint()
      .map(Checkpoint::id)
      .filter(|saved| Some(*saved) != before);
    // Kept for the next invocation after it had stopped: it restarts all the same, as that
    // invocation.
    if self.run.restarts_for_permissions(execution) {
      self.transition(|run, now| run.apply_permissions(execution, checkpoint, now))?;
    }
    Ok(())
  }

  /// An agent that exited on its own. A turn that ended, or delivered a question, acknowledges the
  /// messages it read. The step then waits for the answer to the agent's question — also when its turn
  /// failed afterwards — or continues to read messages queued for its agent meanwhile; otherwise it
  /// fails when the agent reported `failed` — also when its turn failed afterwards — or when the turn
  /// failed, and passes otherwise; a step that passes
  /// joins its work to the run's branch. A follow-up leaves no output: its answer is its
  /// reply in the conversation, and the step's output stays what it was.
  fn finish(&mut self, execution: StepExecutionId, active: &Active, finished: &Finished) -> Result<(), ExecutionError> {
    let step = self
      .run
      .execution(execution)
      .expect("an agent works on an execution of the run")
      .step()
      .clone();
    let output = StepOutput::from_final_message(
      self.run.id(),
      execution,
      step,
      finished.final_message.as_deref(),
      Timestamp::now(),
    );
    self.note_cli_too_old(execution, active, &finished.outcome)?;
    let failure = output.failure(finished.outcome.is_success());
    if finished.outcome.is_success() || output.outcome == Some(StepOutcome::Question) {
      self.acknowledge(execution, &active.invocation, &output)?;
    }
    if failure.is_none() {
      if output.outcome == Some(StepOutcome::Question) {
        return self.ask(execution, active, &output.text);
      }
      if !self.queued_for(execution)?.is_empty() {
        return self.save(execution, active, "saved to read messages");
      }
    }
    if (output.outcome.is_some() || !output.text.is_empty()) && !self.run.is_follow_up(execution) {
      self.db.save_step_output(&output)?;
    }
    if let Some(failure) = failure {
      self.save(execution, active, "step failed")?;
      self.keep_handoff(execution, StepFate::Failed);
      return self.fail(execution, failure);
    }
    self.pass(execution, active, output.text)
  }

  /// A step its agent or command finished with `output`: its work joins the run's branch and the
  /// step passes — a gated one proposes `output` as its next plan — or it fails when its work cannot
  /// be committed or merged.
  pub(in super::super) fn pass(
    &mut self,
    execution: StepExecutionId,
    active: &Active,
    output: String,
  ) -> Result<(), ExecutionError> {
    match self.integrate(execution, active) {
      Ok(Integrated::Done) if self.run.is_gated(execution).map_err(RecordError::from)? => {
        self.propose(execution, output)
      }
      Ok(Integrated::Done) => {
        self.keep_handoff(execution, StepFate::Passed);
        self.transition(|run, now| run.complete_step(execution, now))
      }
      Ok(Integrated::Conflict { branch, files }) => {
        self.report(&active.invocation, RunnerProblem::MergeConflict { branch, files })?;
        self.keep_handoff(execution, StepFate::Failed);
        self.fail(execution, StepFailure::MergeConflict)
      }
      Err(IntegrateError::Execution(error)) => Err(error),
      Err(IntegrateError::Git(error)) => {
        let problem = RunnerProblem::CheckpointFailed {
          detail: error.to_string(),
        };
        self.report(&active.invocation, problem)?;
        self.keep_handoff(execution, StepFate::Failed);
        self.fail(execution, StepFailure::CheckpointFailed)
      }
    }
  }

  /// Commits a passed step's work and brings it onto the run's branch: in the run's worktree it is
  /// there already; from a worktree of its own it is merged, and that worktree removed.
  fn integrate(&mut self, execution: StepExecutionId, active: &Active) -> Result<Integrated, IntegrateError> {
    let message = self.commit_message(execution, None);
    let committed = checkpoint::commit(active.workspace.path(), &message)?;
    let Workspace::Own { branch, owner, .. } = &active.workspace else {
      if let Some(revision) = committed {
        self.checkpoint(execution, revision)?;
      }
      return Ok(Integrated::Done);
    };
    let before = checkpoint::head(&self.worktree)?;
    let message = self.commit_message(execution, Some("merge"));
    let merged = match checkpoint::merge(&self.worktree, branch, &message)? {
      Merge::Merged(revision) => revision,
      Merge::Conflict(files) => {
        return Ok(Integrated::Conflict {
          branch: branch.clone(),
          files,
        });
      }
    };
    if merged != before {
      self.checkpoint(execution, merged)?;
    }
    // The step's commits are on the run's branch now; a worktree left behind only takes space.
    if let Err(error) = Worktrees::new(&self.root).and_then(|worktrees| worktrees.remove_merged_step(*owner, branch)) {
      tracing::warn!(run = %self.run.id(), %execution, %error, "the merged step's worktree was not removed");
    }
    Ok(Integrated::Done)
  }

  /// Commits what an agent that did not pass changed, in its own worktree, as a checkpoint of its
  /// execution: nothing is discarded, and the run's worktree stays clean for later merges. A commit
  /// that fails is reported; the changes stay uncommitted.
  pub(in super::super) fn save(
    &mut self,
    execution: StepExecutionId,
    active: &Active,
    what: &str,
  ) -> Result<(), ExecutionError> {
    let message = self.commit_message(execution, Some(what));
    match checkpoint::commit(active.workspace.path(), &message) {
      Ok(Some(revision)) => self.checkpoint(execution, revision),
      Ok(None) => Ok(()),
      Err(error) => {
        let problem = RunnerProblem::CheckpointFailed {
          detail: error.to_string(),
        };
        self.report(&active.invocation, problem)
      }
    }
  }

  fn checkpoint(&mut self, execution: StepExecutionId, revision: Revision) -> Result<(), ExecutionError> {
    self.transition(|run, now| run.record_checkpoint(execution, revision, now))
  }

  /// Fails `execution` for `failure` and routes it; agents of executions the failure ended are
  /// ended.
  pub(in super::super) fn fail(
    &mut self,
    execution: StepExecutionId,
    failure: StepFailure,
  ) -> Result<(), ExecutionError> {
    self.transition(|run, now| run.fail_step(execution, failure, now))?;
    self.announce_waiting();
    let run = &self.run;
    for (id, active) in &mut self.agents {
      let open = run
        .current_attempt()
        .and_then(|attempt| attempt.execution(*id))
        .is_some_and(|execution| execution.status().is_open());
      if !open && !active.abandoned {
        active.abandon();
      }
    }
    Ok(())
  }

  /// Tells the desktop that the run waits for a person's decision, if a failure just made it wait.
  fn announce_waiting(&self) {
    let RunStatus::WaitingForPerson(stuck) = self.run.status() else {
      return;
    };
    let step = self
      .run
      .snapshot()
      .body()
      .step(stuck.step())
      .map_or_else(|| stuck.step().to_string(), |step| step.name.as_str().to_owned());
    self.notifier.send(Event::RunWaitsForPerson {
      run: self.run.id(),
      task: self.run.task(),
      task_key: self.task.key().to_string(),
      step,
    });
  }

  /// "ORC-129: Implement (run 128)", or with `what` happened: "ORC-129: Implement, step failed
  /// (run 128)".
  fn commit_message(&self, execution: StepExecutionId, what: Option<&str>) -> String {
    let step = self
      .run
      .execution(execution)
      .and_then(|execution| self.run.snapshot().body().step(execution.step()))
      .map_or_else(String::new, |step| step.name.as_str().to_owned());
    let run = self.run.number();
    match what {
      Some(what) => format!("{}: {step}, {what} (run {run})", self.task.key()),
      None => format!("{}: {step} (run {run})", self.task.key()),
    }
  }
}

enum Integrated {
  Done,
  Conflict { branch: String, files: Vec<String> },
}

#[derive(Debug, thiserror::Error)]
enum IntegrateError {
  #[error(transparent)]
  Git(#[from] WorktreeError),
  #[error(transparent)]
  Execution(#[from] ExecutionError),
}
