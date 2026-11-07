//! The commit a team module's branch is at in this checkout: a start's base and a delivery's source. What else a start
//! needs of this computer (the connection each profile runs on, the program and the switch of each MCP server, the
//! secret values) is what the computer filled in, read when the bundle is made into the run's inputs
//! (`bundle::inputs`), after readiness said the pipeline can start here ([`crate::readiness`]).

use std::path::Path;

use orchester_core::{
  command::CommandRunner,
  integration::base::set_branch_tip,
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
