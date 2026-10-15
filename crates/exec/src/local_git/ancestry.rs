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

  pub fn commits_after(&self, base: &str, source: &str) -> Result<Vec<String>, LocalGitError> {
    validate_object(base)?;
    validate_object(source)?;
    let excluded = format!("^{base}");
    let text = self.text(&["rev-list", "--ancestry-path", "--reverse", source, &excluded])?;
    let commits = text.lines().map(str::to_owned).collect::<Vec<_>>();
    if commits.len() > 1024 {
      return Err(super::LocalError::Unsupported.into());
    }
    for commit in &commits {
      validate_object(commit)?;
    }
    Ok(commits)
  }

  pub fn has_equivalent_patch(&self, base: &str, source: &str, target: &str) -> Result<bool, LocalGitError> {
    for object in [base, source, target] {
      validate_object(object)?;
    }
    let text = self.text(&["cherry", target, source, base])?;
    Ok(text.lines().any(|line| line.starts_with("- ")))
  }
}
