use orchester_core::run::BranchName;

use super::{LocalGit, LocalGitError, validate_object};

impl LocalGit {
  pub fn branch_tip(&self, branch: &BranchName) -> Result<Option<String>, LocalGitError> {
    let reference = format!("refs/heads/{}", branch.as_str());
    let output = self.raw(
      &["rev-parse", "--verify", "--quiet", "--end-of-options", &reference],
      None,
      None,
    )?;
    match output.code {
      Some(0) => {
        let value = String::from_utf8(output.bytes).map_err(|_| LocalGitError::Data)?;
        let value = value.trim();
        validate_object(value)?;
        Ok(Some(value.to_owned()))
      }
      Some(1) => Ok(None),
      _ => Err(LocalGitError::Refused),
    }
  }

  pub fn is_ancestor(&self, source: &str, target: &str) -> Result<bool, LocalGitError> {
    validate_object(source)?;
    validate_object(target)?;
    let output = self.raw(&["merge-base", "--is-ancestor", source, target], None, None)?;
    match output.code {
      Some(0) => Ok(true),
      Some(1) => Ok(false),
      _ => Err(LocalGitError::Refused),
    }
  }

  /// The commits on `source`'s own line above `base`, oldest first: those made on its branch, the merges of the
  /// target it took in included (ORC-54), not the target's commits those merges brought, which are the target's
  /// work and no evidence about the run's. `base` need not be on that line: when `source` merged `base` in, the
  /// commits of its own from before that merge count too (ORC-67).
  pub fn commits_after(&self, base: &str, source: &str) -> Result<Vec<String>, LocalGitError> {
    validate_object(base)?;
    validate_object(source)?;
    let excluded = format!("^{base}");
    let text = self.text(&["rev-list", "--first-parent", "--reverse", source, &excluded])?;
    let commits = text.lines().map(str::to_owned).collect::<Vec<_>>();
    if commits.len() > 1024 {
      return Err(super::LocalError::Unsupported.into());
    }
    for commit in &commits {
      validate_object(commit)?;
    }
    Ok(commits)
  }

  /// Whether a commit `source` holds above `base` that `target` does not hold makes the change of a commit `target`
  /// holds, a cherry-pick or a rebase of it (`git cherry`, by patch id; merges have none and are left out). An
  /// empty commit, as an agent's `checkpoint before merging` is when it has nothing to commit (ORC-54), has the
  /// patch id of every other empty commit and proves nothing.
  pub fn has_equivalent_patch(&self, base: &str, source: &str, target: &str) -> Result<bool, LocalGitError> {
    for object in [base, source, target] {
      validate_object(object)?;
    }
    let text = self.text(&["cherry", target, source, base])?;
    for commit in text.lines().filter_map(|line| line.strip_prefix("- ")) {
      validate_object(commit)?;
      if self.changes_something(commit)? {
        return Ok(true);
      }
    }
    Ok(false)
  }

  /// Whether `commit` changes anything against its parent.
  fn changes_something(&self, commit: &str) -> Result<bool, LocalGitError> {
    let changed = self.text(&["diff-tree", "--no-commit-id", "--name-only", "-r", "--root", commit])?;
    Ok(!changed.is_empty())
  }

  /// The exact patch ids of the commits reachable from `to` and not from `from`, merges left out,
  /// sorted: `git patch-id --verbatim`, which counts every whitespace change, over their patches.
  pub fn exact_patch_ids(&self, from: &str, to: &str) -> Result<Vec<String>, LocalGitError> {
    validate_object(from)?;
    validate_object(to)?;
    let excluded = format!("^{from}");
    let patches = self.raw(
      &[
        "log",
        "--no-merges",
        "--patch",
        "--no-color",
        "--no-ext-diff",
        "--full-index",
        to,
        &excluded,
        "--",
      ],
      None,
      None,
    )?;
    if patches.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    let ids = self.raw(&["patch-id", "--verbatim"], Some(&patches.bytes), None)?;
    if ids.code != Some(0) {
      return Err(LocalGitError::Refused);
    }
    let text = String::from_utf8(ids.bytes).map_err(|_| LocalGitError::Data)?;
    let mut ids: Vec<String> = text
      .lines()
      .filter_map(|line| line.split(' ').next())
      .map(str::to_owned)
      .collect();
    ids.sort();
    Ok(ids)
  }
}
