//! Returning a run's work to one of its agent steps with a note, as the task page and `continue_run` do it (ORC-44):
//! a run that waits for a person goes back with its decision ([`StuckDecision::ReturnTo`], to the agent steps the
//! waiting step runs after); one that ended — succeeded, or failed after it started — is reopened from any of its
//! executed agent steps ([`Command::ReturnRun`]). Refused while the run works, once its work was merged, and once its
//! worktree was removed. *Ask an agent to resolve* on a merge that conflicted offers the steps whose agent may write
//! in the worktree, with the note [`conflict_note`] words.

use std::path::Path;

use orchester_core::{
  permission::RepositoryAccess,
  resource::StepKey,
  run::{AttemptReason, ReviewNote, Run, RunStatus, StuckDecision},
};
use orchester_ipc::{Command, WorkReturn};

use super::{
  changes::run_worktree,
  detail::{RunView, StepView, TaskDetail},
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
}

impl ReturnRefusal {
  /// The code `continue_run` refuses with.
  pub const fn code(self) -> &'static str {
    match self {
      Self::NotEnded => "run_not_ended",
      Self::Merged => "run_merged",
      Self::WorktreeRemoved => "worktree_removed",
    }
  }

  /// What a person reads.
  pub fn message(self) -> Message {
    Message::new(match self {
      Self::NotEnded => "runs.control.notEnded",
      Self::Merged => "runs.control.merged",
      Self::WorktreeRemoved => "runs.control.worktreeRemoved",
    })
  }
}

/// Why the work of `detail`'s latest run `view` cannot go back to a step now, its worktree looked for under
/// `repo_root`; `None` when it can.
pub fn refusal(repo_root: &Path, detail: &TaskDetail, view: &RunView) -> Option<ReturnRefusal> {
  let waits = matches!(view.run.status(), RunStatus::WaitingForPerson(_));
  if !waits && !view.run.can_return_ended() {
    return Some(ReturnRefusal::NotEnded);
  }
  if detail.merge.as_ref().is_some_and(|merge| merge.run == view.id()) {
    return Some(ReturnRefusal::Merged);
  }
  if view.worktree_removed.is_some() || !run_worktree(repo_root, view.id()).is_dir() {
    return Some(ReturnRefusal::WorktreeRemoved);
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

/// The command that returns `view`'s work to `step` with `note`.
pub fn command(view: &RunView, step: StepKey, note: ReviewNote) -> Command {
  match view.run.status() {
    RunStatus::WaitingForPerson(_) => Command::DecideStuckRun {
      run: view.id(),
      decision: StuckDecision::ReturnTo { step, note },
    },
    _ => Command::ReturnRun(WorkReturn {
      run: view.id(),
      step,
      note,
    }),
  }
}

/// What [`conflict_note`] says between the base branch and the files.
const INTO_THE_RUN: &str = " into this branch and resolve the conflicts in ";
/// What [`conflict_note`] says after the files.
const KEEPING: &str = ", keeping both sides' intent; for appended lists such as migrations keep the base's entries \
                       first and put this run's after them; commit the merge.";

/// The note an agent reads when the conflicts of a merge into `base` in `files` go back to it. Written to an agent
/// in English, as prompts are.
pub fn conflict_note(base: &str, files: &[String]) -> ReviewNote {
  let files = if files.is_empty() {
    "the files that conflict".to_owned()
  } else {
    files.join(", ")
  };
  ReviewNote::new(&format!("Merge {base}{INTO_THE_RUN}{files}{KEEPING}"))
    .expect("the note names the merge, so it is never blank")
}

/// A return to resolve a merge's conflicts, as its note ([`conflict_note`]) says it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConflictReturn {
  /// The branch the run merges into.
  pub base: String,
  /// The files that conflicted, as the note lists them.
  pub files: String,
}

/// The conflicts `note` returned the work to resolve, when it is a note [`conflict_note`] wrote.
pub fn conflict_return(note: &ReviewNote) -> Option<ConflictReturn> {
  let (base, rest) = note.as_str().strip_prefix("Merge ")?.split_once(INTO_THE_RUN)?;
  let files = rest.strip_suffix(KEEPING)?;
  Some(ConflictReturn {
    base: base.to_owned(),
    files: files.to_owned(),
  })
}

/// The step `run`'s current attempt returned its work to resolve a merge's conflicts, and what it resolves, while
/// that step is at work or waits to start.
pub fn resolving_conflicts(run: &Run) -> Option<(&StepKey, ConflictReturn)> {
  let attempt = run.current_attempt()?;
  let AttemptReason::ReturnedAfterEnd { return_to, note } = attempt.reason() else {
    return None;
  };
  let working = attempt
    .executions()
    .iter()
    .any(|execution| execution.step() == return_to && execution.status().is_open());
  working.then(|| conflict_return(note).map(|conflict| (return_to, conflict)))?
}
