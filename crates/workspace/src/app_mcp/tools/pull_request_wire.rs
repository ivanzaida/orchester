//! How `list_pull_requests` and `get_pull_request` write a pull request, its task and its repository's sync.

use std::num::NonZeroU64;

use orchester_core::{
  id::ProjectId,
  pull_request::{PullRequest, ReviewDecision},
};
use serde_json::{Value, json};

use super::wire;
use crate::{
  links::{Link, TaskPage},
  pull_requests::{
    linked::LinkedTask,
    list::{ListedPullRequest, PullRequestList, PullRequestTab},
    status::SyncStatus,
  },
  workspace::OpenProject,
};

/// The repository a project's pull requests come from and how its latest sync went: `sync` names it, `synced_at`
/// is when the list shown was read, `attempted_at` when the latest sync ran, and `paused_until` when a rate limit
/// lets the next one ask GitHub.
pub(super) fn repository(project: &OpenProject, list: &PullRequestList) -> Value {
  let named = json!({
    "id": project.project.id().to_string(),
    "name": project.project.name().as_str(),
  });
  let PullRequestList::Linked(linked) = list else {
    return json!({ "project": named, "repository": null, "link": null });
  };
  json!({
    "project": named,
    "repository": linked.repo.to_string(),
    "expected_account": linked.expected.as_str(),
    "sync": sync(&linked.status()),
    "synced_at": linked.synced_at.map(wire::time),
    "attempted_at": linked.attempted_at.map(wire::time),
    "paused_until": linked
      .failure
      .as_ref()
      .and_then(orchester_core::pull_request::SyncFailure::paused_until)
      .map(wire::time),
    "link": Link::PullRequests(project.project.id(), PullRequestTab::Open).path(),
  })
}

/// How the latest sync went, by name.
const fn sync(status: &SyncStatus) -> &'static str {
  match status {
    SyncStatus::NotSynced => "not_synced",
    SyncStatus::Synced(_) => "synced",
    SyncStatus::Unreachable { .. } => "github_unreachable",
    SyncStatus::Troubled { .. } => "github_server_error",
    SyncStatus::Failed { .. } => "failed",
    SyncStatus::RateLimited { .. } => "rate_limited",
    SyncStatus::Paused { .. } => "paused_other_account",
    SyncStatus::Account(_) => "gh_unavailable",
  }
}

pub(super) fn item(project: &OpenProject, id: ProjectId, listed: &ListedPullRequest) -> Value {
  let pull_request = &listed.pull_request;
  json!({
    "project": { "id": id.to_string(), "name": project.project.name().as_str() },
    "number": pull_request.number(),
    "title": pull_request.title(),
    "state": pull_request.state().as_str(),
    "review": review(pull_request.review_decision()),
    "checks": checks(pull_request),
    "head": pull_request.head().as_str(),
    "base": pull_request.base().as_str(),
    "author": pull_request.author(),
    "changes": {
      "files": pull_request.changes().files,
      "additions": pull_request.changes().additions,
      "deletions": pull_request.changes().deletions,
    },
    "updated_at": wire::time(pull_request.updated_at()),
    "merged_at": pull_request.merged_at().map(wire::time),
    "closed_at": pull_request.closed_at().map(wire::time),
    "from_fork": pull_request.is_cross_repository(),
    "task": listed.task.as_ref().map(|task| linked_task(id, task)),
    "link": Link::PullRequest(id, number(pull_request)).path(),
    "url": pull_request.url(),
  })
}

/// `pull_request`'s number, which is never 0.
fn number(pull_request: &PullRequest) -> NonZeroU64 {
  NonZeroU64::new(pull_request.number()).expect("a pull request's constructor refuses number 0")
}

fn checks(pull_request: &PullRequest) -> Value {
  let checks = pull_request.checks();
  json!({
    "passed": checks.passed,
    "failed": checks.failed,
    "running": checks.pending,
    "skipped": checks.skipped,
    "revision": checks.on.as_str(),
  })
}

fn linked_task(project: ProjectId, task: &LinkedTask) -> Value {
  json!({
    "key": task.key.to_string(),
    "title": &*task.title,
    "link": Link::Task(project, task.key.clone(), TaskPage::Summary).path(),
    "run": { "id": task.run.to_string(), "number": task.run_number.get() },
    "agent_working": task.agent_working,
  })
}

/// What GitHub's reviews decided; `review_not_required` when the repository asks for none.
const fn review(decision: Option<ReviewDecision>) -> &'static str {
  match decision {
    None => "review_not_required",
    Some(decision) => decision.as_str(),
  }
}
