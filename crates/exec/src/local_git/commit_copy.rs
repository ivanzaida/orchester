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
    // A null device cannot be replaced with a hook directory. Actual commit hooks use hook_directory().
    let setting = if cfg!(windows) {
      "core.hooksPath=NUL"
    } else {
      "core.hooksPath=/dev/null"
    };
    let mut configured = vec!["-c", setting];
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

  /// A copy holding only `paths` of `index`, for reads that need a few of its files, such as its ignore rules.
  pub(super) fn partial_copy(
    &self,
    index: &Path,
    paths: &[super::LocalPath],
  ) -> Result<tempfile::TempDir, LocalGitError> {
    let directory = tempfile::Builder::new()
      .prefix("commit-")
      .tempdir_in(&self.scratch)
      .map_err(|_| LocalGitError::Io)?;
    if paths.is_empty() {
      return Ok(directory);
    }
    let mut input = Vec::new();
    for path in paths {
      input.extend_from_slice(path.as_str().as_bytes());
      input.push(0);
    }
    let copied = self.scoped_raw(
      &["checkout-index", "--force", "-z", "--stdin"],
      Some(&input),
      Some(index),
      Some(directory.path()),
    )?;
    if copied.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    Ok(directory)
  }

  pub(super) fn in_commit_copy(&self, command: &Command, worktree: &Path) -> Result<Command, LocalGitError> {
    let worktree = worktree.canonicalize().map_err(|_| LocalGitError::Io)?;
    if !worktree.starts_with(&self.scratch) {
      return Err(LocalError::Unsupported.into());
    }
    let git_dir = &self.git_dir_text;
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
        .with_env("GIT_DIR", &git_name(Path::new(git_dir))?)
        .with_env("GIT_WORK_TREE", &git_name(&worktree)?)
        .with_env("GIT_INDEX_FILE", index)
        .without_env("GIT_COMMON_DIR")
        .without_env("GIT_CONFIG_COUNT")
        .without_env("GIT_CONFIG_PARAMETERS")
        .without_env("GIT_OBJECT_DIRECTORY")
        .without_env("GIT_ALTERNATE_OBJECT_DIRECTORIES"),
    )
  }

  /// The repository's hook directory, resolved once.
  fn hook_directory(&self) -> Result<PathBuf, LocalGitError> {
    if let Some(hooks) = self.hooks.get() {
      return Ok(hooks.clone());
    }
    let hooks = self.find_hook_directory()?;
    Ok(self.hooks.get_or_init(|| hooks).clone())
  }

  fn find_hook_directory(&self) -> Result<PathBuf, LocalGitError> {
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
