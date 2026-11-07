//! *Ask the agent to resolve* (design 11 `CT6Gc`): the run whose approved merge found conflicts goes back to Implement,
//! its writing step, as *Ask an agent to resolve* on a merge that conflicted sends it
//! ([`return_work::resolve_command`]): the runner merges the base branch into the agent's worktree, the agent resolves
//! what conflicts and the run waits for review again. The command is made from what is stored ([`begin`]) and then
//! handed to the project's runner ([`send`]).

use std::path::Path;

use orchester_core::{
  id::{ResourceId, RunId},
  run::Answerer,
};
use orchester_ipc::Command;
use orchester_store::ProjectDb;

use super::standing;
use crate::{
  runner::Runners,
  tasks::{
    detail::TaskDetail,
    resolve::{self, RunWorktrees},
    return_work,
    review_threads::{ReviewRefusal, chosen, waiting},
    run::control_message,
  },
  workspace::OpenProject,
};

/// The command that returns `run`'s work of the project at `root` to its implementing step to resolve the conflicts of
/// its last merge attempt, to be handed to the runner with [`send`]. Refused while the run does not wait for review,
/// when its last attempt found no conflicts that still stand, and as a return of its work is refused. The run's
/// worktree is read through `worktrees`.
pub fn begin(
  root: &Path,
  worktrees: &RunWorktrees,
  run: RunId,
  profile: Option<ResourceId>,
) -> Result<Command, ReviewRefusal> {
  let db = ProjectDb::open(root)?;
  let stored = waiting(&db, run)?;
  let conflicts = standing(&db, &stored)?.ok_or(ReviewRefusal::NoConflicts)?;
  let chosen = chosen(&db, profile)?;
  drop(db);
  let detail = TaskDetail::load_in(root, stored.task())?;
  let view = detail
    .run
    .as_ref()
    .filter(|view| view.id() == run)
    .ok_or(ReviewRefusal::NotWaiting)?;
  let step = return_work::implementing_step(view).ok_or(ReviewRefusal::NoImplementingStep)?;
  let worktree = resolve::read(worktrees, root, (run, false)).map_err(ReviewRefusal::Store)?;
  if let Some(refusal) = return_work::refusal(&detail, view, &worktree) {
    return Err(ReviewRefusal::Returned(refusal));
  }
  return_work::resolve_command(view, step, (&conflicts.files, None), (chosen, Answerer::Person))
    .ok_or(ReviewRefusal::NoConflicts)
}

/// Hands `command`, made by [`begin`], to the runner of `open`, starting it when the project has none. The refusal
/// says why the runner did not take it; nothing was changed then.
pub async fn send(runners: &Runners, open: &OpenProject, command: Command) -> Result<(), ReviewRefusal> {
  runners.control_run_in(open, command).await.map_err(|error| {
    tracing::info!(%error, "the runner did not take the return to resolve conflicts");
    ReviewRefusal::Store(control_message(&error))
  })
}
