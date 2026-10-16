//! The read-only `gh` commands behind a pull request's detail: `gh pr view <n> -R <host/owner/
//! name> --json …` for its current head and base revisions, and `gh api --hostname <host> -H
//! "Accept: application/vnd.github.diff" repos/<owner>/<name>/compare/<base>...<head>` for the
//! diff between exactly those revisions. GitHub's pull request diff is the same three-dot compare,
//! but `gh pr diff` always reads the current head, which may move between the two calls.

use std::path::Path;

use serde::Deserialize;

use super::PullRequestHead;
use crate::{
  command::{Command, CommandOutput},
  connection::gh_command,
  pull_request::gh::{Fields, Listed, pull_request},
  repository::{GitHubRepo, RepositoryError, classify_failure, unrecognized},
  revision::Revision,
};

/// The media type that makes GitHub answer a compare with a unified diff.
const DIFF_MEDIA_TYPE: &str = "Accept: application/vnd.github.diff";

/// Reads pull request `number` of `repo` with `fields` plus its checks (one pull request's are few
/// enough) and its base revision.
pub fn gh_pr_view_command(executable: &Path, repo: &GitHubRepo, number: u64, fields: Fields) -> Command {
  let args = [
    "pr".to_owned(),
    "view".to_owned(),
    number.to_string(),
    "-R".to_owned(),
    repo.to_string(),
    "--json".to_owned(),
    format!("{},statusCheckRollup,baseRefOid", fields.as_str()),
  ];
  gh_command(executable, args.to_vec())
}

/// The diff of `repo` from the merge base of `base` and `head` to `head`. Escape sequences are
/// allowed: `gh` refuses to print a diff containing them otherwise, and Orchester shows the
/// text in its own view, never in a terminal.
pub fn gh_compare_diff_command(executable: &Path, repo: &GitHubRepo, base: &Revision, head: &Revision) -> Command {
  let args = [
    "api".to_owned(),
    "--allow-escape-sequences".to_owned(),
    "--hostname".to_owned(),
    repo.host().as_str().to_owned(),
    "-H".to_owned(),
    DIFF_MEDIA_TYPE.to_owned(),
    format!("repos/{}/{}/compare/{base}...{head}", repo.owner(), repo.name()),
  ];
  gh_command(executable, args.to_vec())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Viewed {
  #[serde(flatten)]
  listed: Listed,
  base_ref_oid: String,
}

/// Reads `gh pr view` output for pull request `number` of `repo`.
pub fn parse_gh_pr_view(
  repo: &GitHubRepo,
  number: u64,
  output: &CommandOutput,
) -> Result<PullRequestHead, RepositoryError> {
  if !output.success() {
    let not_found = output
      .stderr
      .to_ascii_lowercase()
      .contains("could not resolve to a pullrequest");
    if not_found {
      return Err(RepositoryError::PullRequestNotFound {
        repo: repo.clone(),
        number,
      });
    }
    return Err(classify_failure(repo, &output.stderr));
  }
  let viewed: Viewed = serde_json::from_str(output.stdout.trim()).map_err(|error| unrecognized(&error.to_string()))?;
  if viewed.listed.number != number {
    return Err(unrecognized(&format!("pull request #{}", viewed.listed.number)));
  }
  let base = Revision::new(&viewed.base_ref_oid).map_err(|error| unrecognized(&error.to_string()))?;
  Ok(PullRequestHead {
    pull_request: pull_request(repo, viewed.listed)?,
    base,
  })
}

/// Reads the diff a compare answered. GitHub answers 404 when either revision is gone.
pub fn parse_gh_compare_diff(repo: &GitHubRepo, output: &CommandOutput) -> Result<String, RepositoryError> {
  if output.success() {
    return Ok(output.stdout.clone());
  }
  if output.stderr.contains("(HTTP 404)") {
    return Err(RepositoryError::DiffNotFound(repo.clone()));
  }
  Err(classify_failure(repo, &output.stderr))
}
