use std::{path::Path, time::Duration};

use orchester_core::command::Command;

use super::{LocalGit, LocalGitError};

pub(super) struct GitOutput {
  pub code: Option<i32>,
  pub bytes: Vec<u8>,
}

impl LocalGit {
  pub(super) fn raw(
    &self,
    args: &[&str],
    input: Option<&[u8]>,
    index: Option<&Path>,
  ) -> Result<GitOutput, LocalGitError> {
    self.scoped_raw(args, input, index, None)
  }

  pub(super) fn scoped_raw(
    &self,
    args: &[&str],
    input: Option<&[u8]>,
    index: Option<&Path>,
    worktree: Option<&Path>,
  ) -> Result<GitOutput, LocalGitError> {
    self._root_owner.ensure_current()?;
    self._scratch_owner.ensure_current()?;
    let mut command = Command::new(
      Path::new("git"),
      ["-c", "core.fsmonitor=false"].into_iter().chain(args.iter().copied()),
    )
    .with_current_dir(&self.root)
    .with_env("LC_ALL", "C")
    .with_env("GIT_TERMINAL_PROMPT", "0")
    .with_env("GIT_LITERAL_PATHSPECS", "1")
    .with_env("GIT_OPTIONAL_LOCKS", "0");
    if let Some(action) = &self.ref_action {
      command = command.with_env("GIT_REFLOG_ACTION", action);
    }
    for name in [
      "GIT_DIR",
      "GIT_WORK_TREE",
      "GIT_COMMON_DIR",
      "GIT_CONFIG_COUNT",
      "GIT_CONFIG_PARAMETERS",
      "GIT_OBJECT_DIRECTORY",
      "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    ] {
      command = command.without_env(name);
    }
    if index.is_none() {
      command = command.without_env("GIT_INDEX_FILE");
    }
    if let Some(index) = index {
      let name = super::commit_copy::git_name(index)?;
      command = command.with_env("GIT_INDEX_FILE", &name);
    }
    if let Some(worktree) = worktree {
      command = self.in_commit_copy(&command, worktree)?;
    }
    let mutation = args
      .iter()
      .any(|arg| matches!(*arg, "commit" | "merge" | "restore" | "checkout-index"))
      || (index.is_none() && args.contains(&"update-index"));
    super::execute::invoke(
      &command,
      input,
      Duration::from_secs(60 * 60),
      true,
      if mutation { self.journal.as_deref() } else { None },
    )
  }

  pub fn check_command(&self, command: &Command, timeout: Duration) -> Result<Option<i32>, LocalGitError> {
    Ok(super::execute::invoke(command, None, timeout, false, self.journal.as_deref())?.code)
  }
}
