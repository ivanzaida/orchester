//! How `get_run` writes a pull request's CI as `checks`
//! ([`crate::pull_requests::ci`]): its state, the checks one by one and, for a failed GitHub Actions job, the tail of
//! its log; or why they were not read, by name. A state that was not read is never `passed`.

use orchester_core::{
  connection::GitHubState,
  id::ProjectId,
  pull_request::ci::{CiCheck, CiState},
  repository::RepositoryError,
  run::Run,
  store::RepositoryStore,
  time::Timestamp,
};
use orchester_store::ProjectDb;
use serde_json::{Value, json};

use super::wire;
use crate::{
  app_mcp::{error::ToolError, services::Services},
  messages::Message,
  pull_requests::ci::{PullRequestCi, run_pull_request},
};

/// The CI of pull request `number` of `project` at `root`, read through the server's
/// [`crate::pull_requests::ci::CiReads`].
fn of_pull_request(
  services: &Services,
  (project, root): (ProjectId, &std::path::Path),
  number: u64,
) -> Result<Value, ToolError> {
  let read = services
    .ci_reads
    .read((project, root), &services.programs, number, Timestamp::now())
    .map_err(|error| refused(error.message()))?;
  Ok(checks(&read))
}

/// The CI of the pull request of `run`'s branch, as the last sync of `db`'s project stored it.
pub(super) fn of_run(
  services: &Services,
  (project, root): (ProjectId, &std::path::Path),
  db: &ProjectDb,
  run: &Run,
) -> Result<Value, ToolError> {
  if db.repository_link()?.is_none() {
    return Ok(checks(&PullRequestCi::NotLinked));
  }
  match run_pull_request(db, run).map_err(|error| refused(error.message()))? {
    Some(number) => of_pull_request(services, (project, root), number),
    None => Ok(checks(&PullRequestCi::NoPullRequest)),
  }
}

fn refused(message: Message) -> ToolError {
  ToolError::refused("project_data", vec![message])
}

/// `ci` as the tools write it.
pub(super) fn checks(ci: &PullRequestCi) -> Value {
  let mut written = json!({
    "state": state(ci),
    "pull_request": null,
    "read_at": null,
    "next_read_at": ci.next_read_at().map(wire::time),
    "counts": null,
    "runs": [],
  });
  match ci {
    PullRequestCi::Read(read) => {
      let count = |state| read.checks.count(state);
      written["pull_request"] = json!(read.number);
      written["read_at"] = json!(wire::time(read.read_at));
      written["counts"] = json!({
        "queued": count(CiState::Queued),
        "running": count(CiState::Running),
        "passed": count(CiState::Passed),
        "failed": count(CiState::Failed),
        "cancelled": count(CiState::Cancelled),
        "skipped": count(CiState::Skipped),
      });
      written["runs"] = read.checks.checks.iter().map(run).collect();
    }
    PullRequestCi::RateLimited { resets_at } => written["resets_at"] = json!(resets_at.map(wire::time)),
    _ => {}
  }
  written
}

fn run(check: &CiCheck) -> Value {
  json!({
    "name": check.name,
    "workflow": check.workflow,
    "state": check.state.as_str(),
    "url": check.url,
    "log_tail": check.log_tail,
  })
}

/// How the checks stand, or why they were not read.
fn state(ci: &PullRequestCi) -> &'static str {
  match ci {
    PullRequestCi::NotLinked => "not_linked",
    PullRequestCi::NoPullRequest => "no_pull_request",
    PullRequestCi::GhMissing => "gh_missing",
    PullRequestCi::NotSignedIn => "not_signed_in",
    PullRequestCi::Unavailable(GitHubState::WrongAccount { .. }) => "wrong_account",
    PullRequestCi::Unavailable(GitHubState::MissingScope { .. }) => "missing_scope",
    PullRequestCi::Unavailable(_) => "gh_unavailable",
    PullRequestCi::RateLimited { .. } => "rate_limited",
    PullRequestCi::NotRead(RepositoryError::Network { .. }) => "github_unreachable",
    PullRequestCi::NotRead(RepositoryError::ServerError { .. }) => "github_server_error",
    PullRequestCi::NotRead(_) => "not_read",
    PullRequestCi::Read(read) => read.checks.summary().as_str(),
  }
}
