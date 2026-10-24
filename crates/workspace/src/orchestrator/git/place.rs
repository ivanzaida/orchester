//! Where the chat's git tools run: the project's checkout, or a run's worktree named by its id or number. A run's
//! worktree belongs to its agents while the run has not ended (it works, waits, is paused or interrupted and may go
//! on), so the chat only reads there then; a write is refused (`run_active`).

use std::path::{Path, PathBuf};

use orchester_core::{orchestrator::git::GitPlace, store::WorktreeStore};
use orchester_store::ProjectDb;

use super::command::refused;
use crate::{
  app_mcp::{ToolError, find_run},
  messages::Message,
  tasks::changes::run_worktree,
};

/// A folder git runs in, and what it is.
#[derive(Clone, Debug)]
pub(super) struct Place {
  pub(super) dir: PathBuf,
  pub(super) at: GitPlace,
}

/// Whether a tool only reads where it runs, or writes there.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Use {
  Read,
  Write,
}

/// The place of the project at `root` a call names: its checkout, or the worktree of the run `run` names. Blocks on
/// SQLite.
pub(super) fn resolve(root: &Path, run: Option<&str>, using: Use) -> Result<Place, ToolError> {
  let Some(named) = run else {
    return Ok(Place {
      dir: root.to_path_buf(),
      at: GitPlace::Checkout,
    });
  };
  let db = ProjectDb::open(root)?;
  let run = find_run(&db, named)?;
  let number = run.number().get();
  if using == Use::Write && !run.status().is_terminal() {
    return Err(refused(
      "run_active",
      Message::new("orchestrator.git.runActive").with("run", number.to_string()),
    ));
  }
  let dir = run_worktree(root, run.id());
  if db.worktree_removal(run.id())?.is_some() || !dir.is_dir() {
    return Err(refused(
      "not_a_repository",
      Message::new("orchestrator.git.worktreeGone").with("run", number.to_string()),
    ));
  }
  Ok(Place {
    dir,
    at: GitPlace::Run { run: run.id(), number },
  })
}

/// The folder of `at` in the project at `root`, for a proposal applied later; checked again as a write.
pub(super) fn again(root: &Path, at: GitPlace) -> Result<Place, ToolError> {
  match at {
    GitPlace::Checkout => resolve(root, None, Use::Write),
    GitPlace::Run { run, .. } => resolve(root, Some(&run.to_string()), Use::Write),
  }
}
