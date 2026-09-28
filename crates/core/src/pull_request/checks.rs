//! The checks of the listed pull requests' heads, counted by GitHub, in one read-only `gh api
//! graphql` request. `gh pr list --json statusCheckRollup` returns every check of every pull
//! request, which on repositories with many checks (microsoft/vscode, BurntSushi/ripgrep) makes
//! GitHub time out (HTTP 504); the counts by state of the exact head commits answer in about a
//! second.

use std::{collections::HashMap, path::Path};

use serde::Deserialize;

use super::CheckRollup;
use crate::{
  command::{Command, CommandOutput},
  connection::gh_command,
  repository::{GitHubRepo, RepositoryError, classify_failure, unrecognized},
  revision::Revision,
};

/// Asks for the check counts of each of `heads`, commits of `repo`, one aliased field each. The
/// query and its variables are the request body on standard input (`--input -`), never an
/// argument: on Windows a `gh` started through a `.cmd` script would have `cmd.exe` split the
/// query text into many arguments.
pub fn gh_check_rollups_command(executable: &Path, repo: &GitHubRepo, heads: &[Revision]) -> Command {
  // Revisions are hexadecimal object ids, so they are safe inside the query text; owner and
  // name travel as variables.
  let commits: String = heads
    .iter()
    .enumerate()
    .map(|(index, head)| {
      format!(
        "c{index}:object(oid:\"{head}\"){{...on Commit{{oid statusCheckRollup{{contexts{{         checkRunCountsByState{{state count}} statusContextCountsByState{{state count}}}}}}}}}}"
      )
    })
    .collect();
  let query = format!("query($owner:String!,$name:String!){{repository(owner:$owner,name:$name){{{commits}}}}}");
  let body = serde_json::json!({
    "query": query,
    "variables": { "owner": repo.owner(), "name": repo.name() },
  });
  let args = ["api", "graphql", "--hostname", repo.host().as_str(), "--input", "-"];
  gh_command(executable, args.map(str::to_owned).to_vec()).with_stdin(body.to_string())
}

#[derive(Deserialize)]
struct Answer {
  data: Data,
}

#[derive(Deserialize)]
struct Data {
  repository: HashMap<String, Option<Commit>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Commit {
  oid: String,
  status_check_rollup: Option<Rollup>,
}

#[derive(Deserialize)]
struct Rollup {
  contexts: Contexts,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Contexts {
  check_run_counts_by_state: Vec<StateCount>,
  status_context_counts_by_state: Vec<StateCount>,
}

#[derive(Deserialize)]
struct StateCount {
  state: String,
  count: u32,
}

/// Reads the answer to [`gh_check_rollups_command`] for `heads`, in their order. A commit
/// GitHub does not know, or one without checks, has none.
pub fn parse_gh_check_rollups(
  repo: &GitHubRepo,
  heads: &[Revision],
  output: &CommandOutput,
) -> Result<Vec<CheckRollup>, RepositoryError> {
  if !output.success() {
    return Err(classify_failure(repo, &output.stderr));
  }
  let mut answer: Answer =
    serde_json::from_str(output.stdout.trim()).map_err(|error| unrecognized(&error.to_string()))?;
  heads
    .iter()
    .enumerate()
    .map(|(index, head)| {
      let commit = answer
        .data
        .repository
        .remove(&format!("c{index}"))
        .ok_or_else(|| unrecognized(&format!("no answer for commit {head}")))?;
      match commit {
        None => Ok(CheckRollup::none(head.clone())),
        Some(commit) if commit.oid != head.as_str() => Err(unrecognized(&format!("commit {}", commit.oid))),
        Some(commit) => rollup(head, commit.status_check_rollup),
      }
    })
    .collect()
}

fn rollup(head: &Revision, rollup: Option<Rollup>) -> Result<CheckRollup, RepositoryError> {
  let mut checks = CheckRollup::none(head.clone());
  let Some(Rollup { contexts }) = rollup else {
    return Ok(checks);
  };
  let runs = contexts
    .check_run_counts_by_state
    .iter()
    .map(|count| (count, check_run(&count.state)));
  let statuses = contexts
    .status_context_counts_by_state
    .iter()
    .map(|count| (count, commit_status(&count.state)));
  for (count, outcome) in runs.chain(statuses) {
    if count.count == 0 {
      continue;
    }
    let total = match outcome {
      Some(Outcome::Passed) => &mut checks.passed,
      Some(Outcome::Failed) => &mut checks.failed,
      Some(Outcome::Pending) => &mut checks.pending,
      Some(Outcome::Skipped) => &mut checks.skipped,
      None => return Err(unrecognized(&format!("check state {}", count.state))),
    };
    *total += count.count;
  }
  Ok(checks)
}

#[derive(Clone, Copy)]
enum Outcome {
  Passed,
  Failed,
  Pending,
  Skipped,
}

/// A check run's state as GitHub counts it: its conclusion once completed, else its status.
/// Neutral passes, as for GitHub's merge rules; a stale run was superseded, and one completed
/// without a conclusion has no outcome, so both count as skipped.
fn check_run(state: &str) -> Option<Outcome> {
  match state {
    "SUCCESS" | "NEUTRAL" => Some(Outcome::Passed),
    "SKIPPED" | "STALE" | "COMPLETED" => Some(Outcome::Skipped),
    "FAILURE" | "CANCELLED" | "TIMED_OUT" | "ACTION_REQUIRED" | "STARTUP_FAILURE" => Some(Outcome::Failed),
    "IN_PROGRESS" | "PENDING" | "QUEUED" | "WAITING" | "REQUESTED" => Some(Outcome::Pending),
    _ => None,
  }
}

fn commit_status(state: &str) -> Option<Outcome> {
  match state {
    "SUCCESS" => Some(Outcome::Passed),
    "PENDING" | "EXPECTED" => Some(Outcome::Pending),
    "FAILURE" | "ERROR" => Some(Outcome::Failed),
    _ => None,
  }
}
