use std::path::{Path, PathBuf};

use orchester_core::command::Command;

use super::{LocalError, LocalGit, LocalGitError};

impl LocalGit {
  pub(super) fn quiet_index(
    &self,
    args: &[&str],
    input: Option<&[u8]>,
    index: &Path,
  ) -> Result<super::process::GitOutput, LocalGitError> {
    self.quiet_plumbing(args, input, Some(index))
  }

  pub(super) fn quiet_plumbing(
    &self,
    args: &[&str],
    input: Option<&[u8]>,
    index: Option<&Path>,
  ) -> Result<super::process::GitOutput, LocalGitError> {
    let hooks = tempfile::Builder::new()
      .prefix("index-hooks-")
      .tempdir_in(&self.scratch)
      .map_err(|_| LocalGitError::Io)?;
    let setting = format!("core.hooksPath={}", git_name(hooks.path())?);
    let mut configured = vec!["-c", setting.as_str()];
    configured.extend_from_slice(args);
    self.raw(&configured, input, index)
  }

  /// Relative hook writes run against a verified copy of the selected tree, never the user's working files.
  pub(super) fn commit_copy(&self, index: &Path) -> Result<tempfile::TempDir, LocalGitError> {
    let tree = self.index_command(&["write-tree"], None, index)?;
    if self
      .tree_entries(&tree)?
      .values()
      .any(|entry| !matches!(entry.mode, 0o100644 | 0o100755))
    {
      return Err(LocalError::Unsupported.into());
    }
    let directory = tempfile::Builder::new()
      .prefix("commit-")
      .tempdir_in(&self.scratch)
      .map_err(|_| LocalGitError::Io)?;
    let copied = self.scoped_raw(
      &["checkout-index", "--all", "--force"],
      None,
      Some(index),
      Some(directory.path()),
    )?;
    if copied.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    Ok(directory)
  }

  pub(super) fn add_untracked_in_copy(
    &self,
    index: &Path,
    path: &super::LocalPath,
    executable: bool,
  ) -> Result<(), LocalGitError> {
    let directory = self.commit_copy(index)?;
    let hash = super::copy_untracked_file(&self.root, directory.path(), path, executable)?;
    let output = self.scoped_raw(&["add", "--", path.as_str()], None, Some(index), Some(directory.path()))?;
    if output.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    let chmod = if cfg!(unix) && executable {
      "--chmod=+x"
    } else {
      "--chmod=-x"
    };
    self.index_command(&["update-index", chmod, "--", path.as_str()], None, index)?;
    let tree = self.index_command(&["write-tree"], None, index)?;
    let copied = self.tree_entries(&tree)?;
    let mode = if cfg!(unix) && executable { 0o100755 } else { 0o100644 };
    if copied.get(path).is_none_or(|entry| entry.mode != mode)
      || super::file_hash(directory.path(), path)?.as_ref() != Some(&hash)
      || super::executable(directory.path(), path)? != (cfg!(unix) && executable)
    {
      return Err(LocalError::Reconcile.into());
    }
    Ok(())
  }

  pub(super) fn in_commit_copy(&self, command: &Command, worktree: &Path) -> Result<Command, LocalGitError> {
    let worktree = worktree.canonicalize().map_err(|_| LocalGitError::Io)?;
    if !worktree.starts_with(&self.scratch) {
      return Err(LocalError::Unsupported.into());
    }
    let git_dir = self.text(&["rev-parse", "--absolute-git-dir"])?;
    let hooks = self.hook_directory()?;
    let configuration = format!("core.hooksPath={}", git_name(&hooks)?);
    let args = [
      "-c".to_owned(),
      configuration,
      "-c".to_owned(),
      "core.longpaths=true".to_owned(),
    ]
    .into_iter()
    .chain(command.args().iter().cloned());
    let index = command
      .env()
      .iter()
      .find(|(key, _)| key == "GIT_INDEX_FILE")
      .map(|(_, value)| value.as_str())
      .ok_or(LocalGitError::Data)?;
    Ok(
      Command::new(Path::new("git"), args)
        .with_current_dir(&worktree)
        .with_env("LC_ALL", "C")
        .with_env("GIT_TERMINAL_PROMPT", "0")
        .with_env("GIT_LITERAL_PATHSPECS", "1")
        .with_env("GIT_OPTIONAL_LOCKS", "0")
        .with_env("GIT_DIR", &git_name(Path::new(&git_dir))?)
        .with_env("GIT_WORK_TREE", &git_name(&worktree)?)
        .with_env("GIT_INDEX_FILE", index)
        .without_env("GIT_COMMON_DIR")
        .without_env("GIT_CONFIG_COUNT")
        .without_env("GIT_CONFIG_PARAMETERS")
        .without_env("GIT_OBJECT_DIRECTORY")
        .without_env("GIT_ALTERNATE_OBJECT_DIRECTORIES"),
    )
  }

  fn hook_directory(&self) -> Result<PathBuf, LocalGitError> {
    let output = self.raw(&["config", "--path", "--get", "core.hooksPath"], None, None)?;
    match output.code {
      Some(1) => Ok(PathBuf::from(self.text(&[
        "rev-parse",
        "--path-format=absolute",
        "--git-path",
        "hooks",
      ])?)),
      Some(0) => {
        let path = String::from_utf8(output.bytes).map_err(|_| LocalGitError::Data)?;
        let path = Path::new(path.trim_end_matches(['\r', '\n']));
        Ok(if path.is_absolute() {
          path.to_path_buf()
        } else {
          self.root.join(path)
        })
      }
      _ => Err(LocalGitError::Refused),
    }
  }
}
pub(super) fn git_name(path: &Path) -> Result<String, LocalGitError> {
  let name = path.to_str().ok_or(LocalGitError::Data)?;
  if let Some(unc) = name.strip_prefix(r"\\?\UNC\") {
    return Ok(format!(r"\\{unc}"));
  }
  Ok(name.strip_prefix(r"\\?\").unwrap_or(name).to_owned())
}
