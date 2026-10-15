//! The repository's commit hooks run on a copy of the selected tree and a scratch index, before anything moves:
//! `pre-commit`, then `prepare-commit-msg` and `commit-msg` on the message as `git commit -m` would clean it up.
//! A hook that fails stops the commit; a hook that changes the staged tree or the message stops it too, with
//! what it changed, because the person reviewed exactly that tree and message. `post-commit` runs after the
//! branch moved.
use std::path::Path;

use orchester_core::integration::local::{HookChange, LocalPath};

use super::{LocalGit, LocalGitError, commit_copy::git_name};

/// The reviewed message after the hooks ran, unchanged, in the file the commit reads it from.
pub(super) struct HookedMessage {
  _scope: tempfile::TempDir,
  pub file: String,
}

impl LocalGit {
  pub(super) fn run_commit_hooks(
    &self,
    (index, copy): (&Path, &Path),
    tree: &str,
    message: &str,
  ) -> Result<HookedMessage, LocalGitError> {
    self.hook(index, copy, &["pre-commit"])?;
    self.ensure_hooks_kept(index, tree, false)?;
    let cleaned = self.raw(&["stripspace"], Some(message.as_bytes()), None)?;
    if cleaned.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    let scope = tempfile::Builder::new()
      .prefix("message-")
      .tempdir_in(&self.scratch)
      .map_err(|_| LocalGitError::Io)?;
    let path = scope.path().join("COMMIT_EDITMSG");
    std::fs::write(&path, &cleaned.bytes).map_err(|_| LocalGitError::Io)?;
    let file = git_name(&path)?;
    self.hook(index, copy, &["prepare-commit-msg", "--", &file, "message"])?;
    self.hook(index, copy, &["commit-msg", "--", &file])?;
    let hooked = std::fs::read(&path).map_err(|_| LocalGitError::Io)?;
    self.ensure_hooks_kept(index, tree, hooked != cleaned.bytes)?;
    Ok(HookedMessage { _scope: scope, file })
  }

  /// Runs `post-commit` once the branch moved.
  pub(super) fn post_commit_hook(&self, index: &Path, copy: &Path) -> Result<(), LocalGitError> {
    // Like `git commit`, the hook's exit status does not undo the commit that already moved the branch.
    self.scoped_raw(
      &["hook", "run", "--ignore-missing", "post-commit"],
      None,
      Some(index),
      Some(copy),
    )?;
    Ok(())
  }

  fn hook(&self, index: &Path, copy: &Path, hook: &[&str]) -> Result<(), LocalGitError> {
    let mut args = vec!["hook", "run", "--ignore-missing"];
    args.extend_from_slice(hook);
    let output = self.scoped_raw(&args, None, Some(index), Some(copy))?;
    if output.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    Ok(())
  }

  /// Stops with what the hooks changed when the scratch index no longer writes `tree`, or the message changed.
  fn ensure_hooks_kept(&self, index: &Path, tree: &str, message: bool) -> Result<(), LocalGitError> {
    let hooked = self.index_command(&["write-tree"], None, index)?;
    if hooked == tree && !message {
      return Ok(());
    }
    let changed = self.text(&["diff-tree", "-r", "--name-only", "--no-renames", "-z", tree, &hooked])?;
    // A path the checkout model cannot hold cannot be listed for the person; the stop itself still holds.
    let paths = changed
      .split('\0')
      .filter_map(|path| LocalPath::new(path).ok())
      .collect();
    Err(LocalGitError::HookChanged(HookChange { paths, message }))
  }
}
