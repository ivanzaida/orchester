//! The read-only `gh` commands behind a pull request's CI: `gh pr checks <n> -R <host/owner/name> --json …` for
//! every check run and commit status of its head, and `gh run view --job <id> -R <host/owner/name> --log-failed`
//! for the log of a failed GitHub Actions job's failed steps.

use std::path::Path;

use serde::Deserialize;

use super::{CiCheck, CiState};
use crate::{
  command::{Command, CommandOutput},
  connection::gh_command,
  repository::{GitHubRepo, RepositoryError, classify_failure, unrecognized},
};

/// The fields `gh pr checks --json` is asked for.
const CHECK_FIELDS: &str = "name,state,bucket,link,workflow";
/// How many lines of a failed job's log a read keeps: its end, where the failure is.
pub const LOG_TAIL_LINES: usize = 40;
/// Most characters of a kept log line; a longer one (minified output, a dump) is cut.
const LOG_LINE_CHARS: usize = 400;

/// Reads every check of pull request `number`'s head.
pub fn gh_pr_checks_command(executable: &Path, repo: &GitHubRepo, number: u64) -> Command {
  let args = [
    "pr".to_owned(),
    "checks".to_owned(),
    number.to_string(),
    "-R".to_owned(),
    repo.to_string(),
    "--json".to_owned(),
    CHECK_FIELDS.to_owned(),
  ];
  gh_command(executable, args.to_vec())
}

/// Reads the log of the failed steps of GitHub Actions job `job`.
pub fn gh_job_log_command(executable: &Path, repo: &GitHubRepo, job: u64) -> Command {
  let args = [
    "run".to_owned(),
    "view".to_owned(),
    "--job".to_owned(),
    job.to_string(),
    "-R".to_owned(),
    repo.to_string(),
    "--log-failed".to_owned(),
  ];
  gh_command(executable, args.to_vec())
}

#[derive(Deserialize)]
struct Listed {
  name: String,
  state: String,
  bucket: String,
  #[serde(default)]
  link: String,
  #[serde(default)]
  workflow: String,
}

/// Reads `gh pr checks` output for pull request `number` of `repo`. `gh` ends with an error when the head has no
/// checks, which is no checks, not a failure; it exits with 8 while checks are pending and 1 when one failed, after
/// writing them, so what it wrote is read whatever its exit code.
pub fn parse_gh_pr_checks(
  repo: &GitHubRepo,
  number: u64,
  output: &CommandOutput,
) -> Result<Vec<CiCheck>, RepositoryError> {
  let stdout = output.stdout.trim();
  if !output.success() && !stdout.starts_with('[') {
    let message = output.stderr.to_ascii_lowercase();
    if message.contains("no checks reported") {
      return Ok(Vec::new());
    }
    if message.contains("could not resolve to a pullrequest") || message.contains("no pull requests found") {
      return Err(RepositoryError::PullRequestNotFound {
        repo: repo.clone(),
        number,
      });
    }
    return Err(classify_failure(repo, &output.stderr));
  }
  let listed: Vec<Listed> = serde_json::from_str(stdout).map_err(|error| unrecognized(&error.to_string()))?;
  listed.into_iter().map(check).collect()
}

fn check(listed: Listed) -> Result<CiCheck, RepositoryError> {
  let state = state(&listed.state, &listed.bucket)
    .ok_or_else(|| unrecognized(&format!("check {} {}", listed.state, listed.bucket)))?;
  let url = Some(listed.link).filter(|link| !link.is_empty());
  Ok(CiCheck {
    job: url.as_deref().and_then(actions_job),
    name: listed.name,
    workflow: Some(listed.workflow).filter(|workflow| !workflow.is_empty()),
    state,
    url,
    log_tail: None,
  })
}

/// GitHub's check run conclusions and statuses and commit status states. Neutral passes and a stale run was
/// superseded, as the pull request list counts them; a state `gh` knows and Orchester does not goes by `gh`'s
/// bucket.
fn state(state: &str, bucket: &str) -> Option<CiState> {
  Some(match state {
    "SUCCESS" | "NEUTRAL" => CiState::Passed,
    "SKIPPED" | "STALE" => CiState::Skipped,
    "FAILURE" | "ERROR" | "TIMED_OUT" | "ACTION_REQUIRED" | "STARTUP_FAILURE" => CiState::Failed,
    "CANCELLED" => CiState::Cancelled,
    "IN_PROGRESS" => CiState::Running,
    "QUEUED" | "PENDING" | "WAITING" | "REQUESTED" | "EXPECTED" => CiState::Queued,
    _ => match bucket {
      "pass" => CiState::Passed,
      "fail" => CiState::Failed,
      "cancel" => CiState::Cancelled,
      "skipping" => CiState::Skipped,
      "pending" => CiState::Queued,
      _ => return None,
    },
  })
}

/// The job of a GitHub Actions address, `https://<host>/<owner>/<name>/actions/runs/<run>/job/<job>`; `None` for
/// any other (a commit status's or an app's own page).
fn actions_job(url: &str) -> Option<u64> {
  let path = url.split(['?', '#']).next()?;
  let segments: Vec<&str> = path.trim_end_matches('/').rsplit('/').take(4).collect();
  match segments.as_slice() {
    [job, "job", run, "runs"] if run.parse::<u64>().is_ok() => job.parse().ok(),
    _ => None,
  }
}

/// The last [`LOG_TAIL_LINES`] lines of a `gh run view --log-failed` answer, each without the job and step names
/// and the time `gh` puts before it; `None` when it gave no log (GitHub keeps logs for a while only).
pub fn parse_gh_job_log(repo: &GitHubRepo, output: &CommandOutput) -> Result<Option<String>, RepositoryError> {
  if !output.success() {
    return Err(classify_failure(repo, &output.stderr));
  }
  let lines: Vec<&str> = output.stdout.lines().filter(|line| !line.trim().is_empty()).collect();
  if lines.is_empty() {
    return Ok(None);
  }
  let tail: Vec<String> = lines[lines.len().saturating_sub(LOG_TAIL_LINES)..]
    .iter()
    .map(|line| log_text(line).chars().take(LOG_LINE_CHARS).collect())
    .collect();
  Ok(Some(tail.join("\n")))
}

/// A log line's text: `gh` writes `<job>\t<step>\t<time> <text>`.
fn log_text(line: &str) -> &str {
  let text = line.splitn(3, '\t').nth(2).unwrap_or(line);
  match text.split_once(' ') {
    Some((time, rest)) if is_log_time(time) => rest,
    _ => text,
  }
}

/// GitHub's log time, as `2026-11-03T10:15:02.1234567Z`.
fn is_log_time(word: &str) -> bool {
  word.len() >= 20
    && word.ends_with('Z')
    && word.as_bytes().get(10) == Some(&b'T')
    && word.get(..4).is_some_and(|year| year.parse::<u16>().is_ok())
}
