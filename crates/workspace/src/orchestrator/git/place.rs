//! Where the chat's git tools run, and what they must leave alone there. They run in the project's checkout, or in a
//! run's worktree named by its id or number. A run's worktree belongs to its agents while the run has not ended (it
//! works, waits, is paused or interrupted and may go on), so the chat only reads there then (`run_active`). A write
//! also respects Orchester's own claims (`checkout_reserved`): it holds the project's local-integration lease while it
//! runs when it changes the working tree or the index, as local integration and a run's merge do, and is refused while
//! a local integration reserves the checkout or its Git process has not finished, and in a run's worktree while a
//! resolution of the run's conflicts is recorded or the merge queue holds the run (`run_queued`).

use std::path::{Path, PathBuf};

use orchester_core::{
  integration::local::{LocalIntegrationStore, LocalProcessStore},
  orchestrator::git::GitPlace,
  store::WorktreeStore,
};
use orchester_exec::local_git::IntegrationLease;
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

/// Claims the project at `root` for a write at `place`: refused (`checkout_reserved`) while a local integration
/// reserves the checkout or its Git process has not finished, and in a run's worktree while a resolution of the run's
/// conflicts is recorded; refused (`run_queued`) in a run's worktree while the merge queue holds the run, which syncs
/// and checks there. With `lease`, for a write that changes the working tree or the index, it also takes the
/// local-integration lease, held until the claim is dropped; a push, fetch or tag only checks. Blocks on SQLite.
pub(super) fn claim(root: &Path, place: &Place, lease: bool) -> Result<Option<IntegrationLease>, ToolError> {
  let held = if lease {
    Some(
      IntegrationLease::acquire(root)
        .map_err(|_| refused("checkout_reserved", Message::new("orchestrator.git.integrationWorking")))?,
    )
  } else {
    None
  };
  let db = ProjectDb::open(root)?;
  for operation in db.local_operations()? {
    let unfinished = db
      .local_process(operation.run())?
      .is_some_and(|process| !process.finished);
    if operation.reserves_checkout() || unfinished {
      return Err(refused(
        "checkout_reserved",
        Message::new("orchestrator.git.checkoutReserved"),
      ));
    }
  }
  let GitPlace::Run { run, number } = place.at else {
    return Ok(held);
  };
  if db.resolution(run)?.is_some() {
    return Err(refused(
      "checkout_reserved",
      Message::new("orchestrator.git.resolutionOpen").with("run", number.to_string()),
    ));
  }
  if db.merge_queue_entry(run)?.is_some_and(|entry| entry.state.in_queue()) {
    return Err(refused(
      "run_queued",
      Message::new("orchestrator.git.runQueued").with("run", number.to_string()),
    ));
  }
  Ok(held)
}
