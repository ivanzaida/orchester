//! Returning a run's work to one of its agent steps with a note, as the task page and `continue_run` do it:
//! a run that waits for a person goes back with its decision ([`StuckDecision::ReturnTo`], to the agent steps the
//! waiting step runs after); one that ended — succeeded, or failed after it started — is reopened from any of its
//! executed agent steps ([`Command::ReturnRun`]; the project task ORC-44 · Return a finished run to an earlier step).
//! Refused while the run works, once its work was merged, once its worktree was removed, and while a merge waits in its
//! worktree. *Ask an agent to resolve* on a merge that conflicted offers the steps whose agent may write in the
//! worktree, with the note [`conflict_note`] words and the conflicts as data.

use orchester_core::{
  permission::RepositoryAccess,
  resource::StepKey,
  run::{Answerer, AttemptReason, BranchName, MergeConflicts, ReviewNote, Run, RunStatus, StuckDecision},
};
use orchester_ipc::{Command, WorkReturn};

use super::{
  detail::{RunView, StepView, TaskDetail},
  resolve::Worktree,
};
use crate::messages::Message;

/// Why the work of a run cannot go back to a step now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReturnRefusal {
  /// The run works, waits to start, is paused or interrupted: it has not ended.
  NotEnded,
  /// The run's work was merged into its base branch.
  Merged,
  /// The run's worktree was removed, so no agent can work in it again.
  WorktreeRemoved,
  /// A merge waits in the run's worktree for a person to resolve it.
  InMerge,
}

impl ReturnRefusal {
  /// The code `continue_run` refuses with.
  pub const fn code(self) -> &'static str {
    match self {
      Self::NotEnded => "run_not_ended",
      Self::Merged => "run_merged",
      Self::WorktreeRemoved => "worktree_removed",
      Self::InMerge => "worktree_in_merge",
    }
  }

  /// What a person reads.
  pub fn message(self) -> Message {
    Message::new(match self {
      Self::NotEnded => "runs.control.notEnded",
      Self::Merged => "runs.control.merged",
      Self::WorktreeRemoved => "runs.control.worktreeRemoved",
      Self::InMerge => "runs.control.worktreeInMerge",
    })
  }
}

/// Why the work of `detail`'s latest run `view` cannot go back to a step now, its `worktree` as it was read; `None`
/// when it can.
pub fn refusal(detail: &TaskDetail, view: &RunView, worktree: &Worktree) -> Option<ReturnRefusal> {
  let waits = matches!(view.run.status(), RunStatus::WaitingForPerson(_));
  if !waits && !view.run.can_return_ended() {
    return Some(ReturnRefusal::NotEnded);
  }
  if detail.merge.as_ref().is_some_and(|merge| merge.run == view.id()) {
    return Some(ReturnRefusal::Merged);
  }
  if view.worktree_removed.is_some() || !worktree.present {
    return Some(ReturnRefusal::WorktreeRemoved);
  }
  if worktree.merge.is_some() {
    return Some(ReturnRefusal::InMerge);
  }
  None
}

/// The agent steps `view`'s work may go back to, in pipeline order.
pub fn targets(view: &RunView) -> Vec<StepKey> {
  match view.run.status() {
    RunStatus::WaitingForPerson(_) => view.run.return_targets(),
    _ => view.run.ended_return_targets(),
  }
}

/// The steps of [`targets`] whose agent may write in the worktree in its next invocation, as the run's pinned
/// pipeline version, its profile version and any change of its permissions for this run give them.
pub fn writing_targets(view: &RunView) -> Vec<&StepView> {
  let targets = targets(view);
  view
    .steps
    .iter()
    .filter(|step| targets.contains(&step.key))
    .filter(|step| {
      step
        .permissions
        .as_ref()
        .is_some_and(|permissions| permissions.permissions.repository() == RepositoryAccess::Write)
    })
    .collect()
}

/// The writing step a conflict goes back to unless the person picks another: the one whose work the run's last
/// checkpoint holds, which implemented what conflicts; else the last writing step.
pub fn implementing_step(view: &RunView) -> Option<StepKey> {
  let writing = writing_targets(view);
  let checkpointed = view
    .run
    .last_checkpoint()
    .and_then(|checkpoint| view.run.execution(checkpoint.execution()))
    .map(|execution| execution.step().clone());
  checkpointed
    .filter(|step| writing.iter().any(|view| view.key == *step))
    .or_else(|| writing.last().map(|view| view.key.clone()))
}

/// The command that returns `view`'s work to `step` with `note`, to resolve `conflicts` when that is what it is for,
/// as `by` decides it on the run as `view` read it.
pub fn command(
  view: &RunView,
  step: StepKey,
  (note, conflicts): (ReviewNote, Option<MergeConflicts>),
  by: Answerer,
) -> Command {
  match view.run.status() {
    RunStatus::WaitingForPerson(_) => Command::DecideStuckRun {
      run: view.id(),
      decision: StuckDecision::ReturnTo { step, note },
    },
    _ => Command::ReturnRun(WorkReturn {
      run: view.id(),
      step,
      note,
      conflicts,
      by,
      expected: view.run.last_sequence(),
    }),
  }
}

/// The note an agent reads when the conflicts of a merge into `base` go back to it; the files that conflicted reach
/// its prompt as data beside it ([`MergeConflicts`]), never in its words. Written to an agent in English, as prompts
/// are.
pub fn conflict_note(base: &BranchName) -> ReviewNote {
  let base = base.as_str();
  ReviewNote::new(&format!(
    "Merge {base} into this branch and resolve the conflicts listed below, keeping both sides' intent; for appended \
     lists such as migrations keep the base's entries first and put this run's after them; commit the merge."
  ))
  .expect("the note names the merge, so it is never blank")
}

/// The step `run`'s current attempt returned its work to, to resolve a merge's conflicts, and what it resolves,
/// while that step is at work or waits to start.
pub fn resolving_conflicts(run: &Run) -> Option<(&StepKey, &MergeConflicts)> {
  let attempt = run.current_attempt()?;
  let AttemptReason::ReturnedAfterEnd {
    return_to,
    conflicts: Some(conflicts),
    ..
  } = attempt.reason()
  else {
    return None;
  };
  attempt
    .executions()
    .iter()
    .any(|execution| execution.step() == return_to && execution.status().is_open())
    .then_some((return_to, conflicts))
}
