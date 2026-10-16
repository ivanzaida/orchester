//! The read-only `gh` commands behind the pull request list: `gh pr list -R <host/owner/name>
//! --state <open|closed> --limit <n> --json …` and, after a rate limit, `gh api --hostname <host>
//! rate_limit` (reading the limit does not count against it).

use std::path::Path;

use serde::Deserialize;

use super::{CheckRollup, PullRequest, PullRequestParts, PullRequestState, ReviewDecision};
use crate::{
  command::{Command, CommandOutput},
  connection::{GitHubHost, gh_command},
  diff::Changes,
  repository::{GitHubRepo, RepositoryError, classify_failure, unrecognized},
  revision::Revision,
  run::BranchName,
  time::Timestamp,
};

/// The fields `gh pr list --json` is asked for. Not `statusCheckRollup`: every check of every
/// pull request makes GitHub time out on large repositories; the checks are counted in a second
/// request (`checks`).
pub const PR_LIST_FIELDS: &str = "number,title,url,state,isDraft,headRefName,baseRefName,headRefOid,author,\
                                  additions,deletions,changedFiles,updatedAt,reviewDecision";
/// [`PR_LIST_FIELDS`] with when a pull request was merged or closed and whether a fork opened it.
pub const PR_PAGE_FIELDS: &str = "number,title,url,state,isDraft,headRefName,baseRefName,headRefOid,author,\
                                  additions,deletions,changedFiles,updatedAt,reviewDecision,mergedAt,closedAt,\
                                  isCrossRepository";

/// Which fields a list or a view of pull requests asks `gh` for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fields {
  /// [`PR_LIST_FIELDS`], what the deprecated desktop's inbox has always asked for.
  Inbox,
  /// [`PR_PAGE_FIELDS`], for the Pull requests page.
  Page,
}

impl Fields {
  pub const fn as_str(self) -> &'static str {
    match self {
      Self::Inbox => PR_LIST_FIELDS,
      Self::Page => PR_PAGE_FIELDS,
    }
  }
}

/// Which pull requests one `gh pr list` asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListedStates {
  /// Open ones, drafts included.
  Open,
  /// Closed and merged ones: `gh` counts a merged pull request as closed.
  Closed,
}

impl ListedStates {
  const fn as_str(self) -> &'static str {
    match self {
      Self::Open => "open",
      Self::Closed => "closed",
    }
  }
}

/// Lists at most `limit` pull requests of `repo` in `states`, with `fields`.
pub fn gh_pr_list_command(
  executable: &Path,
  repo: &GitHubRepo,
  (states, fields): (ListedStates, Fields),
  limit: u32,
) -> Command {
  let args = [
    "pr".to_owned(),
    "list".to_owned(),
    "-R".to_owned(),
    repo.to_string(),
    "--state".to_owned(),
    states.as_str().to_owned(),
    "--limit".to_owned(),
    limit.to_string(),
    "--json".to_owned(),
    fields.as_str().to_owned(),
  ];
  gh_command(executable, args.to_vec())
}

pub fn gh_rate_limit_command(executable: &Path, host: &GitHubHost) -> Command {
  let args = ["api", "--hostname", host.as_str(), "rate_limit"];
  gh_command(executable, args.map(str::to_owned).to_vec())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Listed {
  pub(super) number: u64,
  title: String,
  url: String,
  state: String,
  is_draft: bool,
  head_ref_name: String,
  base_ref_name: String,
  head_ref_oid: String,
  author: Option<Author>,
  additions: u32,
  deletions: u32,
  changed_files: u32,
  updated_at: String,
  review_decision: Option<String>,
  /// Empty or absent until it was merged or closed.
  #[serde(default)]
  merged_at: Option<String>,
  #[serde(default)]
  closed_at: Option<String>,
  #[serde(default)]
  is_cross_repository: bool,
  status_check_rollup: Option<Vec<Check>>,
}

#[derive(Deserialize)]
struct Author {
  login: String,
}

/// An entry of `statusCheckRollup`: a check run (GitHub Actions, apps) or a commit status.
#[derive(Deserialize)]
#[serde(tag = "__typename")]
enum Check {
  CheckRun { status: String, conclusion: Option<String> },
  StatusContext { state: String },
}

/// Reads `gh pr list` output for `repo`. A failed run is classified from `gh`'s error message.
/// Without `statusCheckRollup` in the output a pull request has no checks until
/// [`PullRequest::with_checks`] gives it the counted ones.
pub fn parse_gh_pr_list(repo: &GitHubRepo, output: &CommandOutput) -> Result<Vec<PullRequest>, RepositoryError> {
  if !output.success() {
    return Err(classify_failure(repo, &output.stderr));
  }
  let listed: Vec<Listed> =
    serde_json::from_str(output.stdout.trim()).map_err(|error| unrecognized(&error.to_string()))?;
  listed.into_iter().map(|listed| pull_request(repo, listed)).collect()
}

pub(super) fn pull_request(repo: &GitHubRepo, listed: Listed) -> Result<PullRequest, RepositoryError> {
  let state = match (listed.state.as_str(), listed.is_draft) {
    ("OPEN", true) => PullRequestState::Draft,
    ("OPEN", false) => PullRequestState::Open,
    ("MERGED", _) => PullRequestState::Merged,
    ("CLOSED", _) => PullRequestState::Closed,
    (other, _) => return Err(unrecognized(other)),
  };
  let branch = |name: &str| BranchName::new(name).map_err(|error| unrecognized(&error.to_string()));
  let head_sha = Revision::new(&listed.head_ref_oid).map_err(|error| unrecognized(&error.to_string()))?;
  let review_decision = match listed.review_decision.as_deref() {
    None | Some("") => None,
    Some(value) => Some(ReviewDecision::parse(value).ok_or_else(|| unrecognized(value))?),
  };
  let parts = PullRequestParts {
    number: listed.number,
    title: listed.title,
    url: listed.url,
    state,
    head: branch(&listed.head_ref_name)?,
    base: branch(&listed.base_ref_name)?,
    checks: rollup(listed.status_check_rollup.unwrap_or_default(), head_sha.clone())?,
    head_sha,
    author: listed
      .author
      .map(|author| author.login)
      .filter(|login| !login.is_empty()),
    changes: Changes {
      files: listed.changed_files,
      additions: listed.additions,
      deletions: listed.deletions,
    },
    review_decision,
    updated_at: Timestamp::parse_utc(&listed.updated_at).ok_or_else(|| unrecognized(&listed.updated_at))?,
    merged_at: time(listed.merged_at.as_deref())?,
    closed_at: time(listed.closed_at.as_deref())?,
    cross_repository: listed.is_cross_repository,
  };
  PullRequest::new(repo, parts).map_err(|error| unrecognized(&error.to_string()))
}

/// A time `gh` gave, `None` when it gave none: GitHub writes an empty or zero time for a pull request not
/// merged or closed yet.
fn time(value: Option<&str>) -> Result<Option<Timestamp>, RepositoryError> {
  match value {
    None | Some("" | "0001-01-01T00:00:00Z") => Ok(None),
    Some(value) => Timestamp::parse_utc(value).map(Some).ok_or_else(|| unrecognized(value)),
  }
}

#[derive(Clone, Copy)]
enum Outcome {
  Passed,
  Failed,
  Pending,
  Skipped,
}

fn rollup(checks: Vec<Check>, on: Revision) -> Result<CheckRollup, RepositoryError> {
  let mut rollup = CheckRollup::none(on);
  for check in checks {
    let count = match outcome(&check)? {
      Outcome::Passed => &mut rollup.passed,
      Outcome::Failed => &mut rollup.failed,
      Outcome::Pending => &mut rollup.pending,
      Outcome::Skipped => &mut rollup.skipped,
    };
    *count += 1;
  }
  Ok(rollup)
}

/// GitHub's check run conclusions and commit status states. Neutral passes, as it does for
/// GitHub's merge rules; a stale run was superseded and counts as skipped.
fn outcome(check: &Check) -> Result<Outcome, RepositoryError> {
  let outcome = match check {
    Check::CheckRun { status, .. } if status != "COMPLETED" => Some(Outcome::Pending),
    Check::CheckRun { conclusion, .. } => match conclusion.as_deref().unwrap_or_default() {
      "SUCCESS" | "NEUTRAL" => Some(Outcome::Passed),
      "SKIPPED" | "STALE" => Some(Outcome::Skipped),
      "FAILURE" | "CANCELLED" | "TIMED_OUT" | "ACTION_REQUIRED" | "STARTUP_FAILURE" => Some(Outcome::Failed),
      _ => None,
    },
    Check::StatusContext { state } => match state.as_str() {
      "SUCCESS" => Some(Outcome::Passed),
      "PENDING" | "EXPECTED" => Some(Outcome::Pending),
      "FAILURE" | "ERROR" => Some(Outcome::Failed),
      _ => None,
    },
  };
  outcome.ok_or_else(|| {
    let detail = match check {
      Check::CheckRun { status, conclusion } => format!("check run {status} {conclusion:?}"),
      Check::StatusContext { state } => format!("commit status {state}"),
    };
    unrecognized(&detail)
  })
}

/// When the GraphQL rate limit (the one `gh pr list` uses) resets, from `gh api rate_limit`.
/// A failed run is classified as for `repo`, the repository whose list hit the limit.
pub fn parse_gh_rate_limit(repo: &GitHubRepo, output: &CommandOutput) -> Result<Timestamp, RepositoryError> {
  #[derive(Deserialize)]
  struct Limits {
    resources: Resources,
  }
  #[derive(Deserialize)]
  struct Resources {
    graphql: Limit,
  }
  #[derive(Deserialize)]
  struct Limit {
    /// Unix seconds.
    reset: i64,
  }
  if !output.success() {
    return Err(classify_failure(repo, &output.stderr));
  }
  let limits: Limits = serde_json::from_str(output.stdout.trim()).map_err(|error| unrecognized(&error.to_string()))?;
  Ok(Timestamp::from_unix_seconds(limits.resources.graphql.reset))
}
