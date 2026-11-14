//! The commit a team module's branch is at in this checkout: a start's base and a delivery's source; and, for a task
//! in no module, the branch the checkout has checked out with its commit. A start makes a module's branch that is not
//! there yet ([`start_tip`]). What else a start
//! needs of this computer (the connection each profile runs on, the program and the switch of each MCP server, the
//! secret values) is what the computer filled in, read when the bundle is made into the run's inputs
//! (`bundle::inputs`), after readiness said the pipeline can start here ([`crate::readiness`]).

use std::path::Path;

use orchester_core::{
  command::CommandRunner,
  integration::{
    base::{branch_choices, create_branch_at, create_tracking_branch, set_branch_tip},
    checked_out_branch,
  },
  revision::Revision,
  run::{BranchName, CommitId},
};

use crate::tasks::changes::GIT;

/// The branch is not in this checkout, or git cannot read its tip.
#[derive(Debug, thiserror::Error)]
#[error("branch {0} cannot be read here")]
pub struct BranchNotFound(pub String);

/// The commit `branch` is at in the checkout at `root`: a module's branch is a start's base and a delivery's source.
pub fn branch_tip(
  runner: &(dyn CommandRunner + Send + Sync),
  root: &Path,
  branch: &str,
) -> Result<CommitId, BranchNotFound> {
  let missing = || BranchNotFound(branch.to_owned());
  let name = BranchName::new(branch).map_err(|_| missing())?;
  let revision = set_branch_tip(&runner, Path::new(GIT), (root, &name)).map_err(|error| {
    tracing::warn!(%error, "cannot read a team branch's tip");
    missing()
  })?;
  CommitId::new(revision.as_str()).map_err(|_| missing())
}

/// The commit a start takes of a module's `branch` in the checkout at `root`: its tip, the branch being made first
/// when the checkout does not have it (`docs/plan/orchestrator-autonomy/OA-33-…`), so that nobody runs `git branch` by
/// hand before a module's first task starts. A remote has it: its local branch is made, tracking that one. No remote
/// has it: it is made at the tip of the branch the checkout has checked out, the base the module is delivered to. A
/// branch that exists is never moved, and the checkout stays on its branch. Refused when the branch cannot be made:
/// the checkout has no branch checked out, or git refuses.
pub fn start_tip(
  runner: &(dyn CommandRunner + Send + Sync),
  root: &Path,
  branch: &str,
) -> Result<CommitId, BranchNotFound> {
  let missing = || BranchNotFound(branch.to_owned());
  let name = BranchName::new(branch).map_err(|_| missing())?;
  let git = Path::new(GIT);
  let choices = branch_choices(&runner, git, root).map_err(|error| {
    tracing::warn!(%error, "cannot read the branches a team start looks its module's branch up in");
    missing()
  })?;
  if !choices.has_local(&name) {
    let made = match choices.remote.iter().find(|remote| *remote.local() == name) {
      Some(remote) => create_tracking_branch(&runner, git, (root, remote)),
      None => {
        let (_, base) = checked_out_tip(runner, root)?.ok_or_else(missing)?;
        let base = Revision::new(base.as_str()).map_err(|_| missing())?;
        create_branch_at(&runner, git, (root, &name, &base))
      }
    };
    // Another start may have made it meanwhile: the tip is read either way, and tells.
    if let Err(error) = made {
      tracing::info!(%error, branch, "a module's branch was not made by this start");
    }
  }
  branch_tip(runner, root, branch)
}

/// The branch the checkout at `root` has checked out and the commit it is at: where the run of a task in no module
/// starts, and what it merges into. `None` on a detached head, where there is no branch to merge into.
pub fn checked_out_tip(
  runner: &(dyn CommandRunner + Send + Sync),
  root: &Path,
) -> Result<Option<(BranchName, CommitId)>, BranchNotFound> {
  let unread = || BranchNotFound("HEAD".to_owned());
  let Some(branch) = checked_out_branch(&runner, Path::new(GIT), root).map_err(|error| {
    tracing::warn!(%error, "cannot read the branch a team task in no module starts on");
    unread()
  })?
  else {
    return Ok(None);
  };
  let commit = branch_tip(runner, root, branch.as_str())?;
  Ok(Some((branch, commit)))
}
