//! The repository and branch the project's Overview names over its title (`Eyebrow` `YVq5r`): the
//! network address of the user's checkout's `origin` and the branch checked out there. Blocks on git;
//! the UI reads it off the UI thread.

use std::path::Path;

use orchester_core::{
  integration::checked_out_branch,
  repository::{RemoteName, RepositoryIdentity, read_remotes},
  run::BranchName,
};
use orchester_exec::programs::Programs;

use super::changes::GIT;
use crate::messages::Message;

/// What git tells of the checkout; each part is `None`, logged when git fails, when git cannot tell.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Checkout {
  /// `origin`'s address, when it names a network repository; a folder on this computer is not named.
  pub repository: Option<RepositoryIdentity>,
  /// `None` on a detached `HEAD` too.
  pub branch: Option<BranchName>,
}

impl Checkout {
  /// The checkout at `repo_root`.
  pub fn read(programs: &Programs, repo_root: &Path) -> Self {
    let runner = programs.runner();
    let git = Path::new(GIT);
    let repository = match read_remotes(&runner, git, repo_root) {
      Ok(remotes) => {
        let origin = RemoteName::origin();
        remotes
          .iter()
          .find(|remote| remote.name() == &origin)
          .and_then(|remote| RepositoryIdentity::from_remote_url(remote.url()).ok())
      }
      Err(error) => {
        tracing::warn!(%error, "cannot list the checkout's remotes");
        None
      }
    };
    let branch = checked_out_branch(&runner, git, repo_root).unwrap_or_else(|error| {
      tracing::warn!(%error, "cannot read the checkout's branch");
      None
    });
    Self { repository, branch }
  }

  /// The line over the project's title: its key `key`, then the repository and the branch git named.
  pub fn eyebrow(&self, key: &str) -> Message {
    let message = match (&self.repository, &self.branch) {
      (Some(repository), Some(branch)) => Message::new("overview.eyebrow.repositoryBranch")
        .with("repository", repository.as_str().to_owned())
        .with("branch", branch.as_str().to_owned()),
      (Some(repository), None) => {
        Message::new("overview.eyebrow.repository").with("repository", repository.as_str().to_owned())
      }
      (None, Some(branch)) => Message::new("overview.eyebrow.branch").with("branch", branch.as_str().to_owned()),
      (None, None) => Message::new("overview.eyebrow.key"),
    };
    message.with("key", key.to_owned())
  }
}
