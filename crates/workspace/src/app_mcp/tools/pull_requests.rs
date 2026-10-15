//! `list_pull_requests` and `get_pull_request`: the pull requests of a project's linked GitHub repository
//! as the Pull requests page lists them ([`crate::pull_requests::list`]), with their state, checks, review
//! state and the task and run whose branch they are of. They read what the last sync stored; `refresh`
//! syncs first, through `gh`, as the page's Refresh does. Every item carries its app `link` and its
//! GitHub `url`.

use orchester_core::{
  id::ProjectId,
  pull_request::{PullRequest, ReviewDecision},
};
use rmcp::schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::wire;
use crate::{
  app_mcp::{
    error::{Missing, ToolError},
    services::{Services, blocking},
  },
  pull_requests::{
    InboxError,
    linked::LinkedTask,
    links,
    list::{
      LinkedList, ListedPullRequest, PullRequestList, PullRequestSort, PullRequestTab, load_list_in, sync_list_in,
    },
    status::SyncStatus,
  },
  workspace::OpenProject,
};

/// A tab of the Pull requests page.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[schemars(crate = "rmcp::schemars")]
pub enum Tab {
  /// Open, drafts included.
  Open,
  /// Open, ready for review, and GitHub asks for a review.
  NeedsReview,
  /// Open, and the run that made its branch is running.
  AgentWorking,
  Merged,
  /// Closed without merging.
  Closed,
}

impl From<Tab> for PullRequestTab {
  fn from(tab: Tab) -> Self {
    match tab {
      Tab::Open => Self::Open,
      Tab::NeedsReview => Self::NeedsReview,
      Tab::AgentWorking => Self::AgentWorking,
      Tab::Merged => Self::Merged,
      Tab::Closed => Self::Closed,
    }
  }
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct ListPullRequests {
  /// The project's id, or its name; without it, every project's, each pull request naming its project.
  #[serde(default)]
  pub project: Option<String>,
  /// Only the pull requests of this tab of the Pull requests page; without it, every one the last sync read.
  #[serde(default)]
  pub tab: Option<Tab>,
  /// A number (`42`, `#42`), words of the title, a branch, or a task's key.
  #[serde(default)]
  pub search: Option<String>,
  /// Syncs with GitHub through `gh` first, as the page's Refresh does; needs a project.
  #[serde(default)]
  pub refresh: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(crate = "rmcp::schemars")]
pub struct GetPullRequest {
  /// The project's id, or its name.
  pub project: String,
  /// The pull request's number.
  pub number: u64,
  /// Syncs with GitHub through `gh` first, as the page's Refresh does.
  #[serde(default)]
  pub refresh: bool,
}

pub async fn list_pull_requests(services: &Services, args: ListPullRequests) -> Result<Value, ToolError> {
  if args.refresh && args.project.is_none() {
    return Err(ToolError::InvalidArguments(
      "refresh syncs one project: name it".to_owned(),
    ));
  }
  let services = services.clone();
  let named = args.project.clone();
  let lists = blocking(move || {
    let projects = match named {
      Some(named) => vec![services.project(&named)?],
      None => every_project(&services)?,
    };
    projects
      .into_iter()
      .map(|project| {
        let list = read(&services, &project, args.refresh)?;
        Ok((project, list))
      })
      .collect::<Result<Vec<_>, ToolError>>()
  })
  .await?;
  let query = args.search.as_deref().unwrap_or_default();
  let mut pull_requests = Vec::new();
  let mut repositories = Vec::new();
  for (project, list) in &lists {
    let id = project.project.id();
    repositories.push(repository(project, list));
    let PullRequestList::Linked(linked) = list else {
      continue;
    };
    let rows = match args.tab {
      Some(tab) => linked.rows(tab.into(), query, PullRequestSort::RecentlyUpdated),
      None => all_rows(linked, query),
    };
    pull_requests.extend(rows.iter().map(|listed| item(project, id, listed)));
  }
  Ok(json!({ "pull_requests": pull_requests, "repositories": repositories }))
}

pub async fn get_pull_request(services: &Services, args: GetPullRequest) -> Result<Value, ToolError> {
  let services = services.clone();
  let (project, list) = blocking(move || {
    let project = services.project(&args.project)?;
    let list = read(&services, &project, args.refresh)?;
    Ok((project, list))
  })
  .await?;
  let missing = || ToolError::not_found(Missing::PullRequest, format!("#{}", args.number));
  let PullRequestList::Linked(linked) = &list else {
    return Err(missing());
  };
  let listed = linked.find(args.number).ok_or_else(missing)?;
  let mut found = item(&project, project.project.id(), listed);
  found["repository"] = repository(&project, &list);
  Ok(found)
}

/// Every project whose folder is there.
fn every_project(services: &Services) -> Result<Vec<OpenProject>, ToolError> {
  services
    .workspace
    .entries()?
    .into_iter()
    .filter(|entry| !entry.folder_missing)
    .map(|entry| services.project(&entry.project.id.to_string()))
    .collect()
}

fn read(services: &Services, project: &OpenProject, refresh: bool) -> Result<PullRequestList, ToolError> {
  let root = &project.repo_root;
  let list = if refresh {
    sync_list_in(root, &services.programs)
  } else {
    load_list_in(root)
  };
  list.map_err(|error: InboxError| ToolError::refused("project_data", vec![error.message()]))
}

/// Every pull request of `linked` a search for `query` finds, most recently updated first.
fn all_rows(linked: &LinkedList, query: &str) -> Vec<ListedPullRequest> {
  linked
    .pull_requests
    .iter()
    .filter(|listed| listed.matches(query))
    .cloned()
    .collect()
}

/// The repository a project's pull requests come from and how its last sync went.
fn repository(project: &OpenProject, list: &PullRequestList) -> Value {
  let named = json!({
    "id": project.project.id().to_string(),
    "name": project.project.name().as_str(),
  });
  let PullRequestList::Linked(linked) = list else {
    return json!({ "project": named, "repository": null, "link": null });
  };
  let (sync, last) = sync(&linked.status());
  json!({
    "project": named,
    "repository": linked.repo.to_string(),
    "expected_account": linked.expected.as_str(),
    "synced_at": linked.synced_at.map(wire::time),
    "sync": sync,
    "last_synced_at": last.map(wire::time),
    "link": links::pull_requests(project.project.id(), PullRequestTab::Open),
  })
}

/// How the last sync went, by name, and when the list shown was read.
fn sync(status: &SyncStatus) -> (&'static str, Option<orchester_core::time::Timestamp>) {
  match status {
    SyncStatus::NotSynced => ("not_synced", None),
    SyncStatus::Synced(at) => ("synced", Some(*at)),
    SyncStatus::Unreachable { last } => ("github_unreachable", *last),
    SyncStatus::Troubled { last } => ("github_server_error", *last),
    SyncStatus::Failed { last } => ("failed", *last),
    SyncStatus::RateLimited { .. } => ("rate_limited", None),
    SyncStatus::Paused { .. } => ("paused_other_account", None),
    SyncStatus::Account(_) => ("gh_unavailable", None),
  }
}

fn item(project: &OpenProject, id: ProjectId, listed: &ListedPullRequest) -> Value {
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
    "task": listed.task.as_ref().map(|task| linked_task(id, task)),
    "link": links::pull_request(id, pull_request.number()),
    "url": pull_request.url(),
  })
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
    "link": links::task(project, &task.key),
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
